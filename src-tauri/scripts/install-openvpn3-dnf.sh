#!/bin/sh
# Installs openvpn3 on Fedora from the openvpn3 maintainer's Copr repository,
# as listed in OpenVPN's installation guide. Runs as root.
set -eu

command -v dnf >/dev/null || { echo "dnf not found." >&2; exit 4; }

dnf -y -q install 'dnf-command(copr)'
dnf -y -q copr enable dsommers/openvpn3
dnf -y -q install openvpn3-client

dbus-send --system --type=method_call --dest=org.freedesktop.DBus \
  / org.freedesktop.DBus.ReloadConfig 2>/dev/null || true
