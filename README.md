# commonplace

Agent-agnostic long-term memory CLI with BM25 search. Zero dependencies. Single binary.

## Why a Commonplace Book?

The [commonplace book](https://en.wikipedia.org/wiki/Commonplace_book) is a knowledge management technique from the Renaissance (1500s+). Thinkers like John Locke, Francis Bacon, and Isaac Newton kept a single notebook organized by topic headings — writing down observations, quotes, ideas, and facts as they encountered them. When they needed to recall something, they scanned the relevant heading.

The technique survived for 500 years because it works. It's simple (just a notebook and headings), durable (plain text survives anything), and it scales (Locke maintained his for decades). Luhmann's Zettelkasten (1960s) and modern tools like Obsidian are descendants of the same idea.

AI agents have the same problem Renaissance scholars had: they accumulate knowledge across sessions but have no persistent memory. Vector databases and knowledge graphs are the modern answer, but they're complex, fragile, and overkill for most agents. An agent doesn't need a database — it needs a notebook.

## How This Works

`commonplace` is a digital commonplace book for AI agents:

- **Topics are headings.** Each topic (`preferences`, `decisions`, `errors`) is a separate markdown file.
- **Entries are timestamped observations.** When an agent learns something, it writes a one-liner under the relevant topic.
- **Recall uses BM25 search.** Instead of scanning headings manually, the agent searches across all topics and gets ranked results.
- **Files are human-readable.** You can open any topic file in a text editor, grep it, or cat it. No binary database to corrupt or migrate.

The tradeoff vs. vector embeddings: BM25 is lexical, not semantic. It won't match "TDD" when you search "test driven development." The mitigation is simple — write keyword-rich entries: `"prefers TDD (test-driven development) workflow"`. Agents that write well recall well, just like humans with good note-taking habits.

## Install

**One-line install** (Linux/macOS, auto-detects OS and arch):

```bash
curl -fsSL https://raw.githubusercontent.com/saikatkumardey/commonplace/master/install.sh | sh
```

**Build from source:**

```bash
cargo install --path .
```

## Usage

```bash
# Write memories
commonplace write preferences "likes TDD (test-driven development)"
commonplace write decisions "chose BM25 over vector search for memory"
commonplace write errors "mock tests passed but prod migration failed"

# Read a topic
commonplace read preferences

# Search across all topics (BM25 ranked)
commonplace search "testing"
# [preferences] - 2026-04-08: likes TDD (test-driven development) (score: 2.41)
# [errors] - 2026-04-08: mock tests passed but prod migration failed (score: 0.92)

# List topics
commonplace topics
# decisions    1 entry
# errors       1 entry
# preferences  1 entry

# Forget
commonplace forget preferences "TDD"
# Removed 1 entry:
# - 2026-04-08: likes TDD (test-driven development)
```

## Storage

```
~/.commonplace/
  preferences.md    # one markdown file per topic
  decisions.md
  errors.md
  .index            # BM25 index (auto-rebuilt if missing)
```

Override with `COMMONPLACE_HOME` env var.

Topic files are plain markdown, human-readable and human-editable:

```markdown
# preferences

- 2026-04-08: likes TDD (test-driven development)
- 2026-04-08: prefers flat files over databases
```

## Search

BM25 ranking (k1=1.2, b=0.75) across all topics. Each entry is a document. Results sorted by relevance score.

```bash
commonplace search "testing approach" --limit 5
```

Index is persistent (`.index` binary file), invalidated on write/forget, rebuilt automatically when missing or corrupt.

## Agent Integration

Any agent with shell access can use commonplace directly:

```bash
# Claude Code / Codex / any CLI agent
commonplace write context "user is building a Telegram bot in Python"
commonplace search "what framework does user prefer"

# Wrap as a tool for SDK-based agents (Python example)
import subprocess
result = subprocess.run(["commonplace", "search", query], capture_output=True, text=True)
print(result.stdout)
```

### Suggested Topics

| Topic | What goes in it |
|-------|----------------|
| `preferences` | How the user likes to work |
| `decisions` | Technical choices and their rationale |
| `errors` | Bugs encountered and their fixes |
| `context` | Project descriptions, architecture notes |
| `people` | Who the user is, their skills, their role |

### Writing Good Entries

BM25 is keyword-based. Entries recall better when they contain the words you'd search for:

```bash
# Bad — won't match "test driven development"
commonplace write preferences "likes TDD"

# Good — matches both "TDD" and "test driven development"
commonplace write preferences "likes TDD (test-driven development), always write tests first"
```

## Benchmarks

### Performance

| Operation | 100 entries | 1000 entries |
|-----------|------------|-------------|
| Write | 1ms | 2ms |
| Read | — | 2ms |
| Search (cold) | 5ms | 8ms |
| Search (warm) | 2ms | 2ms |
| Forget | — | 2ms |

Run `./bench.sh commonplace` to reproduce.

### Recall Quality

| Category | Score | Notes |
|----------|-------|-------|
| Exact keyword recall | 5/5 | Query terms match stored terms |
| Partial/synonym recall | 2/5 | BM25 can't infer TDD = "test driven development" |
| Cross-topic search | 3/5 | Fails when query shares no tokens with answer |
| Noise resistance | 1/1 | Irrelevant queries correctly return nothing |
| Ranking quality | 2/2 | Right topic ranks first |
| **Total** | **12/18 (66%)** | |

66% is the honest BM25 baseline. The gap is semantic — not a bug, it's the tradeoff for zero dependencies. Write keyword-rich entries and recall improves significantly.

Run `./bench_recall.sh commonplace` to reproduce.

## Design

- **Zero dependencies** — stdlib only, no crates
- **Human-readable storage** — plain markdown, works with grep/cat/vim
- **BM25 search** — relevance-ranked, not just substring match
- **446KB binary** — instant startup, no runtime
- **No network, no daemon, no config** — just files

## License

MIT
