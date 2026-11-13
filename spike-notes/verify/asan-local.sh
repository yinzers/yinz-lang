#!/bin/sh
# usage: asan.sh file.ynz  (run from .spike-env/p)
. /worktrees/3d64df98-866b-483f-bb8a-5f0fdaf4e998/.spike-env/env.sh
b=${1%.ynz}
PATH=$ASAN_BIN:$PATH ynz build $1 2>&1 | grep -v "^Build succeeded" | head -30
rm -f $b.cnt
YNZ_ALLOC_COUNTER=1 YNZ_ALLOC_COUNTER_OUTPUT=$PWD/$b.cnt ASAN_OPTIONS=detect_leaks=1:symbolize=0:max_free_fill_size=65536:free_fill_byte=0 ./$b 2>&1 | grep -E "^[0-9a-z]|ERROR|SUMMARY|leak" | head -30
grep -E "alloc=|free=" $b.cnt 2>/dev/null | head -2 | tr '\n' ' '; echo
