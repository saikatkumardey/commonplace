use crate::embeddings::{cosine, EmbeddingStore};
use crate::store;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const REAFFIRM_THRESHOLD: f32 = 0.95;
pub const SUPERSEDE_THRESHOLD: f32 = 0.85;

#[derive(Debug)]
pub enum Outcome {
    Appended,
    Reaffirmed { old: String, new: String, count: u32 },
    Superseded { old: String, new: String },
}

fn tombstone_path(home: &Path) -> PathBuf {
    home.join(".tombstones.md")
}

pub fn log_tombstone(
    home: &Path,
    kind: &str,
    topic: &str,
    old: &str,
    new: &str,
) -> std::io::Result<()> {
    fs::create_dir_all(home)?;
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(tombstone_path(home))?;
    writeln!(
        f,
        "## {} [{}] {}\nold: {}\nnew: {}\n",
        store::today(),
        topic,
        kind,
        old,
        new
    )?;
    Ok(())
}

/// Parse a `- YYYY-MM-DD: text` line, optionally with a trailing ` [×N]` counter.
/// Returns (date, body_without_counter, count). Count defaults to 1.
pub fn parse_line(line: &str) -> Option<(String, String, u32)> {
    if !line.starts_with("- ") {
        return None;
    }
    let rest = &line[2..];
    if rest.len() < 12 || &rest[10..12] != ": " {
        return None;
    }
    if !rest[..10]
        .chars()
        .all(|c| c.is_ascii_digit() || c == '-')
    {
        return None;
    }
    let date = rest[..10].to_string();
    let body_full = rest[12..].to_string();

    // Strip trailing ` [×N]` if present.
    if body_full.ends_with(']') {
        if let Some(i) = body_full.rfind(" [\u{00d7}") {
            let prefix = " [\u{00d7}".len();
            let count_str = &body_full[i + prefix..body_full.len() - 1];
            if let Ok(n) = count_str.parse::<u32>() {
                return Some((date, body_full[..i].to_string(), n));
            }
        }
    }
    Some((date, body_full, 1))
}

pub fn render_line(date: &str, body: &str, count: u32) -> String {
    if count <= 1 {
        format!("- {}: {}", date, body)
    } else {
        format!("- {}: {} [\u{00d7}{}]", date, body, count)
    }
}

