use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

pub fn home_dir() -> PathBuf {
    if let Ok(p) = std::env::var("COMMONPLACE_HOME") {
        PathBuf::from(p)
    } else if let Ok(h) = std::env::var("HOME") {
        PathBuf::from(h).join(".commonplace")
    } else {
        PathBuf::from(".commonplace")
    }
}

pub fn normalize_topic(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn topic_path(home: &Path, topic: &str) -> PathBuf {
    home.join(format!("{}.md", topic))
}

pub fn today() -> String {
    // Unix timestamp → calendar date, no dependencies
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let days = secs / 86400;
    let mut y = 1970i32;
    let mut remaining = days;

    loop {
        let dy = if is_leap(y) { 366 } else { 365 };
        if remaining < dy {
            break;
        }
        remaining -= dy;
        y += 1;
    }

    let leap = is_leap(y);
    let months: [i64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];
    let mut m = 0;
    for &dm in &months {
        if remaining < dm {
            break;
        }
        remaining -= dm;
        m += 1;
    }

    format!("{:04}-{:02}-{:02}", y, m + 1, remaining + 1)
}

fn is_leap(y: i32) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

pub fn write_entry(home: &Path, topic: &str, entry: &str) -> io::Result<()> {
    fs::create_dir_all(home)?;
    let path = topic_path(home, topic);
    let exists = path.exists();

    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;

    if !exists {
        writeln!(f, "# {}\n", topic)?;
    }

    writeln!(f, "- {}: {}", today(), entry)?;
    Ok(())
}

pub fn read_topic(home: &Path, topic: &str) -> io::Result<String> {
    let path = topic_path(home, topic);
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("topic '{}' not found", topic),
        ));
    }
    fs::read_to_string(&path)
}

pub fn list_topics(home: &Path) -> io::Result<Vec<(String, usize)>> {
    let mut topics = Vec::new();
    let entries = match fs::read_dir(home) {
        Ok(e) => e,
        Err(_) => return Ok(topics),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let count = count_entries(&path);
            topics.push((name, count));
        }
    }
    topics.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(topics)
}

fn count_entries(path: &Path) -> usize {
    let f = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    io::BufReader::new(f)
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| l.starts_with("- "))
        .count()
}

pub fn forget_entries(home: &Path, topic: &str, search: &str) -> io::Result<Vec<String>> {
    let path = topic_path(home, topic);
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("topic '{}' not found", topic),
        ));
    }

    let content = fs::read_to_string(&path)?;
    let search_lower = search.to_lowercase();
    let mut kept = Vec::new();
    let mut removed = Vec::new();

    for line in content.lines() {
        if line.starts_with("- ") && line.to_lowercase().contains(&search_lower) {
            removed.push(line.to_string());
        } else {
            kept.push(line.to_string());
        }
    }

    // Remove trailing empty lines
    while kept.last().map_or(false, |l| l.is_empty()) {
        kept.pop();
    }

    let mut out = kept.join("\n");
    out.push('\n');
    fs::write(&path, out)?;

    Ok(removed)
}

