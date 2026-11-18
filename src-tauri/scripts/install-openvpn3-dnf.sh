#!/bin/sh
# Installs openvpn3 on Fedora from the openvpn3 maintainer's Copr repository,
# as listed in OpenVPN's installation guide. Runs as root.
set -eu

command -v dnf >/dev/null || { echo "dnf not found." >&2; exit 4; }

# Weak dependencies would add the optional kernel acceleration module and the
# toolchain to build it; the client does not need them.
dnf_() { dnf -y -q --setopt=install_weak_deps=False --setopt=retries=5 "$@"; }

dnf_ install 'dnf-command(copr)'
dnf_ copr enable dsommers/openvpn3
if ! dnf_ install openvpn3-client; then
  echo "Retrying with fresh metadata..." >&2
  dnf -y -q clean metadata
  dnf_ install openvpn3-client
fi

dbus-send --system --type=method_call --dest=org.freedesktop.DBus \
  / org.freedesktop.DBus.ReloadConfig 2>/dev/null || true