/// Read all "- ..." lines for a topic, ignoring header and blanks.
fn topic_entry_lines(home: &Path, topic: &str) -> Vec<String> {
    match store::read_topic(home, topic) {
        Ok(content) => content
            .lines()
            .filter(|l| l.starts_with("- "))
            .map(|l| l.to_string())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Consolidate a new entry against existing entries in the same topic. The
/// caller provides `embed_fn` so this is testable without loading the model.
///
/// Comparison is done on entry *bodies* (no date prefix, no counter) so that
/// metadata noise doesn't bias similarity. Storage embeddings keep the
/// existing convention (full formatted line).
pub fn consolidate_with<F>(
    home: &Path,
    topic: &str,
    new_entry: &str,
    embed_fn: F,
) -> Result<Outcome, String>
where
    F: Fn(&str) -> Result<Vec<f32>, String>,
{
    let today = store::today();
    let new_line = format!("- {}: {}", today, new_entry);

    let new_body_vec = embed_fn(new_entry)?;

    let emb_store = EmbeddingStore::open(home)
        .map_err(|e| format!("failed to open embedding store: {}", e))?;

    let existing_lines = topic_entry_lines(home, topic);
    let mut best: Option<(String, f32)> = None;
    for line in &existing_lines {
        let body = match parse_line(line) {
            Some((_, b, _)) => b,
            None => continue,
        };
        let v = match embed_fn(&body) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let sim = cosine(&new_body_vec, &v);
        if best.as_ref().map_or(true, |(_, s)| sim > *s) {
            best = Some((line.clone(), sim));
        }
    }

    if let Some((old_line, sim)) = best {
        if sim >= REAFFIRM_THRESHOLD {
            if let Some((_, body, count)) = parse_line(&old_line) {
                let new_count = count + 1;
                let updated = render_line(&today, &body, new_count);
                if updated != old_line {
                    store::replace_line(home, topic, &old_line, &updated)
                        .map_err(|e| e.to_string())?;
                }
                let _ = emb_store.delete(topic, &old_line);
                if let Ok(v) = embed_fn(&updated) {
                    let _ = emb_store.upsert(topic, &updated, &v);
                }
                let _ = log_tombstone(home, "reaffirm", topic, &old_line, &updated);
                return Ok(Outcome::Reaffirmed {
                    old: old_line,
                    new: updated,
                    count: new_count,
                });
            }
        }
        if sim >= SUPERSEDE_THRESHOLD {
            store::remove_line(home, topic, &old_line).map_err(|e| e.to_string())?;
            store::write_entry(home, topic, new_entry).map_err(|e| e.to_string())?;
            let _ = emb_store.delete(topic, &old_line);
            if let Ok(v) = embed_fn(&new_line) {
                let _ = emb_store.upsert(topic, &new_line, &v);
            }
            let _ = log_tombstone(home, "supersede", topic, &old_line, &new_line);
            return Ok(Outcome::Superseded {
                old: old_line,
                new: new_line,
            });
        }
    }

    // No similar entry → append.
    store::write_entry(home, topic, new_entry).map_err(|e| e.to_string())?;
    if let Ok(v) = embed_fn(&new_line) {
        let _ = emb_store.upsert(topic, &new_line, &v);
    }
    Ok(Outcome::Appended)
}

/// Real-embedder convenience wrapper. Falls back to plain append if the model
/// or embedding store cannot be loaded.
pub fn consolidate(home: &Path, topic: &str, new_entry: &str) -> Result<Outcome, String> {
    if !crate::semantic::model_is_cached() {
        store::write_entry(home, topic, new_entry).map_err(|e| e.to_string())?;
        return Ok(Outcome::Appended);
    }
    let embedder = match crate::semantic::Embedder::new() {
        Ok(e) => e,
        Err(_) => {
            store::write_entry(home, topic, new_entry).map_err(|e| e.to_string())?;
            return Ok(Outcome::Appended);
        }
    };
    consolidate_with(home, topic, new_entry, |t| embedder.embed_one(t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store;
    use tempfile::tempdir;

    #[test]
    fn parse_plain_line() {
        let (d, b, c) = parse_line("- 2026-04-28: hello world").unwrap();
        assert_eq!(d, "2026-04-28");
        assert_eq!(b, "hello world");
        assert_eq!(c, 1);
    }

    #[test]
    fn parse_with_counter() {
        let (d, b, c) = parse_line("- 2026-04-28: prefers TDD [\u{00d7}3]").unwrap();
        assert_eq!(d, "2026-04-28");
        assert_eq!(b, "prefers TDD");
        assert_eq!(c, 3);
    }

    #[test]
    fn render_omits_counter_for_one() {
        assert_eq!(
            render_line("2026-04-28", "hi", 1),
            "- 2026-04-28: hi"
        );
    }

    #[test]
    fn render_includes_counter_for_n() {
        assert_eq!(
            render_line("2026-04-28", "hi", 5),
            "- 2026-04-28: hi [\u{00d7}5]"
        );
    }

    #[test]
    fn parse_render_roundtrip() {
        let line = "- 2026-04-28: hello [\u{00d7}7]";
        let (d, b, c) = parse_line(line).unwrap();
        assert_eq!(render_line(&d, &b, c), line);
    }

    #[test]
    fn parse_rejects_non_entry() {
        assert!(parse_line("# header").is_none());
        assert!(parse_line("").is_none());
        assert!(parse_line("- not a date: oops").is_none());
    }

    /// Build a deterministic fake "embedding" from text by hashing tokens into
    /// a small bag-of-words vector. Identical text → cosine 1.0; overlapping
    /// text → high similarity; disjoint → near 0.
    fn fake_embed(text: &str) -> Result<Vec<f32>, String> {
        const DIM: usize = 64;
        let mut v = vec![0.0f32; DIM];
        for tok in text.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
            if tok.is_empty() {
                continue;
            }
            let mut h: u64 = 1469598103934665603;
            for b in tok.as_bytes() {
                h ^= *b as u64;
                h = h.wrapping_mul(1099511628211);
            }
            let i = (h as usize) % DIM;
            v[i] += 1.0;
        }
        Ok(v)
    }

    #[test]
    fn appended_when_no_existing() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        let out =
            consolidate_with(home, "prefs", "loves rust programming", fake_embed).unwrap();
        assert!(matches!(out, Outcome::Appended));
        let content = store::read_topic(home, "prefs").unwrap();
        assert!(content.contains("loves rust programming"));
    }

    #[test]
    fn appended_when_dissimilar() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        consolidate_with(home, "prefs", "loves rust programming", fake_embed).unwrap();
        let out = consolidate_with(
            home,
            "prefs",
            "completely different topic about gardening tomatoes",
            fake_embed,
        )
        .unwrap();
        assert!(matches!(out, Outcome::Appended), "got {:?}", out);
        let content = store::read_topic(home, "prefs").unwrap();
        assert!(content.contains("loves rust programming"));
        assert!(content.contains("gardening tomatoes"));
    }

    #[test]
    fn reaffirmed_when_identical() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        consolidate_with(home, "prefs", "likes TDD test driven development", fake_embed)
            .unwrap();
        let out = consolidate_with(
            home,
            "prefs",
            "likes TDD test driven development",
            fake_embed,
        )
        .unwrap();
        match out {
            Outcome::Reaffirmed { count, .. } => assert_eq!(count, 2),
            other => panic!("expected Reaffirmed, got {:?}", other),
        }
        let content = store::read_topic(home, "prefs").unwrap();
        // Only one entry line should exist (the bumped one).
        let entries: Vec<&str> =
            content.lines().filter(|l| l.starts_with("- ")).collect();
        assert_eq!(entries.len(), 1, "got: {:?}", entries);
        assert!(entries[0].ends_with("[\u{00d7}2]"), "got: {}", entries[0]);

        // Reaffirm again → counter goes to 3.
        let out = consolidate_with(
            home,
            "prefs",
            "likes TDD test driven development",
            fake_embed,
        )
        .unwrap();
        match out {
            Outcome::Reaffirmed { count, .. } => assert_eq!(count, 3),
            other => panic!("expected Reaffirmed, got {:?}", other),
        }
        let content = store::read_topic(home, "prefs").unwrap();
        let entries: Vec<&str> =
            content.lines().filter(|l| l.starts_with("- ")).collect();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("[\u{00d7}3]"));
    }

    #[test]
    fn superseded_when_medium_similarity() {
        // Insert a baseline using fake_embed.
        let dir = tempdir().unwrap();
        let home = dir.path();
        let base = "uses postgres for relational data storage backend";
        consolidate_with(home, "decisions", base, fake_embed).unwrap();

        // Near-paraphrase: same 7 base tokens plus 2 extras → cosine ~0.88,
        // which lands in the [SUPERSEDE_THRESHOLD, REAFFIRM_THRESHOLD) band.
        let new_entry = "uses postgres for relational data storage backend system database";
        let out = consolidate_with(home, "decisions", new_entry, fake_embed).unwrap();
        match out {
            Outcome::Superseded { old, new } => {
                assert!(old.contains(base));
                assert!(new.contains(new_entry));
            }
            other => panic!("expected Superseded, got {:?}", other),
        }

        let content = store::read_topic(home, "decisions").unwrap();
        let entries: Vec<&str> =
            content.lines().filter(|l| l.starts_with("- ")).collect();
        assert_eq!(entries.len(), 1, "old entry should be replaced; got {:?}", entries);
        assert!(entries[0].contains(new_entry));

        let tomb = std::fs::read_to_string(home.join(".tombstones.md")).unwrap();
        assert!(tomb.contains("supersede"), "got: {}", tomb);
    }

    #[test]
    fn tombstone_logged_on_reaffirm() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        consolidate_with(home, "prefs", "loves rust", fake_embed).unwrap();
        consolidate_with(home, "prefs", "loves rust", fake_embed).unwrap();
        let tomb = std::fs::read_to_string(home.join(".tombstones.md")).unwrap();
        assert!(tomb.contains("reaffirm"), "got: {}", tomb);
        assert!(tomb.contains("[prefs]"));
    }

    #[test]
    fn force_path_skips_consolidation() {
        // Sanity: write_entry alone (the --force fallback in main) appends
        // even when content is identical; we don't go through consolidate.
        let dir = tempdir().unwrap();
        let home = dir.path();
        store::write_entry(home, "prefs", "loves rust").unwrap();
        store::write_entry(home, "prefs", "loves rust").unwrap();
        let content = store::read_topic(home, "prefs").unwrap();
        let entries: Vec<&str> =
            content.lines().filter(|l| l.starts_with("- ")).collect();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn tombstones_file_not_listed_as_topic() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        store::write_entry(home, "prefs", "hi").unwrap();
        log_tombstone(home, "supersede", "prefs", "old", "new").unwrap();
        let topics = store::list_topics(home).unwrap();
        let names: Vec<&str> = topics.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["prefs"], "tombstones file leaked: {:?}", names);
    }
}
