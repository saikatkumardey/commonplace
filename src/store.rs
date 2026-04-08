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
