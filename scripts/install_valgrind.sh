#!/bin/bash -eux

# spell-checker: ignore waitretry connrefused

# This script is intended to be run on an ubuntu runner in the github ci

set -o pipefail

valgrind_version="${1:-3.26.0}"
ubuntu_version="ubuntu-$(lsb_release -r | awk '{print $2}')"
archive_name="valgrind-${valgrind_version}-x86_64-${ubuntu_version}.tar.gz"

# Don't install a newer libc6 version than the one that is already installed.
# Updating it without a restart might not be the safest thing to do. We just
# need the fitting libc6-dbg package which is required for example for the
# memcheck tool.
sudo apt-mark hold libc6
libc_version="$(dpkg-query -W -f='${Version}' libc6)"
sudo apt-get update

install_libc6_dbg() {
  sudo apt-get install --update --assume-yes --no-install-recommends \
    --no-upgrade "$@" "libc6-dbg=${libc_version}"
}

# Install the libc6-dbg package matching the installed (held) libc6 version.
# If that exact version has been superseded in and removed from the live
# archive, fall back to snapshot.ubuntu.com. Candidate snapshot dates are
# derived from the runner image build date ($ImageVersion, format
# 20260907.300.1): the runner image ships the libc6 version that was current
# in the archive at image build time, so the same-day snapshot usually has
# the matching version, while the next-day snapshot covers versions
# published after midnight of the build day. A pinned date stays as the last
# candidate for runners without a valid ImageVersion; it may need a manual
# bump for such environments (update policy: bump it when a no-ImageVersion
# environment fails to resolve libc6-dbg).
snapshot_dates=("20260908T000000Z")
if [[ "${ImageVersion:-}" =~ ^[0-9]{8}\. ]]; then
  image_date="${ImageVersion%%.*}"
  image_date_iso="${image_date:0:4}-${image_date:4:2}-${image_date:6:2}"
  next_date="$(date --utc --date "${image_date_iso} + 1 day" +%Y%m%d)"
  snapshot_dates=("${image_date}T000000Z" "${next_date}T000000Z" "${snapshot_dates[0]}")
elif [[ -n "${ImageVersion:-}" ]]; then
  echo "WARN: unexpected ImageVersion '${ImageVersion}', using pinned snapshot date only" >&2
fi

libc6_dbg_installed=false
if install_libc6_dbg; then
  libc6_dbg_installed=true
else
  for snapshot_date in "${snapshot_dates[@]}"; do
    if install_libc6_dbg --snapshot "${snapshot_date}"; then
      libc6_dbg_installed=true
      break
    fi
    echo "WARN: libc6-dbg=${libc_version} not found in snapshot ${snapshot_date}, trying next candidate" >&2
  done
fi

if [[ "${libc6_dbg_installed}" == false ]]; then
  echo "ERROR: unable to install libc6-dbg=${libc_version} from the live archive or snapshots: ${snapshot_dates[*]}" >&2
  exit 1
fi

base_url="https://github.com/gungraun/valgrind-builder/releases/latest/download"
curl --fail --location --retry 10 --retry-connrefused --retry-all-errors \
  --output "${archive_name}" \
  "${base_url}/${archive_name}"
curl --fail --location --retry 10 --retry-connrefused --retry-all-errors \
  --output "${archive_name}.sha256" \
  "${base_url}/${archive_name}.sha256"
sha256sum -c "${archive_name}.sha256"

sudo tar xzf "$archive_name" -C /
