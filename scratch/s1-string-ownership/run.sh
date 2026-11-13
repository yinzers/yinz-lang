#!/bin/bash
# usage: run.sh <ynz-binary> <file.ynz>...   (compiles with ASan + scratch scope-exit free, runs, summarises)
# env: OPT=--no-optimize (default) or OPT= for optimized; FREE=1 (default) or 0 for control without free
YNZ=$1; shift
OPT=${OPT---no-optimize}; FREE=${FREE-1}
mkdir -p /tmp/s1bin
for f in "$@"; do
  name=$(basename "$f" .ynz)
  cp "$f" /tmp/s1bin/; out=/tmp/s1bin/$name; rm -f "$out"
  build=$(YNZ_SPIKE_FREE_STRINGS=$FREE YNZ_SPIKE_LINK_EXTRA="-fsanitize=address -g" "$YNZ" build $OPT "/tmp/s1bin/$name.ynz" 2>&1)
  if [ ! -x "$out" ]; then echo "== $name: BUILD FAIL"; echo "$build" | head -6; continue; fi
  res=$(ASAN_OPTIONS=detect_leaks=0 timeout 20 "$out" 2>&1); code=$?
  kind=$(echo "$res" | grep -m1 -o "ERROR: AddressSanitizer: .*" | cut -c1-90)
  echo "== $name: exit=$code ${kind:-CLEAN}"
  echo "$res" | grep -v "^==[0-9]*==\|^ *#\|^$\|^SUMMARY\|^ *0x\|^Shadow\|^  \|^=>\|AddressSanitizer\|^previously\|^freed by\|^0x\|^Address" | head -6 | sed 's/^/     out: /'
  echo "$res" | grep -m2 "^ *#[0-4] .* in \(ynz_\|main\|entrypoint\)" | sed 's/^/     /'
done
