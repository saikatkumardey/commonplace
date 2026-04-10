# commonplace

Long-term memory for AI agents. Single binary. Hybrid BM25 + semantic search.

## The Oldest Trick in the Book

In 1706, John Locke published a method he'd used for decades: keep a notebook, organize it by topic headings, and write down anything worth remembering. He called it a "commonplace book." He wasn't the first — Francis Bacon did the same, and so did Isaac Newton, Thomas Jefferson, and Mark Twain. The method predates all of them, going back to Renaissance scholars in the 1500s.

The idea is almost embarrassingly simple. You have a notebook. You pick a heading — "observations on light," "useful Latin phrases," "things that went wrong." When you learn something that fits, you write it under that heading. When you need to recall something, you scan the heading. That's it.

It worked for 500 years. Locke maintained his for his entire career. Luhmann built a variant (the Zettelkasten) and used it to write 70 books. The technique survived the printing press, the typewriter, the personal computer, and the internet. Every era reinvents it: Zettelkasten, wikis, Evernote, Notion, Obsidian. The format changes. The idea never does.

## The Problem It Solves

AI agents have the same problem Renaissance scholars had: they learn things and then forget them.

Every session, an agent discovers your preferences, makes decisions, hits errors, and builds context. Then the session ends and it's all gone. Next session, the agent is a stranger again. It asks the same questions. Makes the same mistakes. Rediscovers the same preferences.

## What This Is

`commonplace` is a digital commonplace book for AI agents. It's a Rust CLI that any agent can call via shell:

```bash
# An agent learns something
commonplace write preferences "likes TDD (test-driven development), always write tests first"
commonplace write errors "importlib.metadata fails in uv tool installs — use version.local_version() instead"
commonplace write decisions "chose hybrid search over pure BM25 — semantic recall for synonyms"

# Next session, the agent searches before starting work
commonplace search "testing approach"
# [preferences] - 2026-04-08: likes TDD (test-driven development), always write tests first (score: 2.41)

# Or reads an entire topic
commonplace read errors
```

That's it. Write things down. Search when you need them. The same thing Locke did, except the notebook is `~/.commonplace/` and the headings are markdown files.

## How Search Works

Search uses a hybrid of two signals merged via Reciprocal Rank Fusion (RRF):

**BM25** — keyword ranking. Fast, zero latency, works immediately on install. If you write "likes TDD (test-driven development)" it's findable by any of those terms.

**Semantic** — cosine similarity over AllMiniLM-L6-v2 embeddings (via fastembed, runs locally). Finds "test first" when you stored "TDD". Catches synonyms and paraphrases BM25 misses.

The hybrid beats either alone. BM25 handles exact terminology; semantic handles meaning. Results are merged and re-ranked before returning.

Semantic search requires a one-time model download (~80MB). If the model isn't cached, search falls back to BM25-only automatically — no errors, just keyword matching.

### Supersession Detection

When you write a new entry, commonplace checks it against existing entries in the same topic. If any existing entry has cosine similarity > 0.88, it warns you before appending:

```
Warning: similar entry exists:
  - 2026-03-01: prefers PostgreSQL for production databases
Continue anyway? [y/N]
```

Use `--force` to skip the check in non-interactive contexts.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/saikatkumardey/commonplace/master/install.sh | sh
```

Auto-detects Linux/macOS, amd64/arm64. Or build from source: `cargo install --path .`

After install, run once to download the embedding model:

```bash
commonplace init     # downloads AllMiniLM-L6-v2 (~80MB, one-time)
commonplace embed    # backfills existing entries into the semantic index
```

## Commands

| Command | What it does |
|---------|-------------|
| `commonplace write <topic> <entry> [--force]` | Append entry; warns if similar exists |
| `commonplace read <topic>` | Print all entries in a topic |
| `commonplace search <query> [--limit N]` | Hybrid BM25 + semantic search |
| `commonplace search <query> [--semantic]` | Force semantic path |
| `commonplace topics` | List all topics with entry counts |
| `commonplace forget <topic> <search>` | Remove matching entries + embeddings |
| `commonplace init` | Download and cache the embedding model |
| `commonplace embed` | Backfill existing entries into the semantic index |

## Storage

```
~/.commonplace/
  preferences.md      # one markdown file per topic
  decisions.md
  errors.md
  .index              # BM25 index (auto-rebuilt if missing)
  embeddings.db       # SQLite: entry vectors for semantic search
```

Override location with `COMMONPLACE_HOME` env var. Topic files are plain markdown — open them in any editor, grep them, cat them.

## Agent Integration

### Claude Code

Install the [commonplace plugin](https://github.com/saikatkumardey/commonplace-plugin):

```bash
claude plugin add saikatkumardey/commonplace-plugin
/commonplace setup   # installs session hooks
```

Claude Code will automatically recall memories at session start and summarize the session when it ends.

### Any agent with shell access

```bash
commonplace write context "user is building a Telegram bot in Python"
commonplace search "what framework does user prefer"
```

### System prompt snippet

The repo includes [`agent-prompt.md`](agent-prompt.md) — drop it into your agent's instructions (CLAUDE.md, AGENTS.md, or system prompt).

## Design

- **Human-readable storage** — plain markdown files, one per topic
- **Hybrid search** — BM25 for keywords, semantic for meaning, RRF to merge
- **Local embeddings** — AllMiniLM-L6-v2 via fastembed, no API key needed
- **Graceful degradation** — falls back to BM25-only if model not cached
- **Supersession detection** — warns before writing near-duplicate entries
- **No daemon, no config** — just files and a binary

## License

MIT

## Built by [Saikat Kumar Dey](https://saikatkumardey.com)
