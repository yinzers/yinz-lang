#!/usr/bin/env bash
# Resource evidence for a CI job: is it CPU, disk or locks?
#   resources.sh start    print the runner shape and PSI counters, then sample in the background
#   resources.sh summary  stop sampling; print the PSI stall seconds, vmstat averages and peak
#                         /tmp use, and repeat them as one notice annotation (the check-run API
#                         serves annotations without the full log)
#
# PSI (/proc/pressure/*) holds cumulative stall microseconds: cpu "some" is time a runnable
# task waited for a core, io "some" is time a task waited on disk. vmstat columns (-w):
# r b swpd free buff cache si so bi bo in cs us sy id wa st gu.
set -euo pipefail

sample_dir="${RUNNER_TEMP:?RUNNER_TEMP is set by GitHub Actions}/resources"
interval_seconds=5

print_pressure() {
  grep -H . /proc/pressure/cpu /proc/pressure/io /proc/pressure/memory || echo "PSI unavailable"
}

# Cumulative stall microseconds of one PSI file's `some` line.
stall_microseconds() {
  awk '/^some/ { sub("total=", "", $5); print $5 }' "/proc/pressure/$1"
}

start() {
  mkdir -p "${sample_dir}"
  echo "nproc=$(nproc)"
  free -m
  df -h / /tmp
  mount | grep -E ' / | /tmp ' || true
  echo "--- PSI at start"
  print_pressure
  stall_microseconds cpu > "${sample_dir}/psi-cpu-start"
  stall_microseconds io > "${sample_dir}/psi-io-start"
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

  local cpu_stall_seconds io_stall_seconds vmstat_averages peak_tmp_mib
  cpu_stall_seconds="$(awk -v start="$(cat "${sample_dir}/psi-cpu-start")" -v end="$(stall_microseconds cpu)" 'BEGIN { printf "%.0f", (end - start) / 1e6 }')"
  io_stall_seconds="$(awk -v start="$(cat "${sample_dir}/psi-io-start")" -v end="$(stall_microseconds io)" 'BEGIN { printf "%.0f", (end - start) / 1e6 }')"
  vmstat_averages="$(awk 'NR>3 { r+=$1; b+=$2; us+=$13; sy+=$14; id+=$15; wa+=$16; n++ }
    END { if (n) printf "vmstat samples=%d avg_runnable=%.1f avg_blocked=%.1f us=%.1f sy=%.1f id=%.1f wa=%.1f", n, r/n, b/n, us/n, sy/n, id/n, wa/n }' \
    "${sample_dir}/vmstat.log")"
  peak_tmp_mib="$(sort -n "${sample_dir}/tmp-used-kib.log" | tail -1 | awk '{ printf "%.0f", $1/1024 }')"

  local line="PSI stall: cpu ${cpu_stall_seconds} s, io ${io_stall_seconds} s; ${vmstat_averages}; peak /tmp use ${peak_tmp_mib} MiB"
  echo "RESOURCES: ${line}"
  echo "::notice title=resources::${line}"
  echo "--- vmstat.log (every ${interval_seconds} s)"
  cat "${sample_dir}/vmstat.log"
}

case "${1:-}" in
  start) start ;;
  summary) summary ;;
  *) echo "usage: resources.sh start|summary" >&2; exit 2 ;;
esac
