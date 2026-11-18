#!/bin/sh
# Installs openvpn3 from OpenVPN's official repository on Debian, Ubuntu and
# derivatives. Runs as root (through pkexec from the app, or directly in CI).
# Usage: install-openvpn3-apt.sh <distribution codename>
set -eu

codename="${1:-}"
repo="https://packages.openvpn.net/openvpn3/debian"

case "$codename" in
  ''|*[!a-z0-9]*) echo "Unknown distribution codename '$codename'." >&2; exit 2 ;;
esac
command -v apt-get >/dev/null || { echo "apt-get not found." >&2; exit 4; }

export DEBIAN_FRONTEND=noninteractive
# Retry flaky downloads; mirrors are sometimes caught mid-sync.
apt_get() { apt-get -q -o Acquire::Retries=3 "$@"; }

fetch() {
  if command -v curl >/dev/null; then curl -fsSL "$1"
  elif command -v wget >/dev/null; then wget -qO- "$1"
  else return 5
  fi
}

if ! command -v curl >/dev/null && ! command -v wget >/dev/null; then
  apt_get update
  apt_get install -y --no-install-recommends ca-certificates curl
fi

if ! fetch "$repo/dists/$codename/Release" >/dev/null; then
  echo "OpenVPN does not publish openvpn3 packages for '$codename'." >&2
  exit 3
fi

mkdir -p /etc/apt/keyrings
fetch https://packages.openvpn.net/packages-repo.gpg > /etc/apt/keyrings/openvpn.asc
echo "deb [signed-by=/etc/apt/keyrings/openvpn.asc] $repo $codename main" \
  > /etc/apt/sources.list.d/openvpn3.list

# openvpn3-client is the VPN client itself. Its only recommendation is the
# optional kernel acceleration module (openvpn-dco-dkms), which would pull in
# dkms, compilers and kernel headers to build it locally; skip that.
install_client() {
  apt_get update
  apt_get install -y --no-install-recommends openvpn3-client
}
if ! install_client; then
  echo "Retrying with fresh package lists..." >&2
  sleep 5
  install_client
fi

# openvpn3 adds D-Bus policies for its new system user; make sure the bus
# has loaded them before the app talks to the services.
dbus-send --system --type=method_call --dest=org.freedesktop.DBus \
  / org.freedesktop.DBus.ReloadConfig 2>/dev/null || true
