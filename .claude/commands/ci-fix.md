---
description: Diagnose and fix CI failures with local validation before re-pushing
---

CI failed. Follow this sequence strictly:

1. **Get logs**: `gh run list --limit 3` then `gh run view <id> --log-failed` to read the actual failure
2. **Classify**: What type of failure?
   - Format/lint → direct fix
   - Type error → locate and fix
   - Test failure → analyze if code bug or test needs update
   - YAML syntax → fix syntax
   - Environment issue → adjust CI config
3. **Fix**: Apply the minimal fix
4. **Validate locally**: Run the exact same check that failed in CI
5. **Push**: Only after local validation passes

CRITICAL: Do not guess at the fix. Read the actual CI log first.
CRITICAL: Do not push without local validation passing.
