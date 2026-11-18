# openvpn3-linux client + an OpenVPN 2 test server in one container.
FROM tunnelo-build
RUN apt-get update && apt-get install -y --no-install-recommends gnupg dbus openvpn easy-rsa iproute2 \
 && mkdir -p /etc/apt/keyrings \
 && curl -fsSL https://packages.openvpn.net/packages-repo.gpg > /etc/apt/keyrings/openvpn.asc \
 && echo "deb [signed-by=/etc/apt/keyrings/openvpn.asc] https://packages.openvpn.net/openvpn3/debian jammy main" > /etc/apt/sources.list.d/openvpn3.list \
 && apt-get update && apt-get install -y --no-install-recommends openvpn3 \
 && rm -rf /var/lib/apt/lists/*
