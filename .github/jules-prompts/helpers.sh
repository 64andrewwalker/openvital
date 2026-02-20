#!/usr/bin/env bash
set -euo pipefail

# jules_create_session — create a Jules API session with retry and error handling
#
# Usage: jules_create_session <prompt_file> <title> <branch> [automation_mode]
#
# Required env vars: JULES_API_URL, JULES_API_KEY, JULES_SOURCE
jules_create_session() {
  local PROMPT_FILE="$1" TITLE="$2" BRANCH="$3" MODE="${4-}"
  local PROMPT
  PROMPT=$(cat "$PROMPT_FILE")

  local PAYLOAD
  if [[ -n "$MODE" ]]; then
    PAYLOAD=$(jq -n \
      --arg title "$TITLE" \
      --arg prompt "$PROMPT" \
      --arg source "$JULES_SOURCE" \
      --arg branch "$BRANCH" \
      --arg mode "$MODE" \
      '{title: $title, prompt: $prompt, sourceContext: {source: $source, githubRepoContext: {startingBranch: $branch}}, automationMode: $mode}')
  else
    PAYLOAD=$(jq -n \
      --arg title "$TITLE" \
      --arg prompt "$PROMPT" \
      --arg source "$JULES_SOURCE" \
      --arg branch "$BRANCH" \
      '{title: $title, prompt: $prompt, sourceContext: {source: $source, githubRepoContext: {startingBranch: $branch}}}')
  fi

  local HTTP_CODE RESPONSE BODY
  RESPONSE=$(curl -s -w "\n%{http_code}" -X POST "$JULES_API_URL/sessions" \
    -H "Content-Type: application/json" \
    -H "X-Goog-Api-Key: $JULES_API_KEY" \
    -d "$PAYLOAD")
  HTTP_CODE=$(echo "$RESPONSE" | tail -1)
  BODY=$(echo "$RESPONSE" | sed '$d')

  if [[ "$HTTP_CODE" -ge 200 && "$HTTP_CODE" -lt 300 ]]; then
    echo "$BODY"
    return 0
  fi

  echo "::warning::Jules API returned HTTP $HTTP_CODE — retrying in 60s"
  sleep 60

  RESPONSE=$(curl -s -w "\n%{http_code}" -X POST "$JULES_API_URL/sessions" \
    -H "Content-Type: application/json" \
    -H "X-Goog-Api-Key: $JULES_API_KEY" \
    -d "$PAYLOAD")
  HTTP_CODE=$(echo "$RESPONSE" | tail -1)
  BODY=$(echo "$RESPONSE" | sed '$d')

  if [[ "$HTTP_CODE" -ge 200 && "$HTTP_CODE" -lt 300 ]]; then
    echo "$BODY"
    return 0
  fi

  echo "::error::Jules API failed after retry (HTTP $HTTP_CODE): $BODY"
  return 1
}
