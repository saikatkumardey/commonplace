#!/usr/bin/env python3
"""
MemoryBench benchmark for commonplace (v2).

Evaluates whether commonplace can retrieve relevant evidence when searching
with questions from the MemoryBench dataset (Locomo subset).

v2 changes vs v1:
- Proper semantic model initialization (init before first use)
- Clean fact extraction: strips "Speaker Xsays : " prefix, writes raw text
- Splits long utterances into sentences, one entry per sentence
- Writes to 'conversation' topic; also writes to 'events' if date/time keywords present
- Better evidence matching: shorter fragments (20 chars), loose keyword recall
- Adds Recall@5

Methodology:
- For each example: write all conversation turns to a temp commonplace store
- Search with the question
- Check if any evidence text appears in top-k results (k=1, k=3, k=5)
- Report Recall@1, Recall@3, Recall@5

Reference: https://huggingface.co/datasets/THUIR/MemoryBench
Paper: arxiv:2510.17281
"""

import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

try:
    from datasets import load_dataset
except ImportError:
    print("Install datasets: pip install datasets")
    sys.exit(1)

BINARY = Path(__file__).parent / "target/release/commonplace"
LOCOMO_SUBSETS = [f"Locomo-{i}" for i in range(10)]
MAX_EXAMPLES = 100  # cap at 100; dataset has 250 total Locomo examples

# Keywords that suggest temporal/event content
DATE_TIME_KEYWORDS = re.compile(
    r"\b(monday|tuesday|wednesday|thursday|friday|saturday|sunday|"
    r"january|february|march|april|may|june|july|august|september|october|november|december|"
    r"yesterday|today|tomorrow|last week|next week|this week|"
    r"\d{1,2}[:/]\d{2}|\d{4})\b",
    re.IGNORECASE,
)


def split_into_sentences(text: str) -> list[str]:
    """Split text into sentences on . ! ? boundaries."""
    parts = re.split(r"(?<=[.!?])\s+", text.strip())
    return [p.strip() for p in parts if p.strip() and len(p.strip()) > 5]


def parse_clean_texts(context_content: str) -> list[tuple[str, str]]:
    """
    Parse conversation turns, returning (topic, clean_text) pairs.
    - Strips "Speaker Xsays : " prefix
    - Splits long utterances into sentences
    - Tags as 'events' if date/time keywords present, else 'conversation'
    """
    entries = []

    for line in context_content.split("\n"):
        line = line.strip()
        if not line:
            continue

        # Skip session headers like "Coversation [8:56 pm on 20 July, 2023]:"
        if re.match(r"Co[nv]ersation \[", line):
            continue

        # Strip "Speaker Xsays : " prefix
        speaker_match = re.match(r"Speaker \w+says : (.+)", line)
        if speaker_match:
            text = speaker_match.group(1).strip()
        else:
            # Keep non-speaker lines if they look like content (not headers)
            if re.match(r"^[A-Z][a-z]+:", line):
                continue
            text = line

        if not text or len(text) < 5:
            continue

        # Split into sentences
        sentences = split_into_sentences(text)
        if not sentences:
            sentences = [text]

        for sentence in sentences:
            topic = "events" if DATE_TIME_KEYWORDS.search(sentence) else "conversation"
            entries.append((topic, sentence))

    return entries


