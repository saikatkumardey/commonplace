#!/usr/bin/env python3
"""
MemoryBench benchmark for commonplace.

Evaluates whether commonplace can retrieve relevant evidence when searching
with questions from the MemoryBench dataset (Locomo subset).

Methodology:
- For each example: write all conversation turns to a temp commonplace store
- Search with the question
- Check if any evidence text appears in top-k results (k=1, k=3)
- Report Recall@1 and Recall@3

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


def parse_conversation_turns(context_content: str) -> list[tuple[str, str]]:
    """
    Parse conversation turns from the context string.
    Returns list of (session_header, speaker_line) pairs.
    """
    turns = []
    current_session = "general"

    for line in context_content.split("\n"):
        line = line.strip()
        # Session header like "Coversation [8:56 pm on 20 July, 2023]:"
        session_match = re.match(r"Co[nv]ersation \[([^\]]+)\]", line)
        if session_match:
            current_session = session_match.group(1)
            continue
        # Speaker turn like "Speaker Carolinesays : text"
        speaker_match = re.match(r"Speaker (\w+)says : (.+)", line)
        if speaker_match:
            speaker = speaker_match.group(1)
            text = speaker_match.group(2)
            turns.append((current_session, f"{speaker}: {text}"))

    return turns


def write_memories(home_dir: str, turns: list[tuple[str, str]]) -> int:
    """Write conversation turns to commonplace. Returns count written."""
    written = 0
    for session, text in turns:
        result = subprocess.run(
            [str(BINARY), "write", "conversation", text, "--force"],
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


def extract_result_text(result_line: str) -> str:
    """
    Extract entry text from result line.
    Format: [topic] - DATE: text (score: X.XX)
    """
    # Remove score suffix
    text = re.sub(r"\s*\(score: [\d.]+\)\s*$", "", result_line)
    # Remove topic prefix like "[conversation] - DATE: "
    text = re.sub(r"^\[.*?\] - .*?: ", "", text)
    return text.lower()


def evidence_in_results(evidence_texts: list[str], result_lines: list[str]) -> bool:
    """
    Check if any evidence text fragment appears in any result line.
    Uses a substring match on a meaningful chunk (first 40 chars) of evidence.
    """
    results_combined = " ".join(extract_result_text(r) for r in result_lines).lower()

    for ev_text in evidence_texts:
        # Use first 40 non-trivial characters as the key fragment
        fragment = ev_text.strip().lower()
        # Try progressively shorter fragments until we find a match or give up
        for length in [60, 40, 25]:
            if len(fragment) >= length:
                chunk = fragment[:length]
                if chunk in results_combined:
                    return True
        # Also try key words from the evidence
        words = [w for w in fragment.split() if len(w) > 4]
        if len(words) >= 3:
            key_words = words[:3]
            if all(w in results_combined for w in key_words):
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

        # Get full conversation context from dialog_bm25_dialog (full convo retrieval)
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
        turns = parse_conversation_turns(context_content)

        if not turns:
            errors += 1
            continue

        with tempfile.TemporaryDirectory() as tmpdir:
            written = write_memories(tmpdir, turns)

            if written == 0:
                errors += 1
                continue

            # Search with top-1
            results_1 = search_memories(tmpdir, question, limit=1)
            # Search with top-3
            results_3 = search_memories(tmpdir, question, limit=3)

            hit_at_1 = evidence_in_results(evidence_texts, results_1)
            hit_at_3 = evidence_in_results(evidence_texts, results_3)

            if hit_at_1:
                recall_at_1 += 1
            if hit_at_3:
                recall_at_3 += 1

        if (i + 1) % 10 == 0:
            print(
                f"  [{i+1}/{total}] Recall@1={recall_at_1/(i+1-errors):.3f}  "
                f"Recall@3={recall_at_3/(i+1-errors):.3f}  "
                f"(errors: {errors})"
            )

    evaluated = total - errors
    r1 = recall_at_1 / evaluated if evaluated > 0 else 0.0
    r3 = recall_at_3 / evaluated if evaluated > 0 else 0.0

    print()
    print("=" * 50)
    print("MemoryBench Results (Locomo subset)")
    print("=" * 50)
    print(f"Total examples:   {total}")
    print(f"Evaluated:        {evaluated}")
    print(f"Errors/skipped:   {errors}")
    print(f"Recall@1:         {r1:.3f}  ({recall_at_1}/{evaluated})")
    print(f"Recall@3:         {r3:.3f}  ({recall_at_3}/{evaluated})")
    print()

    return {
        "total": total,
        "evaluated": evaluated,
        "errors": errors,
        "recall_at_1": r1,
        "recall_at_3": r3,
        "recall_at_1_count": recall_at_1,
        "recall_at_3_count": recall_at_3,
    }


if __name__ == "__main__":
    if not BINARY.exists():
        print(f"Binary not found at {BINARY}")
        print("Run: cargo build --release")
        sys.exit(1)

    results = run_benchmark()
