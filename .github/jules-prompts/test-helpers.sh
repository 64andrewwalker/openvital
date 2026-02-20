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

assert_contains() {
  local label="$1" needle="$2" haystack="$3"
  if [[ "$haystack" == *"$needle"* ]]; then
    echo "  PASS: $label"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label — expected to contain '$needle', got '$haystack'"
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

# ─── Test 1: Successful API call (HTTP 200) ──────────────────────────────────
echo "Test 1: Successful API call (HTTP 200)"
export MOCK_HTTP_CODE=200
export MOCK_RESPONSE='{"id":"session-1"}'
OUTPUT=$(jules_create_session "$TMPFILE" "test title" "main" 2>/dev/null)
EXIT_CODE=$?
assert_eq "exit code" "0" "$EXIT_CODE"
assert_eq "returns body" '{"id":"session-1"}' "$OUTPUT"

# ─── Test 2: Double failure (HTTP 500) returns non-zero exit ──────────────────
echo ""
echo "Test 2: Double failure (HTTP 500) returns non-zero exit"
export MOCK_HTTP_CODE=500
export MOCK_RESPONSE='{"error":"server error"}'
EXIT_CODE=0
OUTPUT=$(jules_create_session "$TMPFILE" "test title" "main" 2>/dev/null) || EXIT_CODE=$?
assert_eq "exit code non-zero on failure" "1" "$EXIT_CODE"

# ─── Test 3: No automationMode when 4th arg is empty ─────────────────────────
echo ""
echo "Test 3: No automationMode when 4th arg is empty"
export MOCK_HTTP_CODE=200
export MOCK_RESPONSE='{"id":"session-2"}'
# Capture payload by overriding curl to also write args to a file
PAYLOAD_FILE=$(mktemp)
curl() {
  # Capture the -d argument (last arg)
  local args=("$@")
  for i in "${!args[@]}"; do
    if [[ "${args[$i]}" == "-d" ]]; then
      echo "${args[$((i+1))]}" > "$PAYLOAD_FILE"
    fi
  done
  echo "$MOCK_RESPONSE"
  echo "$MOCK_HTTP_CODE"
}
export PAYLOAD_FILE
export -f curl
OUTPUT=$(jules_create_session "$TMPFILE" "test title" "main" "" 2>/dev/null)
PAYLOAD_CONTENT=$(cat "$PAYLOAD_FILE")
if echo "$PAYLOAD_CONTENT" | jq -e '.automationMode' >/dev/null 2>&1; then
  echo "  FAIL: payload should NOT contain automationMode when 4th arg is empty"
  FAIL=$((FAIL + 1))
else
  echo "  PASS: no automationMode in payload"
  PASS=$((PASS + 1))
fi

# ─── Test 4: automationMode present when 4th arg is set ──────────────────────
echo ""
echo "Test 4: automationMode present when 4th arg is set"
OUTPUT=$(jules_create_session "$TMPFILE" "test title" "main" "AUTO_CREATE_PR" 2>/dev/null)
PAYLOAD_CONTENT=$(cat "$PAYLOAD_FILE")
MODE=$(echo "$PAYLOAD_CONTENT" | jq -r '.automationMode')
assert_eq "automationMode value" "AUTO_CREATE_PR" "$MODE"

# ─── Test 5: 4xx errors fail immediately (no retry) ──────────────────────────
echo ""
echo "Test 5: 4xx errors fail immediately (no retry)"
CALL_COUNT_FILE=$(mktemp)
echo "0" > "$CALL_COUNT_FILE"
curl() {
  local count
  count=$(cat "$CALL_COUNT_FILE")
  count=$((count + 1))
  echo "$count" > "$CALL_COUNT_FILE"
  echo "$MOCK_RESPONSE"
  echo "$MOCK_HTTP_CODE"
}
export CALL_COUNT_FILE
export -f curl
export MOCK_HTTP_CODE=401
export MOCK_RESPONSE='{"error":"unauthorized"}'
EXIT_CODE=0
OUTPUT=$(jules_create_session "$TMPFILE" "test title" "main" 2>/dev/null) || EXIT_CODE=$?
CALL_COUNT=$(cat "$CALL_COUNT_FILE")
assert_eq "exit code non-zero on 401" "1" "$EXIT_CODE"
assert_eq "curl called only once for 4xx" "1" "$CALL_COUNT"
rm -f "$CALL_COUNT_FILE"

# ─── Test 6: 5xx errors DO retry ─────────────────────────────────────────────
echo ""
echo "Test 6: 5xx errors DO retry"
CALL_COUNT_FILE=$(mktemp)
echo "0" > "$CALL_COUNT_FILE"
curl() {
  local count
  count=$(cat "$CALL_COUNT_FILE")
  count=$((count + 1))
  echo "$count" > "$CALL_COUNT_FILE"
  echo "$MOCK_RESPONSE"
  echo "$MOCK_HTTP_CODE"
}
export CALL_COUNT_FILE
export -f curl
export MOCK_HTTP_CODE=503
export MOCK_RESPONSE='{"error":"service unavailable"}'
EXIT_CODE=0
OUTPUT=$(jules_create_session "$TMPFILE" "test title" "main" 2>/dev/null) || EXIT_CODE=$?
CALL_COUNT=$(cat "$CALL_COUNT_FILE")
assert_eq "exit code non-zero on double 503" "1" "$EXIT_CODE"
assert_eq "curl called twice for 5xx" "2" "$CALL_COUNT"
rm -f "$CALL_COUNT_FILE"

# ─── Test 7: Missing prompt file gives clear error ────────────────────────────
echo ""
echo "Test 7: Missing prompt file gives clear error"
# Reset curl mock
curl() {
  echo "$MOCK_RESPONSE"
  echo "$MOCK_HTTP_CODE"
}
export -f curl
export MOCK_HTTP_CODE=200
export MOCK_RESPONSE='{"id":"session-1"}'
EXIT_CODE=0
OUTPUT=$(jules_create_session "/nonexistent/file.txt" "test title" "main" 2>&1) || EXIT_CODE=$?
assert_eq "exit code non-zero for missing file" "1" "$EXIT_CODE"
assert_contains "error mentions file" "not found" "$OUTPUT"

# ─── Test 8: 429 (rate limit) retries like 5xx ───────────────────────────────
echo ""
echo "Test 8: 429 (rate limit) retries like 5xx"
CALL_COUNT_FILE=$(mktemp)
echo "0" > "$CALL_COUNT_FILE"
curl() {
  local count
  count=$(cat "$CALL_COUNT_FILE")
  count=$((count + 1))
  echo "$count" > "$CALL_COUNT_FILE"
  echo "$MOCK_RESPONSE"
  echo "$MOCK_HTTP_CODE"
}
export CALL_COUNT_FILE
export -f curl
export MOCK_HTTP_CODE=429
export MOCK_RESPONSE='{"error":"rate limited"}'
EXIT_CODE=0
OUTPUT=$(jules_create_session "$TMPFILE" "test title" "main" 2>/dev/null) || EXIT_CODE=$?
CALL_COUNT=$(cat "$CALL_COUNT_FILE")
assert_eq "exit code non-zero on double 429" "1" "$EXIT_CODE"
assert_eq "curl called twice for 429" "2" "$CALL_COUNT"
rm -f "$CALL_COUNT_FILE"

rm -f "$TMPFILE" "$PAYLOAD_FILE"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]]