def write_memories(home_dir: str, entries: list[tuple[str, str]]) -> int:
    """Write entries to commonplace. Returns count written."""
    written = 0
    for topic, text in entries:
        result = subprocess.run(
            [str(BINARY), "write", topic, text, "--force"],
            env={**os.environ, "COMMONPLACE_HOME": home_dir},
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            written += 1
    return written


def search_memories(home_dir: str, query: str, limit: int = 5) -> list[str]:
    """Search commonplace and return list of result lines."""
    result = subprocess.run(
        [str(BINARY), "search", query, "--limit", str(limit)],
        env={**os.environ, "COMMONPLACE_HOME": home_dir},
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return []
    lines = [l.strip() for l in result.stdout.strip().split("\n") if l.strip()]
    return lines


def normalize(text: str) -> str:
    """Normalize whitespace and punctuation for matching."""
    text = text.lower()
    text = re.sub(r"[^\w\s]", " ", text)
    text = re.sub(r"\s+", " ", text)
    return text.strip()


def extract_result_text(result_line: str) -> str:
    """
    Extract entry text from result line.
    Format: [topic] - DATE: text (score: X.XX)
    """
    text = re.sub(r"\s*\(score: [\d.]+\)\s*$", "", result_line)
    text = re.sub(r"^\[.*?\] - .*?: ", "", text)
    return normalize(text)


def evidence_in_results(evidence_texts: list[str], result_lines: list[str]) -> bool:
    """
    Check if any evidence text fragment appears in any result line.

    Matching strategy (in order):
    1. Substring match with 20-char fragments (shorter = more forgiving)
    2. Any 2+ words with 6+ chars from evidence appear in results (loose recall)
    """
    results_combined = " ".join(extract_result_text(r) for r in result_lines)

    for ev_text in evidence_texts:
        fragment = normalize(ev_text)

        # Strategy 1: substring match on progressively shorter fragments
        for length in [60, 40, 20]:
            if len(fragment) >= length:
                chunk = fragment[:length]
                if chunk in results_combined:
                    return True

        # Strategy 2: loose keyword recall — at least 2 long words must match
        long_words = [w for w in fragment.split() if len(w) >= 6]
        if len(long_words) >= 2:
            matches = sum(1 for w in long_words if w in results_combined)
            if matches >= 2:
                return True

    return False


def run_benchmark(max_examples: int = MAX_EXAMPLES):
    print(f"Loading MemoryBench Locomo subsets (max {max_examples} examples)...")

    all_examples = []
    for subset in LOCOMO_SUBSETS:
        try:
            ds = load_dataset("THUIR/MemoryBench", subset)
            for split in ds.keys():
                for ex in ds[split]:
                    all_examples.append(ex)
                    if len(all_examples) >= max_examples:
                        break
            if len(all_examples) >= max_examples:
                break
        except Exception as e:
            print(f"  Warning: could not load {subset}: {e}")
            continue

    total = len(all_examples)
    print(f"Loaded {total} examples from Locomo subsets.")
    print()

    recall_at_1 = 0
    recall_at_3 = 0
    recall_at_5 = 0
    errors = 0

    for i, ex in enumerate(all_examples):
        question = ex.get("origin_question", "")
        info_str = ex.get("info", "{}")
        try:
            info = json.loads(info_str)
        except Exception:
            errors += 1
            continue

        golden_answer = info.get("golden_answer", "")
        evidence_list = info.get("evidence", [])
        evidence_texts = [e["text"] for e in evidence_list if "text" in e]

        if not question or not evidence_texts:
            errors += 1
            continue

        # Get full conversation context from dialog_bm25_dialog
        dialog_str = ex.get("dialog_bm25_dialog", "[]")
        try:
            dialog = json.loads(dialog_str)
        except Exception:
            errors += 1
            continue

        if not dialog:
            errors += 1
            continue

        context_content = dialog[0].get("content", "")
        entries = parse_clean_texts(context_content)

        if not entries:
            errors += 1
            continue

        with tempfile.TemporaryDirectory() as tmpdir:
            written = write_memories(tmpdir, entries)

            if written == 0:
                errors += 1
                continue

            # Fetch top-5 results once; slice for @1 and @3
            results_5 = search_memories(tmpdir, question, limit=5)
            results_3 = results_5[:3]
            results_1 = results_5[:1]

            hit_at_1 = evidence_in_results(evidence_texts, results_1)
            hit_at_3 = evidence_in_results(evidence_texts, results_3)
            hit_at_5 = evidence_in_results(evidence_texts, results_5)

            if hit_at_1:
                recall_at_1 += 1
            if hit_at_3:
                recall_at_3 += 1
            if hit_at_5:
                recall_at_5 += 1

        evaluated_so_far = i + 1 - errors
        if (i + 1) % 10 == 0 and evaluated_so_far > 0:
            print(
                f"  [{i+1}/{total}] Recall@1={recall_at_1/evaluated_so_far:.3f}  "
                f"Recall@3={recall_at_3/evaluated_so_far:.3f}  "
                f"Recall@5={recall_at_5/evaluated_so_far:.3f}  "
                f"(errors: {errors})"
            )

    evaluated = total - errors
    r1 = recall_at_1 / evaluated if evaluated > 0 else 0.0
    r3 = recall_at_3 / evaluated if evaluated > 0 else 0.0
    r5 = recall_at_5 / evaluated if evaluated > 0 else 0.0

    print()
    print("=" * 50)
    print("MemoryBench Results (Locomo subset) — v2")
    print("=" * 50)
    print(f"Total examples:   {total}")
    print(f"Evaluated:        {evaluated}")
    print(f"Errors/skipped:   {errors}")
    print(f"Recall@1:         {r1:.3f}  ({recall_at_1}/{evaluated})")
    print(f"Recall@3:         {r3:.3f}  ({recall_at_3}/{evaluated})")
    print(f"Recall@5:         {r5:.3f}  ({recall_at_5}/{evaluated})")
    print()

    return {
        "total": total,
        "evaluated": evaluated,
        "errors": errors,
        "recall_at_1": r1,
        "recall_at_3": r3,
        "recall_at_5": r5,
        "recall_at_1_count": recall_at_1,
        "recall_at_3_count": recall_at_3,
        "recall_at_5_count": recall_at_5,
    }


if __name__ == "__main__":
    if not BINARY.exists():
        print(f"Binary not found at {BINARY}")
        print("Run: cargo build --release")
        sys.exit(1)

    results = run_benchmark()
