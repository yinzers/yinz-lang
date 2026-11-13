#!/bin/sh
# usage: asan.sh file.ynz -> build with ASan wrapper linker, run with LSan + free-fill, print alloc/free counters
. /worktrees/b416b0c7-5971-4c4b-97d0-725d827e4f1f/.spike-env/env.sh
b=${1%.ynz}
PATH=$ASAN_BIN:$PATH ynz build $1 2>&1 | grep -v "^Build succeeded" | head -30
rm -f /tmp/p/$b.cnt
YNZ_ALLOC_COUNTER=1 YNZ_ALLOC_COUNTER_OUTPUT=/tmp/p/$b.cnt ASAN_OPTIONS=detect_leaks=1:symbolize=0:max_free_fill_size=65536:free_fill_byte=0 ./$b 2>&1 | grep -E "^[0-9a-z]|ERROR|SUMMARY|leak" | head -30
grep -E "alloc=|free=" /tmp/p/$b.cnt 2>/dev/null | head -2 | tr '\n' ' '; echo
