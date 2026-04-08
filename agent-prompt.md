# Commonplace Memory

You have access to `commonplace`, a persistent long-term memory tool. Use it to remember facts, decisions, preferences, and context across sessions.

## Commands

```bash
commonplace write <topic> <entry>      # Save a memory
commonplace read <topic>               # Read all entries in a topic
commonplace search <query> [--limit N] # BM25 search across all topics
commonplace topics                     # List all topics
commonplace forget <topic> <search>    # Remove matching entries
```

## When to Use

**Recall** at the start of a task — search for relevant past context before starting work:
```bash
commonplace search "relevant keywords"
```

**Store immediately** when any of these happen:
- Error resolved → `commonplace write errors "description of error and fix"`
- Decision made → `commonplace write decisions "what was chosen and why"`
- User preference discovered → `commonplace write preferences "what they prefer"`
- Significant task completed → `commonplace write context "what was done"`

## Writing Good Entries

BM25 is keyword-based. Include synonyms and related terms so entries are findable:

```bash
# Bad — won't match "test driven development"
commonplace write preferences "likes TDD"

# Good — matches both
commonplace write preferences "likes TDD (test-driven development), always write tests first"
```

## Suggested Topics

| Topic | What goes in it |
|-------|----------------|
| `preferences` | How the user likes to work, coding style, communication preferences |
| `decisions` | Technical choices and their rationale |
| `errors` | Bugs encountered and how they were fixed |
| `context` | Project descriptions, architecture, infrastructure notes |
| `people` | Who the user is, their skills, their role |
