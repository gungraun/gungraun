#!/usr/bin/env -S bash -ex

perf --version --build-options

cat /proc/sys/kernel/perf_event_paranoid

printf '\nAvailable PMUs:\n'
find /sys/bus/event_source/devices -mindepth 1 -maxdepth 1 -printf '%f\n' | sort

lscpu | sed -n \
  -e '/^Architecture:/p' \
  -e '/^Model name:/p' \
  -e '/^CPU(s):/p' \
  -e '/^Hypervisor vendor:/p'

echo "::group::Available perf events"
perf list || {
  status=$?
  echo "::endgroup::"
  exit "$status"
}
echo "::endgroup::"
