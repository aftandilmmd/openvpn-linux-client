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

fetch() {
  if command -v curl >/dev/null; then curl -fsSL "$1"
  elif command -v wget >/dev/null; then wget -qO- "$1"
  else return 5
  fi
}

if ! command -v curl >/dev/null && ! command -v wget >/dev/null; then
  apt-get update -q
  DEBIAN_FRONTEND=noninteractive apt-get install -y -q ca-certificates curl
fi

if ! fetch "$repo/dists/$codename/Release" >/dev/null; then
  echo "OpenVPN does not publish openvpn3 packages for '$codename'." >&2
  exit 3
fi

mkdir -p /etc/apt/keyrings
fetch https://packages.openvpn.net/packages-repo.gpg > /etc/apt/keyrings/openvpn.asc
echo "deb [signed-by=/etc/apt/keyrings/openvpn.asc] $repo $codename main" \
  > /etc/apt/sources.list.d/openvpn3.list

apt-get update -q
DEBIAN_FRONTEND=noninteractive apt-get install -y -q openvpn3

# openvpn3 adds D-Bus policies for its new system user; make sure the bus
# has loaded them before the app talks to the services.
dbus-send --system --type=method_call --dest=org.freedesktop.DBus \
  / org.freedesktop.DBus.ReloadConfig 2>/dev/null || true
