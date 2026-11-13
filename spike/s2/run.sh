#!/bin/bash
# usage: run.sh file.ynz  -> prints compile verdict, output, exit code, alloc/free counts, frame-slot GEPs
export LD_LIBRARY_PATH=/tmp/shim
YNZ=${YNZ:-$(dirname "$0")/../../target/debug/ynz}
f=$1
cnt=$(mktemp)
echo "##### $f"
YNZ_ALLOC_COUNTER=1 YNZ_ALLOC_COUNTER_OUTPUT=$cnt CLICOLOR=0 timeout 20 "$YNZ" run "$f" 2>&1 | head -${LINES_MAX:-25}
echo "exit=${PIPESTATUS[0]}  $(tr '\n' ' ' < $cnt)"
rm -f $cnt
