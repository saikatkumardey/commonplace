#!/usr/bin/env bash
set -euo pipefail

CP="${1:-commonplace}"
export COMMONPLACE_HOME=$(mktemp -d)
trap "rm -rf $COMMONPLACE_HOME" EXIT

echo "commonplace benchmark"
echo "====================="
echo "binary: $CP"
echo "data dir: $COMMONPLACE_HOME"
echo ""

time_ms() {
    local start end
    start=$(date +%s%N)
    eval "$1" >/dev/null 2>&1
    end=$(date +%s%N)
    echo $(( (end - start) / 1000000 ))
}

# --- Write benchmark ---
echo "## Write latency"
for n in 10 100 500 1000; do
    rm -rf "$COMMONPLACE_HOME"/*
    # Pre-populate n-1 entries
    for i in $(seq 1 $((n - 1))); do
        $CP write "topic-$((i % 5))" "entry number $i about various things like testing debugging and deployment" >/dev/null 2>&1
    done
    ms=$(time_ms "$CP write topic-0 'the final benchmark entry about testing'")
    echo "  $n entries: ${ms}ms (single write)"
done
echo ""

# --- Read benchmark ---
echo "## Read latency"
rm -rf "$COMMONPLACE_HOME"/*
for i in $(seq 1 1000); do
    $CP write "topic-$((i % 5))" "entry number $i about various things like testing debugging and deployment" >/dev/null 2>&1
done
ms=$(time_ms "$CP read topic-0")
echo "  1000 entries (read topic-0): ${ms}ms"
echo ""

# --- Topics benchmark ---
echo "## Topics latency"
rm -rf "$COMMONPLACE_HOME"/*
for i in $(seq 1 50); do
    $CP write "topic-$i" "seed entry for topic $i" >/dev/null 2>&1
done
ms=$(time_ms "$CP topics")
echo "  50 topics: ${ms}ms"
echo ""

# --- Search benchmark (cold index — no .index file) ---
echo "## Search latency (cold — index rebuild)"
for n in 100 500 1000; do
    rm -rf "$COMMONPLACE_HOME"/*
    for i in $(seq 1 $n); do
        $CP write "topic-$((i % 10))" "entry $i about testing deployment debugging monitoring logging alerts" >/dev/null 2>&1
    done
    rm -f "$COMMONPLACE_HOME/.index"
    ms=$(time_ms "$CP search 'testing deployment'")
    echo "  $n entries: ${ms}ms (rebuild + search)"
done
echo ""

# --- Search benchmark (warm index — .index exists) ---
echo "## Search latency (warm — cached index)"
for n in 100 500 1000; do
    rm -rf "$COMMONPLACE_HOME"/*
    for i in $(seq 1 $n); do
        $CP write "topic-$((i % 10))" "entry $i about testing deployment debugging monitoring logging alerts" >/dev/null 2>&1
    done
    # Warm up: first search builds the index
    $CP search "warmup" >/dev/null 2>&1
    ms=$(time_ms "$CP search 'testing deployment'")
    echo "  $n entries: ${ms}ms"
done
echo ""

# --- Forget benchmark ---
echo "## Forget latency"
rm -rf "$COMMONPLACE_HOME"/*
for i in $(seq 1 1000); do
    $CP write "topic-0" "entry $i about testing and things" >/dev/null 2>&1
done
ms=$(time_ms "$CP forget topic-0 'entry 500'")
echo "  1000 entries (forget 1): ${ms}ms"
echo ""

# --- Binary size ---
echo "## Binary"
ls -lh "$(which $CP 2>/dev/null || echo $CP)" 2>/dev/null | awk '{print "  size:", $5}'
echo ""

echo "done."
