use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"CPB1";
const K1: f64 = 1.2;
const B: f64 = 0.75;

/// A single indexed document.
struct Doc {
    topic: String,
    text: String,
    token_count: u32,
}

/// In-memory BM25 index.
pub struct Index {
    docs: Vec<Doc>,
    /// term → vec of (doc_id, term_frequency)
    postings: HashMap<String, Vec<(u32, u32)>>,
    avg_dl: f64,
}

pub struct SearchResult {
    pub topic: String,
    pub text: String,
    pub score: f64,
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn index_path(home: &Path) -> PathBuf {
    home.join(".index")
}

impl Index {
    pub fn load_or_build(home: &Path) -> io::Result<Self> {
        let path = index_path(home);
        if path.exists() {
            if let Ok(idx) = Self::load(&path) {
                return Ok(idx);
            }
        }
        Self::build(home)
    }

    fn build(home: &Path) -> io::Result<Self> {
        let entries = crate::store::all_entries(home)?;
        let mut docs = Vec::with_capacity(entries.len());
        let mut postings: HashMap<String, Vec<(u32, u32)>> = HashMap::new();
        let mut total_tokens: u64 = 0;

        for (i, (topic, text)) in entries.iter().enumerate() {
            let tokens = tokenize(text);
            let token_count = tokens.len() as u32;
            total_tokens += token_count as u64;

            let mut tf: HashMap<&str, u32> = HashMap::new();
            for t in &tokens {
                *tf.entry(t.as_str()).or_default() += 1;
            }

            for (term, freq) in tf {
                postings
                    .entry(term.to_string())
                    .or_default()
                    .push((i as u32, freq));
            }

            docs.push(Doc {
                topic: topic.clone(),
                text: text.clone(),
                token_count,
            });
        }

        let avg_dl = if docs.is_empty() {
            0.0
        } else {
            total_tokens as f64 / docs.len() as f64
        };

        let idx = Index {
            docs,
            postings,
            avg_dl,
        };
        let _ = idx.save(&index_path(home));
        Ok(idx)
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() || self.docs.is_empty() {
            return Vec::new();
        }

        let n = self.docs.len() as f64;
        let mut scores = vec![0.0f64; self.docs.len()];

        for qt in &query_tokens {
            let posting = match self.postings.get(qt.as_str()) {
                Some(p) => p,
                None => continue,
            };

            let df = posting.len() as f64;
            let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();

            for &(doc_id, tf) in posting {
                let dl = self.docs[doc_id as usize].token_count as f64;
                let tf_f = tf as f64;
                let score = idf * (tf_f * (K1 + 1.0)) / (tf_f + K1 * (1.0 - B + B * dl / self.avg_dl));
                scores[doc_id as usize] += score;
            }
        }

        let mut ranked: Vec<(usize, f64)> = scores
            .iter()
            .enumerate()
            .filter(|(_, &s)| s > 0.0)
            .map(|(i, &s)| (i, s))
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        ranked.truncate(limit);

        ranked
            .into_iter()
            .map(|(i, score)| SearchResult {
                topic: self.docs[i].topic.clone(),
                text: self.docs[i].text.clone(),
                score,
            })
            .collect()
    }

    // Simple binary serialization
    fn save(&self, path: &Path) -> io::Result<()> {
        let mut buf = Vec::new();
        buf.extend_from_slice(MAGIC);

        write_u32(&mut buf, self.docs.len() as u32);
        write_f64(&mut buf, self.avg_dl);

        for doc in &self.docs {
            write_str(&mut buf, &doc.topic);
            write_str(&mut buf, &doc.text);
            write_u32(&mut buf, doc.token_count);
        }

        write_u32(&mut buf, self.postings.len() as u32);
        for (term, posts) in &self.postings {
            write_str(&mut buf, term);
            write_u32(&mut buf, posts.len() as u32);
            for &(doc_id, tf) in posts {
                write_u32(&mut buf, doc_id);
                write_u32(&mut buf, tf);
            }
        }

        let mut f = fs::File::create(path)?;
        f.write_all(&buf)?;
        Ok(())
    }

    fn load(path: &Path) -> io::Result<Self> {
        let data = fs::read(path)?;
        if data.len() < 4 || &data[0..4] != MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad magic"));
        }
        let mut pos = 4;

        let num_docs = read_u32(&data, &mut pos)?;
        let avg_dl = read_f64(&data, &mut pos)?;

