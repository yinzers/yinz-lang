#!/usr/bin/env bash
# Print the 10 slowest tests and the 10 slowest test binaries (summed test time) of one nextest
# run from its JUnit file, so the numbers sit in the job log, where the log reader can see them.
#   slowest-tests.sh <junit.xml>
set -euo pipefail

junit="${1:?usage: slowest-tests.sh <junit.xml>}"

testcases() {
  grep -o '<testcase name="[^"]*" classname="[^"]*"[^>]*time="[^"]*"' "${junit}"
}

echo "--- 10 slowest tests (seconds, binary, test)"
testcases \
  | sed -E 's/<testcase name="([^"]*)" classname="([^"]*)".*time="([^"]*)"/\3\t\2\t\1/' \
  | sort -t$'\t' -k1,1 -rn | awk 'NR<=10'

slowest="$(testcases | sed -E 's/<testcase name="([^"]*)" classname="([^"]*)".*time="([^"]*)"/\3 s \2 \1/' | sort -rn | awk 'NR==1')"
echo "::notice title=slowest test::${slowest}"

echo "--- 10 binaries by summed test time (seconds, tests, binary)"
testcases \
  | sed -E 's/<testcase name="([^"]*)" classname="([^"]*)".*time="([^"]*)"/\2\t\3/' \
  | awk -F'\t' '{ seconds[$1] += $2; tests[$1]++ } END { for (binary in seconds) printf "%.1f\t%d\t%s\n", seconds[binary], tests[binary], binary }' \
  | sort -rn | awk 'NR<=10'
