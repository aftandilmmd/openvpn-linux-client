#!/bin/bash
# Runs inside the e2e container: PKI, test server with user/pass auth, then the probe.
set -euo pipefail
W=/tmp/e2e; mkdir -p $W && cd $W
export EASYRSA_BATCH=1 EASYRSA_REQ_CN=test
cp -r /usr/share/easy-rsa pki-tool && cd pki-tool
./easyrsa init-pki >/dev/null && ./easyrsa build-ca nopass >/dev/null 2>&1
./easyrsa build-server-full server nopass >/dev/null 2>&1
cd $W
cat > check.sh <<'S'
#!/bin/sh
[ "$(sed -n 1p "$1")" = "alice" ] && [ "$(sed -n 2p "$1")" = "s3cret" ]
S
chmod +x check.sh
cat > server.conf <<S
dev tun
proto udp
port 1194
server 10.8.0.0 255.255.255.0
ca $W/pki-tool/pki/ca.crt
cert $W/pki-tool/pki/issued/server.crt
key $W/pki-tool/pki/private/server.key
dh none
verify-client-cert none
username-as-common-name
script-security 2
auth-user-pass-verify $W/check.sh via-file
keepalive 10 60
S
openvpn --config server.conf --daemon --log $W/server.log
# Client profile with an external ca file, to exercise inlining.
cp pki-tool/pki/ca.crt client-ca.crt
cat > client.ovpn <<S
client
dev tun
proto udp
remote 127.0.0.1 1194
nobind
auth-user-pass
ca client-ca.crt
S
# CI runners already have a system bus; bare containers do not.
if [ ! -S /run/dbus/system_bus_socket ]; then mkdir -p /run/dbus && dbus-daemon --system --fork && sleep 1; fi
"${PROBE:-/src/src-tauri/target/debug/examples/probe}" $W/client.ovpn alice s3cret