        let mut docs = Vec::with_capacity(num_docs as usize);
        for _ in 0..num_docs {
            let topic = read_str(&data, &mut pos)?;
            let text = read_str(&data, &mut pos)?;
            let token_count = read_u32(&data, &mut pos)?;
            docs.push(Doc {
                topic,
                text,
                token_count,
            });
        }

        let num_terms = read_u32(&data, &mut pos)?;
        let mut postings = HashMap::with_capacity(num_terms as usize);
        for _ in 0..num_terms {
            let term = read_str(&data, &mut pos)?;
            let num_posts = read_u32(&data, &mut pos)?;
            let mut posts = Vec::with_capacity(num_posts as usize);
            for _ in 0..num_posts {
                let doc_id = read_u32(&data, &mut pos)?;
                let tf = read_u32(&data, &mut pos)?;
                posts.push((doc_id, tf));
            }
            postings.insert(term, posts);
        }

        Ok(Index {
            docs,
            postings,
            avg_dl,
        })
    }

    /// Invalidate the index so it gets rebuilt next time.
    pub fn invalidate(home: &Path) {
        let _ = fs::remove_file(index_path(home));
    }
}

fn write_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_f64(buf: &mut Vec<u8>, v: f64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_str(buf: &mut Vec<u8>, s: &str) {
    write_u32(buf, s.len() as u32);
    buf.extend_from_slice(s.as_bytes());
}

fn read_u32(data: &[u8], pos: &mut usize) -> io::Result<u32> {
    if *pos + 4 > data.len() {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "truncated"));
    }
    let v = u32::from_le_bytes(data[*pos..*pos + 4].try_into().unwrap());
    *pos += 4;
    Ok(v)
}

fn read_f64(data: &[u8], pos: &mut usize) -> io::Result<f64> {
    if *pos + 8 > data.len() {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "truncated"));
    }
    let v = f64::from_le_bytes(data[*pos..*pos + 8].try_into().unwrap());
    *pos += 8;
    Ok(v)
}

fn read_str(data: &[u8], pos: &mut usize) -> io::Result<String> {
    let len = read_u32(data, pos)? as usize;
    if *pos + len > data.len() {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "truncated"));
    }
    let s = String::from_utf8(data[*pos..*pos + len].to_vec())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    *pos += len;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store;
    use tempfile::tempdir;

    #[test]
    fn build_and_search_finds_exact_match() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        store::write_entry(home, "notes", "likes TDD test driven development").unwrap();
        let idx = Index::load_or_build(home).unwrap();
        let results = idx.search("TDD", 10);
        assert!(!results.is_empty(), "expected at least one result for TDD");
        assert!(
            results[0].text.contains("TDD"),
            "top result should contain TDD, got: {}",
            results[0].text
        );
    }

    #[test]
    fn search_empty_index_returns_empty() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        let idx = Index::load_or_build(home).unwrap();
        let results = idx.search("anything", 10);
        assert!(results.is_empty(), "expected empty results for empty index");
    }

    #[test]
    fn search_respects_limit() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        // Write 5 entries each containing "rust" plus a distinct keyword
        store::write_entry(home, "notes", "rust alpha programming language").unwrap();
        store::write_entry(home, "notes", "rust beta programming language").unwrap();
        store::write_entry(home, "notes", "rust gamma programming language").unwrap();
        store::write_entry(home, "notes", "rust delta programming language").unwrap();
        store::write_entry(home, "notes", "rust epsilon programming language").unwrap();
        let idx = Index::load_or_build(home).unwrap();
        let results = idx.search("rust programming", 2);
        assert_eq!(results.len(), 2, "expected exactly 2 results with limit=2");
    }

    #[test]
    fn index_save_load_roundtrip() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        store::write_entry(home, "tech", "roundtrip serialization test").unwrap();
        // First call: builds and auto-saves the .index file
        let _idx_built = Index::load_or_build(home).unwrap();
        // Remove from memory and reload from disk
        let idx_loaded = Index::load(&index_path(home)).unwrap();
        let results = idx_loaded.search("roundtrip", 10);
        assert!(!results.is_empty(), "search after load should find entry");
        assert!(
            results[0].text.contains("roundtrip"),
            "loaded result should contain 'roundtrip'"
        );
    }

    #[test]
    fn invalidate_removes_index_file() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        store::write_entry(home, "tech", "some content to index").unwrap();
        // Build the index (creates .index file)
        let _idx = Index::load_or_build(home).unwrap();
        assert!(
            index_path(home).exists(),
            ".index file should exist after build"
        );
        Index::invalidate(home);
        assert!(
            !index_path(home).exists(),
            ".index file should be gone after invalidate"
        );
    }
}
