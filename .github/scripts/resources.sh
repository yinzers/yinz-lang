#!/usr/bin/env bash
# Resource evidence for a CI job: is it CPU, disk or locks?
#   resources.sh start    print the runner shape and PSI counters, then sample in the background
#   resources.sh summary  stop sampling; print PSI counters, vmstat averages and the peak /tmp use
#
# PSI (/proc/pressure/*) holds cumulative stall microseconds: cpu "some" is time a runnable
# task waited for a core, io "some" is time a task waited on disk. vmstat columns (-w, no
# header skew): r b swpd free buff cache si so bi bo in cs us sy id wa st.
set -euo pipefail

sample_dir="${RUNNER_TEMP:?RUNNER_TEMP is set by GitHub Actions}/resources"
interval_seconds=5

print_pressure() {
  grep -H . /proc/pressure/cpu /proc/pressure/io /proc/pressure/memory || echo "PSI unavailable"
}

start() {
  mkdir -p "${sample_dir}"
  echo "nproc=$(nproc)"
  free -m
  df -h / /tmp
  mount | grep -E ' / | /tmp ' || true
  echo "--- PSI at start"
  print_pressure
  nohup vmstat -w -n "${interval_seconds}" > "${sample_dir}/vmstat.log" 2>&1 &
  echo "$!" > "${sample_dir}/vmstat.pid"
  # /tmp use in KiB, one line per sample: sizes the tmpfs for the peak in-flight scratch output.
  nohup bash -c "while true; do df --output=used /tmp | tail -1; sleep ${interval_seconds}; done" \
    > "${sample_dir}/tmp-used-kib.log" 2>&1 &
  echo "$!" > "${sample_dir}/tmp-sampler.pid"
}

summary() {
  echo "--- PSI at end"
  print_pressure
  kill "$(cat "${sample_dir}/vmstat.pid")" "$(cat "${sample_dir}/tmp-sampler.pid")" 2> /dev/null || true
  awk 'NR>3 { r+=$1; b+=$2; us+=$13; sy+=$14; id+=$15; wa+=$16; n++ }
       END { if (n) printf "vmstat samples=%d avg_runnable=%.1f avg_blocked=%.1f us=%.1f sy=%.1f id=%.1f wa=%.1f\n", n, r/n, b/n, us/n, sy/n, id/n, wa/n }' \
    "${sample_dir}/vmstat.log"
  sort -n "${sample_dir}/tmp-used-kib.log" | tail -1 | awk '{ printf "peak /tmp use = %.0f MiB\n", $1/1024 }'
  echo "--- vmstat.log (every ${interval_seconds} s)"
  cat "${sample_dir}/vmstat.log"
}

case "${1:-}" in
  start) start ;;
  summary) summary ;;
  *) echo "usage: resources.sh start|summary" >&2; exit 2 ;;
esac
