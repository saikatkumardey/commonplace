#!/usr/bin/env bash
set -euo pipefail

CP="${1:-commonplace}"
export COMMONPLACE_HOME=$(mktemp -d)
trap "rm -rf $COMMONPLACE_HOME" EXIT

echo "commonplace recall benchmark"
echo "============================"
echo ""

# Seed realistic agent memories across topics
$CP write preferences "user prefers TDD workflow, always write tests first"
$CP write preferences "user wants concise responses, no filler or preamble"
$CP write preferences "user prefers Rust for CLI tools and Python for backend"
$CP write preferences "never add co-author lines to git commits"
$CP write preferences "always bump patch version after pushing changes"
$CP write preferences "user likes flat file storage over databases"

$CP write decisions "chose BM25 over vector embeddings for memory search"
$CP write decisions "picked commonplace book pattern over knowledge graphs"
$CP write decisions "using systemd for process management instead of supervisor"
$CP write decisions "went with python-telegram-bot over telethon for bot framework"
$CP write decisions "chose claude-agent-sdk over custom LLM wrapper"

$CP write errors "mock database tests passed but production migration failed silently"
$CP write errors "importlib.metadata throws PackageNotFoundError in uv tool installs"
$CP write errors "playwright browser context leaked when not properly closed"
$CP write errors "git tag already existed causing make release to fail"
$CP write errors "pre-commit hook blocked commit but amend would modify wrong commit"

$CP write context "indieclaw is a self-hosted Telegram AI agent written in Python"
$CP write context "Donna is the running instance of indieclaw"
$CP write context "RTK is a Rust token-saving CLI proxy for Claude Code"
$CP write context "user is building agent infrastructure and developer tools"
$CP write context "commonplace is a zero-dep Rust CLI for agent memory"

$CP write people "user is a senior engineer, deep Python and growing Rust expertise"
$CP write people "user values lean code, minimal dependencies, practical solutions"
$CP write people "user dislikes verbose AI responses and unnecessary abstractions"

passed=0
failed=0
total=0

check() {
    local query="$1"
    local expect="$2"      # substring that MUST appear in top result
    local desc="$3"
    total=$((total + 1))

    result=$($CP search "$query" --limit 1 2>&1)
    if echo "$result" | grep -qi "$expect"; then
        echo "  PASS: $desc"
        passed=$((passed + 1))
    else
        echo "  FAIL: $desc"
        echo "    query:    $query"
        echo "    expected: $expect"
        echo "    got:      $result"
        failed=$((failed + 1))
    fi
}

echo "## Exact recall — query matches stored terms"
check "TDD testing" "TDD" "direct keyword match"
check "vector embeddings" "BM25 over vector" "decision about search approach"
check "mock database production" "mock database" "known error recall"
check "Telegram bot" "Telegram" "project context"
check "co-author git commits" "co-author" "preference about commits"
echo ""

echo "## Partial recall — query uses related but different words"
check "test driven development" "TDD" "synonym for TDD"
check "which database to use" "flat file" "related concept (storage choice)"
check "how to deploy the bot" "systemd" "related concept (process management)"
check "what language for CLI" "Rust" "language preference"
check "version bumping" "bump patch" "related workflow"
echo ""

echo "## Cross-topic recall — answer could be in any topic"
check "what went wrong with tests" "mock" "error about testing"
check "who is Donna" "Donna" "context about the agent"
check "what does the user like" "concise" "user preference"
check "packaging issues" "importlib" "error about packaging"
check "memory architecture" "BM25" "decision about memory design"
echo ""

echo "## Negative / noise resistance"
check "quantum computing blockchain" "no results" "irrelevant query returns nothing"
echo ""

echo "## Ranking quality — right topic should be #1"
# Search with a query that could match multiple topics
result=$($CP search "testing workflow" --limit 3)
top_topic=$(echo "$result" | head -1 | grep -o '\[[^]]*\]' | tr -d '[]')
if [ "$top_topic" = "preferences" ]; then
    echo "  PASS: 'testing workflow' ranks preferences above errors"
    passed=$((passed + 1))
else
    echo "  FAIL: 'testing workflow' should rank preferences first, got: $top_topic"
    echo "    results: $result"
    failed=$((failed + 1))
fi
total=$((total + 1))

result=$($CP search "production failure" --limit 3)
top_topic=$(echo "$result" | head -1 | grep -o '\[[^]]*\]' | tr -d '[]')
if [ "$top_topic" = "errors" ]; then
    echo "  PASS: 'production failure' ranks errors first"
    passed=$((passed + 1))
else
    echo "  FAIL: 'production failure' should rank errors first, got: $top_topic"
    echo "    results: $result"
    failed=$((failed + 1))
fi
total=$((total + 1))
echo ""

echo "============================"
echo "Results: $passed/$total passed, $failed failed"
pct=$((passed * 100 / total))
echo "Recall accuracy: ${pct}%"
