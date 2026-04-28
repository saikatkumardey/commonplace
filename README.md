# commonplace

[![CI](https://github.com/saikatkumardey/commonplace/actions/workflows/ci.yml/badge.svg)](https://github.com/saikatkumardey/commonplace/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/saikatkumardey/commonplace/branch/master/graph/badge.svg)](https://codecov.io/gh/saikatkumardey/commonplace)

You've spent 20 minutes explaining your preferences to an agent, it fixes the problem, and next week you do it again. Not because the agent is bad. Because it forgot.

## The Oldest Trick in the Book

In 1706, John Locke published a method he'd used for decades: keep a notebook, organize it by topic headings, and write down anything worth remembering. He called it a commonplace book. He wasn't the first. Francis Bacon did the same, and so did Isaac Newton, Thomas Jefferson, and Mark Twain. The method goes back to Renaissance scholars in the 1500s.

The idea is almost embarrassingly simple. You pick a heading — "observations on light," "useful Latin phrases," "things that went wrong." When you learn something that fits, you write it there. When you need to recall something, you check the heading. That's it.

It worked for 500 years. Locke maintained his for his entire career. Luhmann built a variant (the Zettelkasten) and used it to write 70 books. Every era reinvents it: Zettelkasten, wikis, Evernote, Notion, Obsidian. The format changes. The idea never does.

## The Problem It Solves

AI agents have the same problem Renaissance scholars had: they learn things and then forget them.

Every session, an agent discovers your preferences, makes decisions, hits errors, and builds context. Then the session ends and it's gone. Next session, the agent is a stranger again. It asks the same questions. Makes the same mistakes. Rediscovers the same preferences.

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

Write things down. Search when you need them. The same thing Locke did, except the notebook is `~/.commonplace/` and the headings are markdown files.

## How Search Works

Search uses a hybrid of two signals merged via Reciprocal Rank Fusion (RRF).

**BM25** — keyword ranking. Fast, zero latency, works immediately on install. If you write "likes TDD (test-driven development)" it's findable by any of those terms.

**Semantic** — cosine similarity over AllMiniLM-L6-v2 embeddings (via fastembed, runs locally). Finds "test first" when you stored "TDD". Catches synonyms and paraphrases BM25 misses.

The hybrid beats either alone. BM25 handles exact terminology; semantic handles meaning. Results are merged and re-ranked before returning.

Semantic search requires a one-time model download (~80MB). If the model isn't cached, search falls back to BM25-only automatically. No errors, just keyword matching.

### Recency and Reinforcement

Two memories with equal relevance shouldn't rank equally if one was confirmed yesterday and the other was written once two years ago. After merging BM25 + semantic scores, every result is multiplied by a boost:

- **Recency** — exponential decay with a 365-day half-life on the entry's last-confirmed date. A two-year-old entry is worth ~25% of a fresh one.
- **Reinforcement** — `1 + 0.5·ln(N)` where N is the `[×N]` counter from consolidation. An entry confirmed three times is ~1.55x; ten times is ~2.15x.

So a stale single-mention entry can be outranked by a more recent, repeatedly-reaffirmed one even when its raw relevance score is higher. Combined with consolidation, this means the notebook self-curates: things you keep saying float up, things you said once and never repeated drift down.

### Consolidation

Real notebooks get edited, not just appended to. When you write a new entry, commonplace compares it (semantically) against existing entries in the same topic and acts:

- **Reaffirm** (cosine ≥ 0.95) — the new entry restates an existing one. The existing line's date is bumped to today and a `[×N]` counter is incremented. No new line is added. Repeated confirmations strengthen a memory instead of cluttering the file.
- **Supersede** (0.85 ≤ cosine < 0.95) — the new entry refines or replaces an old one. The old line is removed, the new one is written, and the change is logged.
- **Append** (cosine < 0.85) — the new entry is novel; appended as usual.

Both reaffirm and supersede write an audit record to `.tombstones.md`:

```markdown
## 2026-04-28 [decisions] supersede
old: - 2025-09-01: chose pure BM25 search
new: - 2026-04-28: chose hybrid search over pure BM25 — semantic recall for synonyms
```

So nothing is lost — you can always replay how an entry evolved. Pass `--force` to bypass consolidation and append unconditionally. If the embedding model isn't cached, `write` falls back to plain append.

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
| `commonplace write <topic> <entry> [--force]` | Add entry; consolidates near-duplicates (reaffirm or supersede). `--force` always appends |
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
  .tombstones.md      # audit log of reaffirmed/superseded entries
  embeddings.db       # SQLite: entry vectors for semantic search
```

Override location with `COMMONPLACE_HOME` env var. Topic files are plain markdown. Open them in any editor, grep them, cat them.

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
- **Consolidation, not just append** — near-duplicate entries reaffirm or supersede, with a tombstone audit log
- **Recency + reinforcement ranking** — search boosts recently-confirmed and repeatedly-reaffirmed entries
- **No daemon, no config** — just files and a binary

## License

MIT
