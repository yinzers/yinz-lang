#!/usr/bin/env bash
# Sum the `test result:` lines of one or more `cargo test` / `cargo miri test` logs, so the job
# log states how many tests a sanitizer lane ran (card #681: same test set as before).
#   sum-test-results.sh <log>...
set -euo pipefail

if [ "$#" -lt 1 ]; then
  echo "usage: sum-test-results.sh <log>..." >&2
  exit 2
fi

cat "$@" | awk '
  /^test result:/ {
    for (field = 1; field <= NF; field++) {
      if ($field == "passed;") passed += $(field - 1)
      if ($field == "failed;") failed += $(field - 1)
      if ($field == "ignored;") ignored += $(field - 1)
    }
    suites++
  }
  END { printf "TEST TOTALS: %d passed, %d failed, %d ignored across %d test suites\n", passed, failed, ignored, suites }'