/// Read all entries across all topics: (topic, line_text)
pub fn all_entries(home: &Path) -> io::Result<Vec<(String, String)>> {
    let mut results = Vec::new();
    let entries = match fs::read_dir(home) {
        Ok(e) => e,
        Err(_) => return Ok(results),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let topic = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let f = fs::File::open(&path)?;
        for line in io::BufReader::new(f).lines() {
            let line = line?;
            if line.starts_with("- ") {
                results.push((topic.clone(), line.clone()));
            }
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn normalize_topic_lowercases() {
        assert_eq!(normalize_topic("Preferences"), "preferences");
    }

    #[test]
    fn normalize_topic_spaces_become_dashes() {
        assert_eq!(normalize_topic("my topic"), "my-topic");
    }

    #[test]
    fn normalize_topic_trims_dashes() {
        assert_eq!(normalize_topic("-foo-"), "foo");
    }

    #[test]
    fn normalize_topic_special_chars() {
        assert_eq!(normalize_topic("foo!bar"), "foo-bar");
    }

    #[test]
    fn today_is_valid_date() {
        let date = today();
        // Must match YYYY-MM-DD
        assert!(
            date.len() == 10,
            "date '{}' is not 10 chars",
            date
        );
        let parts: Vec<&str> = date.split('-').collect();
        assert_eq!(parts.len(), 3, "expected 3 parts in '{}'", date);
        let year: u32 = parts[0].parse().expect("year is numeric");
        let month: u32 = parts[1].parse().expect("month is numeric");
        let day: u32 = parts[2].parse().expect("day is numeric");
        assert!(year >= 2025, "year {} should be >= 2025", year);
        assert!((1..=12).contains(&month), "month {} out of range", month);
        assert!((1..=31).contains(&day), "day {} out of range", day);
    }

    #[test]
    fn write_then_read_roundtrip() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        write_entry(home, "test", "hello world").unwrap();
        let content = read_topic(home, "test").unwrap();
        assert!(content.contains("hello world"), "content: {}", content);
    }

    #[test]
    fn write_creates_header() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        write_entry(home, "mytopic", "some entry").unwrap();
        let content = read_topic(home, "mytopic").unwrap();
        assert!(
            content.starts_with("# mytopic"),
            "expected header, got: {}",
            content
        );
    }

    #[test]
    fn write_multiple_entries() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        write_entry(home, "multi", "entry one").unwrap();
        write_entry(home, "multi", "entry two").unwrap();
        write_entry(home, "multi", "entry three").unwrap();
        let content = read_topic(home, "multi").unwrap();
        assert!(content.contains("entry one"), "missing entry one");
        assert!(content.contains("entry two"), "missing entry two");
        assert!(content.contains("entry three"), "missing entry three");
    }

    #[test]
    fn read_nonexistent_topic_errors() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        let result = read_topic(home, "nosuchttopic");
        assert!(result.is_err(), "expected Err for missing topic");
    }

    #[test]
    fn list_topics_empty() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        let topics = list_topics(home).unwrap();
        assert!(topics.is_empty(), "expected empty list, got {:?}", topics);
    }

    #[test]
    fn list_topics_counts() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        write_entry(home, "prefs", "pref one").unwrap();
        write_entry(home, "prefs", "pref two").unwrap();
        write_entry(home, "errors", "error one").unwrap();
        let topics = list_topics(home).unwrap();
        // sorted alphabetically: errors, prefs
        assert_eq!(topics.len(), 2);
        let errors = topics.iter().find(|(n, _)| n == "errors").unwrap();
        let prefs = topics.iter().find(|(n, _)| n == "prefs").unwrap();
        assert_eq!(errors.1, 1, "errors count");
        assert_eq!(prefs.1, 2, "prefs count");
    }

    #[test]
    fn forget_removes_matching() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        write_entry(home, "notes", "keep this one").unwrap();
        write_entry(home, "notes", "remove the apple").unwrap();
        write_entry(home, "notes", "keep this too").unwrap();
        forget_entries(home, "notes", "apple").unwrap();
        let content = read_topic(home, "notes").unwrap();
        let entry_lines: Vec<&str> = content.lines().filter(|l| l.starts_with("- ")).collect();
        assert_eq!(entry_lines.len(), 2, "expected 2 remaining entries");
        assert!(!content.contains("remove the apple"), "apple entry should be gone");
    }

    #[test]
    fn forget_case_insensitive() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        write_entry(home, "notes", "This has UPPERCASE text").unwrap();
        write_entry(home, "notes", "this stays").unwrap();
        forget_entries(home, "notes", "uppercase").unwrap();
        let content = read_topic(home, "notes").unwrap();
        let entry_lines: Vec<&str> = content.lines().filter(|l| l.starts_with("- ")).collect();
        assert_eq!(entry_lines.len(), 1, "expected 1 remaining entry");
        assert!(!content.contains("UPPERCASE"), "uppercase entry should be gone");
    }

    #[test]
    fn forget_nonexistent_topic_errors() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        let result = forget_entries(home, "ghost", "anything");
        assert!(result.is_err(), "expected Err for missing topic");
    }

    #[test]
    fn all_entries_returns_all() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        write_entry(home, "alpha", "alpha entry one").unwrap();
        write_entry(home, "beta", "beta entry one").unwrap();
        write_entry(home, "beta", "beta entry two").unwrap();
        let entries = all_entries(home).unwrap();
        assert_eq!(entries.len(), 3, "expected 3 total entries, got {}", entries.len());
        let alpha_count = entries.iter().filter(|(t, _)| t == "alpha").count();
        let beta_count = entries.iter().filter(|(t, _)| t == "beta").count();
        assert_eq!(alpha_count, 1);
        assert_eq!(beta_count, 2);
    }
}
