mod bm25;
mod store;

use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        usage();
        process::exit(1);
    }

    let home = store::home_dir();
    let result = match args[1].as_str() {
        "write" => cmd_write(&home, &args[2..]),
        "read" => cmd_read(&home, &args[2..]),
        "search" => cmd_search(&home, &args[2..]),
        "topics" => cmd_topics(&home),
        "forget" => cmd_forget(&home, &args[2..]),
        "--help" | "-h" | "help" => {
            usage();
            Ok(())
        }
        other => {
            eprintln!("unknown command: {}", other);
            usage();
            process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        process::exit(1);
    }
}

fn cmd_write(home: &std::path::Path, args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: commonplace write <topic> <entry>".into());
    }
    let topic = store::normalize_topic(&args[0]);
    if topic.is_empty() {
        return Err("topic name is empty after normalization".into());
    }
    let entry = args[1..].join(" ");
    if entry.trim().is_empty() {
        return Err("entry cannot be empty".into());
    }
    store::write_entry(home, &topic, &entry).map_err(|e| e.to_string())?;
    bm25::Index::invalidate(home);
    Ok(())
}

fn cmd_read(home: &std::path::Path, args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("usage: commonplace read <topic>".into());
    }
    let topic = store::normalize_topic(&args[0]);
    let content = store::read_topic(home, &topic).map_err(|e| e.to_string())?;
    print!("{}", content);
    Ok(())
}

fn cmd_search(home: &std::path::Path, args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("usage: commonplace search <query> [--limit N]".into());
    }

    let mut limit = 10usize;
    let mut query_parts = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--limit" && i + 1 < args.len() {
            limit = args[i + 1].parse().map_err(|_| "invalid --limit value")?;
            i += 2;
        } else {
            query_parts.push(args[i].as_str());
            i += 1;
        }
    }
    let query = query_parts.join(" ");

    let index = bm25::Index::load_or_build(home).map_err(|e| e.to_string())?;
    let results = index.search(&query, limit);

    if results.is_empty() {
        println!("no results");
        return Ok(());
    }

    for r in &results {
        println!("[{}] {} (score: {:.2})", r.topic, r.text, r.score);
    }
    Ok(())
}

fn cmd_topics(home: &std::path::Path) -> Result<(), String> {
    let topics = store::list_topics(home).map_err(|e| e.to_string())?;
    if topics.is_empty() {
        println!("no topics");
        return Ok(());
    }
    let max_len = topics.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
    for (name, count) in &topics {
        let label = if *count == 1 { "entry" } else { "entries" };
        println!("{:width$}  {} {}", name, count, label, width = max_len);
    }
    Ok(())
}

fn cmd_forget(home: &std::path::Path, args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: commonplace forget <topic> <search>".into());
    }
    let topic = store::normalize_topic(&args[0]);
    let search = args[1..].join(" ");
    let removed = store::forget_entries(home, &topic, &search).map_err(|e| e.to_string())?;

    if removed.is_empty() {
        println!("no matching entries");
    } else {
        println!(
            "Removed {} {}:",
            removed.len(),
            if removed.len() == 1 { "entry" } else { "entries" }
        );
        for r in &removed {
            println!("{}", r);
        }
        bm25::Index::invalidate(home);
    }
    Ok(())
}

fn usage() {
    eprintln!(
        "commonplace - agent-agnostic long-term memory with BM25 search

USAGE:
    commonplace write <topic> <entry>      Append a timestamped entry
    commonplace read <topic>               Print a topic
    commonplace search <query> [--limit N] BM25 search across all topics
    commonplace topics                     List all topics with entry counts
    commonplace forget <topic> <search>    Remove matching entries

ENVIRONMENT:
    COMMONPLACE_HOME    Data directory (default: ~/.commonplace)"
    );
}
