mod bm25;
mod consolidate;
mod embeddings;
mod semantic;
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
        "init" => cmd_init(),
        "embed" => cmd_embed(&home),
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

fn cmd_init() -> Result<(), String> {
    println!("Loading embedding model (may download ~80MB on first run)...");
    let _embedder = semantic::Embedder::new()?;
    println!("Model ready.");
    Ok(())
}

fn cmd_embed(home: &std::path::Path) -> Result<(), String> {
    let entries = store::all_entries(home).map_err(|e| e.to_string())?;
    if entries.is_empty() {
        println!("No entries to embed.");
        return Ok(());
    }

    let store = embeddings::EmbeddingStore::open(home)
        .map_err(|e| format!("failed to open embedding store: {}", e))?;
    let embedder = semantic::Embedder::new()?;

    println!("Embedding {} entries...", entries.len());

    let texts: Vec<&str> = entries.iter().map(|(_, t)| t.as_str()).collect();
    let vecs = embedder.embed_batch(&texts)?;

    for ((topic, text), vec) in entries.iter().zip(vecs.iter()) {
        store
            .upsert(topic, text, vec)
            .map_err(|e| format!("failed to store embedding: {}", e))?;
    }

    println!("Done. {} entries embedded.", entries.len());
    Ok(())
}

fn cmd_write(home: &std::path::Path, args: &[String]) -> Result<(), String> {
    let force = args.iter().any(|a| a == "--force");
    let filtered: Vec<&String> = args.iter().filter(|a| *a != "--force").collect();

    if filtered.len() < 2 {
        return Err("usage: commonplace write <topic> <entry> [--force]".into());
    }
    let topic = store::normalize_topic(filtered[0]);
    if topic.is_empty() {
        return Err("topic name is empty after normalization".into());
    }
    let entry = filtered[1..].iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ");
    if entry.trim().is_empty() {
        return Err("entry cannot be empty".into());
    }

    if force {
        store::write_entry(home, &topic, &entry).map_err(|e| e.to_string())?;
        bm25::Index::invalidate(home);
        if semantic::model_is_cached() {
            if let Ok(emb_store) = embeddings::EmbeddingStore::open(home) {
                if let Ok(embedder) = semantic::Embedder::new() {
                    let line = format!("- {}: {}", store::today(), entry);
                    if let Ok(v) = embedder.embed_one(&line) {
                        let _ = emb_store.upsert(&topic, &line, &v);
                    }
                }
            }
        }
        return Ok(());
    }

    let outcome = consolidate::consolidate(home, &topic, &entry)?;
    bm25::Index::invalidate(home);
    match outcome {
        consolidate::Outcome::Appended => {}
        consolidate::Outcome::Reaffirmed { old, new, count } => {
            eprintln!("reaffirmed (\u{00d7}{}): {}", count, new);
            eprintln!("  was: {}", old);
        }
        consolidate::Outcome::Superseded { old, new } => {
            eprintln!("superseded: {}", new);
            eprintln!("  was: {}", old);
            eprintln!("  (logged to .tombstones.md; pass --force to skip consolidation)");
        }
    }
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
        return Err("usage: commonplace search <query> [--limit N] [--semantic]".into());
    }

    let mut limit = 10usize;
    let mut semantic_flag = false;
    let mut query_parts = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--limit" && i + 1 < args.len() {
            limit = args[i + 1].parse().map_err(|_| "invalid --limit value")?;
            i += 2;
        } else if args[i] == "--semantic" {
            semantic_flag = true;
            i += 1;
        } else {
            query_parts.push(args[i].as_str());
            i += 1;
        }
    }
    let query = query_parts.join(" ");

    // BM25 search always
    let index = bm25::Index::load_or_build(home).map_err(|e| e.to_string())?;
    let bm25_results = index.search(&query, 20);

    // Try semantic search if flag set or if DB exists
    let db_path = home.join("embeddings.db");
    let use_semantic = semantic_flag || db_path.exists();

    if use_semantic && db_path.exists() {
        // Try to do hybrid search
        if let Ok(emb_store) = embeddings::EmbeddingStore::open(home) {
            if let Ok(embedder) = semantic::Embedder::new() {
                if let Ok(query_vec) = embedder.embed_one(&query) {
                    if let Ok(all_entries) = emb_store.all() {
                        // Compute cosine similarities
                        let mut sem_scored: Vec<(usize, f32)> = all_entries
                            .iter()
                            .enumerate()
                            .map(|(i, (_, _, vec))| (i, embeddings::cosine(&query_vec, vec)))
                            .collect();
                        sem_scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

                        // Take top 20 semantic results
                        let sem_top: Vec<(usize, f32)> = sem_scored.into_iter().take(20).collect();

                        // RRF merge
                        // Build a map of (topic, text) -> rrf_score
                        use std::collections::HashMap;
                        let mut rrf: HashMap<(String, String), f64> = HashMap::new();

                        // BM25 contributions
                        for (rank, r) in bm25_results.iter().enumerate() {
                            let key = (r.topic.clone(), r.text.clone());
                            *rrf.entry(key).or_default() += 1.0 / (rank as f64 + 60.0);
                        }

                        // Semantic contributions
                        for (rank, (idx, _)) in sem_top.iter().enumerate() {
                            let (topic, text, _) = &all_entries[*idx];
                            let key = (topic.clone(), text.clone());
                            *rrf.entry(key).or_default() += 1.0 / (rank as f64 + 60.0);
                        }

                        // Sort by RRF score
                        let mut merged: Vec<((String, String), f64)> = rrf.into_iter().collect();
                        merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                        merged.truncate(limit);

                        if merged.is_empty() {
                            println!("no results");
                            return Ok(());
                        }

                        for ((topic, text), score) in &merged {
                            println!("[{}] {} (score: {:.2})", topic, text, score);
                        }
                        return Ok(());
                    }
                }
            }
        }
        // Fall through to BM25 only if semantic failed
    }

    // BM25 only
    let results: Vec<_> = bm25_results.into_iter().take(limit).collect();
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

        // Best-effort: delete from embeddings DB
        if let Ok(emb_store) = embeddings::EmbeddingStore::open(home) {
            for r in &removed {
                let _ = emb_store.delete(&topic, r);
            }
        }
    }
    Ok(())
}

fn usage() {
    eprintln!(
        "commonplace - agent-agnostic long-term memory with BM25 + semantic search

USAGE:
    commonplace write <topic> <entry> [--force]   Add an entry; consolidates near-duplicates
    commonplace read <topic>                       Print a topic
    commonplace search <query> [--limit N]         Hybrid BM25+semantic search
                              [--semantic]         Force semantic-only path
    commonplace topics                             List all topics with entry counts
    commonplace forget <topic> <search>            Remove matching entries
    commonplace init                               Download and cache embedding model (~80MB)
    commonplace embed                              Backfill all entries into embeddings DB

ENVIRONMENT:
    COMMONPLACE_HOME    Data directory (default: ~/.commonplace)

CONSOLIDATION:
    By default, 'write' compares the new entry against existing entries in the
    same topic. If similarity is very high (>=0.95) the existing entry is
    reaffirmed (date bumped, [\u{00d7}N] counter incremented). If similarity is
    high (>=0.85) the existing entry is superseded by the new one. Replaced
    entries are logged to .tombstones.md for audit. Pass --force to bypass
    consolidation and always append.

NOTES:
    Run 'commonplace init' once after install to download the AllMiniLM-L6-v2 model.
    Run 'commonplace embed' to backfill existing entries into the semantic index.
    Subsequent 'write' commands auto-embed if the model is cached."
    );
}
