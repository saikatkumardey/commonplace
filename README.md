# commonplace

Long-term memory for AI agents. Zero dependencies. Single binary.

## The Oldest Trick in the Book

In 1706, John Locke published a method he'd used for decades: keep a notebook, organize it by topic headings, and write down anything worth remembering. He called it a "commonplace book." He wasn't the first — Francis Bacon did the same, and so did Isaac Newton, Thomas Jefferson, and Mark Twain. The method predates all of them, going back to Renaissance scholars in the 1500s.

The idea is almost embarrassingly simple. You have a notebook. You pick a heading — "observations on light," "useful Latin phrases," "things that went wrong." When you learn something that fits, you write it under that heading. When you need to recall something, you scan the heading. That's it.

It worked for 500 years. Locke maintained his for his entire career. Luhmann built a variant (the Zettelkasten) and used it to write 70 books. The technique survived the printing press, the typewriter, the personal computer, and the internet. Every era reinvents it: Zettelkasten, wikis, Evernote, Notion, Obsidian. The format changes. The idea never does.

## The Problem It Solves

AI agents have the same problem Renaissance scholars had: they learn things and then forget them.

Every session, an agent discovers your preferences, makes decisions, hits errors, and builds context. Then the session ends and it's all gone. Next session, the agent is a stranger again. It asks the same questions. Makes the same mistakes. Rediscovers the same preferences.

The industry answer is vector databases and knowledge graphs. Embed everything, build ontologies, run a daemon, maintain a schema. It works, but it's complex — and complexity is where things break. We tried this path (ChromaDB + SQLite knowledge graph + semantic search). It added 44 dependencies, needed a running process, and the recall quality wasn't worth the infrastructure.

So we went back to the oldest trick in the book.

## What This Is

`commonplace` is a digital commonplace book for AI agents. It's a Rust CLI that any agent can call via shell:

```bash
# An agent learns something
commonplace write preferences "likes TDD (test-driven development), always write tests first"
commonplace write errors "importlib.metadata fails in uv tool installs — use version.local_version() instead"
commonplace write decisions "chose BM25 over vector search — zero deps, 66% recall, good enough"

# Next session, the agent searches before starting work
commonplace search "testing approach"
# [preferences] - 2026-04-08: likes TDD (test-driven development), always write tests first (score: 2.41)

# Or reads an entire topic
commonplace read errors
# # errors
#
# - 2026-04-08: importlib.metadata fails in uv tool installs — use version.local_version() instead
# - 2026-04-08: mock database tests passed but production migration failed silently
```

That's it. Write things down. Search when you need them. The same thing Locke did, except the notebook is `~/.commonplace/` and the headings are markdown files.

```
                         ~/.commonplace/
                        ┌──────────────────────────────────────────┐
                        │                                          │
  Session 1             │  preferences.md     decisions.md         │
  ───────────           │  ┌──────────────┐   ┌────────────────┐  │
                        │  │ # preferences│   │ # decisions    │  │
  Agent learns:         │  │              │   │                │  │
  "user likes TDD" ────────│ - 2026-04-08:│   │ - 2026-04-08: │  │
                        │  │   likes TDD  │   │   chose BM25   │  │
  Agent decides:        │  │   (test-     │   │   over vector  │  │
  "use BM25" ──────────────│   driven     │   │   search       │  │
                        │  │   develop-   │   └────────────────┘  │
                        │  │   ment)      │                       │
                        │  └──────────────┘   errors.md           │
                        │                     ┌────────────────┐  │
  Session 2             │    .index           │ # errors       │  │
  ───────────           │    ┌──────────┐     │                │  │
                        │    │ BM25     │     │ - 2026-04-08: │  │
  Agent searches:       │    │ reverse  │     │   mock tests   │  │
  "testing" ─────────────────│ index    │     │   passed but   │  │
       │                │    │ (binary) │     │   prod failed  │  │
       │                │    └──────────┘     └────────────────┘  │
       │                │                                          │
       ▼                └──────────────────────────────────────────┘
  Ranked results:
  [preferences] likes TDD (score: 2.41)
  [errors] mock tests passed... (score: 0.92)
```

## The Tradeoff

Search uses [BM25](https://en.wikipedia.org/wiki/Okapi_BM25) — a ranking algorithm from information retrieval, not neural embeddings. It matches on keywords, not meaning. If you search "test driven development," it won't find an entry that only says "TDD."

This is the conscious tradeoff for zero dependencies. No embeddings model, no vector database, no Python runtime. Just a 446KB static binary.

The mitigation is the same thing that makes real commonplace books work: write well. An entry that says `"likes TDD (test-driven development), always write tests first"` is findable by any of those terms. Agents that write keyword-rich entries recall well. Agents that write tersely don't. Just like humans with good and bad note-taking habits.

Our recall benchmarks are honest about this:

| Category | Score |
|----------|-------|
| Exact keyword recall | 5/5 |
| Partial/synonym recall | 2/5 |
| Cross-topic search | 3/5 |
| Noise resistance (no false positives) | 1/1 |
| Ranking quality (right topic first) | 2/2 |
| **Total** | **12/18 (66%)** |

66% with zero dependencies and sub-10ms latency. For most agent use cases — "what does the user prefer," "what went wrong last time," "what did we decide about X" — keyword match is enough.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/saikatkumardey/commonplace/master/install.sh | sh
```

Auto-detects Linux/macOS, amd64/arm64. Or build from source: `cargo install --path .`

## Commands

| Command | What it does |
|---------|-------------|
| `commonplace write <topic> <entry>` | Append a timestamped entry to a topic |
| `commonplace read <topic>` | Print all entries in a topic |
| `commonplace search <query> [--limit N]` | BM25 search across all topics |
| `commonplace topics` | List all topics with entry counts |
| `commonplace forget <topic> <search>` | Remove matching entries |

## Storage

```
~/.commonplace/
  preferences.md    # one markdown file per topic
  decisions.md
  errors.md
  .index            # BM25 index (auto-rebuilt if missing)
```

Override location with `COMMONPLACE_HOME` env var. Topic files are plain markdown — open them in any editor, grep them, cat them. No binary database to corrupt or migrate.

## Agent Integration

### Claude Code

Install the [commonplace plugin](https://github.com/saikatkumardey/commonplace-plugin):

```bash
claude plugin add saikatkumardey/commonplace-plugin
```

Claude Code will automatically recall and store memories when relevant.

### Any agent with shell access

```bash
commonplace write context "user is building a Telegram bot in Python"
commonplace search "what framework does user prefer"
```

### System prompt snippet

The repo includes [`agent-prompt.md`](agent-prompt.md) — drop it into your agent's instructions (CLAUDE.md, AGENTS.md, or system prompt).

## Performance

| Operation | 100 entries | 1000 entries |
|-----------|------------|-------------|
| Write | 1ms | 2ms |
| Read | — | 2ms |
| Search (cold index rebuild) | 5ms | 8ms |
| Search (warm index) | 2ms | 2ms |
| Forget | — | 2ms |

Run `./bench.sh` and `./bench_recall.sh` to reproduce.

## Design

- **Zero dependencies** — Rust stdlib only, no crates
- **Human-readable storage** — plain markdown files
- **BM25 search** — relevance-ranked, not substring match
- **446KB binary** — instant startup, no runtime
- **No network, no daemon, no config** — just files

## License

MIT
