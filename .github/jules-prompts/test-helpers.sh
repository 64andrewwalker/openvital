#!/usr/bin/env bash
set -euo pipefail

# Test harness for helpers.sh — mocks curl to test retry/error logic
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PASS=0
FAIL=0

assert_eq() {
  local label="$1" expected="$2" actual="$3"
  if [[ "$expected" == "$actual" ]]; then
    echo "  PASS: $label"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label — expected '$expected', got '$actual'"
    FAIL=$((FAIL + 1))
  fi
}

# Mock curl: returns predefined response based on MOCK_HTTP_CODE env var
curl() {
  echo "$MOCK_RESPONSE"
  echo "$MOCK_HTTP_CODE"
}
export -f curl

# Mock sleep to avoid waiting 60s during tests
sleep() { :; }
export -f sleep

# Setup required env vars
export JULES_API_URL="https://test.example.com"
export JULES_API_KEY="test-key"
export JULES_SOURCE="sources/github/test/repo"

source "$SCRIPT_DIR/helpers.sh"

# Create a temp prompt file
TMPFILE=$(mktemp)
echo "test prompt content" > "$TMPFILE"

echo "Test 1: Successful API call (HTTP 200)"
export MOCK_HTTP_CODE=200
export MOCK_RESPONSE='{"id":"session-1"}'
OUTPUT=$(jules_create_session "$TMPFILE" "test title" "main" 2>/dev/null)
EXIT_CODE=$?
assert_eq "exit code" "0" "$EXIT_CODE"
assert_eq "returns body" '{"id":"session-1"}' "$OUTPUT"

echo ""
echo "Test 2: Double failure (HTTP 500) returns non-zero exit"
export MOCK_HTTP_CODE=500
export MOCK_RESPONSE='{"error":"server error"}'
EXIT_CODE=0
OUTPUT=$(jules_create_session "$TMPFILE" "test title" "main" 2>/dev/null) || EXIT_CODE=$?
assert_eq "exit code non-zero on failure" "1" "$EXIT_CODE"

rm -f "$TMPFILE"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]]
