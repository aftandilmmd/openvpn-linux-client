<p align="center">
  <img src="docs/icon.png" width="128" height="128" alt="App icon">
</p>

<h1 align="center">Linux Client for OpenVPN</h1>

<p align="center">
  <strong>Flip one switch. You're private.</strong><br>
  A desktop client for OpenVPN 3 on Linux: import a profile, connect in one click.
</p>

<p align="center">
  <a href="https://github.com/aftandilmmd/openvpn-linux-client/releases/latest"><strong>Download</strong></a> · Ubuntu · Debian · Fedora · any distro (AppImage)
</p>

<p align="center">
  <img src="docs/hero.png" width="860" alt="Connected to a VPN, light mode">
</p>

## Features

- **One-click connect** — import an `.ovpn` file (button or drag and drop), then flip the switch.
- **Sign-in built in** — username and password, one-time codes and challenges, and web (SAML/SSO) sign-in that opens in your browser.
- **Live status** — connection time, server, download and upload speed, total traffic and interface.
- **Pause, resume, reconnect** and a live log you can copy.
- **Automatic updates** — when a new version is out, a banner appears in the app. One click downloads, verifies and installs it.
- **Light and dark mode**, following your desktop.

<p align="center">
  <img src="docs/hero-dark.png" width="860" alt="Dark mode">
</p>

It uses **openvpn3-linux**, OpenVPN's own Linux client, and talks to its services directly. Tunnels run in those services, not in the window: closing or updating the app keeps you connected, and connecting doesn't need an admin password.

## Installing

**1. Install openvpn3** (once). Follow OpenVPN's [official instructions](https://community.openvpn.net/openvpn/wiki/OpenVPN3Linux), or on Ubuntu / Debian:

```sh
sudo mkdir -p /etc/apt/keyrings
curl -fsSL https://packages.openvpn.net/packages-repo.gpg | sudo tee /etc/apt/keyrings/openvpn.asc >/dev/null
echo "deb [signed-by=/etc/apt/keyrings/openvpn.asc] https://packages.openvpn.net/openvpn3/debian $(. /etc/os-release && echo $VERSION_CODENAME) main" \
  | sudo tee /etc/apt/sources.list.d/openvpn3.list
sudo apt update && sudo apt install openvpn3
```

On Fedora: `sudo dnf copr enable dsommers/openvpn3 && sudo dnf install openvpn3-client`.

**2. Install the app** from the [latest release](https://github.com/aftandilmmd/openvpn-linux-client/releases/latest):

- **Ubuntu / Debian:** `sudo apt install ./Linux*_amd64.deb` (`arm64` on ARM)
- **Fedora / openSUSE:** `sudo dnf install ./Linux*.x86_64.rpm`
- **Any distribution:** the `.AppImage`. Run `chmod +x Linux*.AppImage`, then start it.

Open the app, click **+** and choose your `.ovpn` file. Profiles you already imported with the `openvpn3` command line show up automatically.

## Updates

The app checks GitHub for new releases on startup and every few hours. Updates are signed, and the app refuses any file that doesn't match its key. The AppImage updates itself. For the `.deb` and `.rpm` packages your system asks for your password once, because they are installed system-wide.

## FAQ

**Is this an official OpenVPN app?**
No. It's an independent app built on OpenVPN's official open-source Linux client, openvpn3-linux.

**My profile refers to separate `ca.crt` / `client.key` files.**
That's fine. Keep them next to the `.ovpn` file when importing and the app embeds them into the profile.

**It says openvpn3-linux is not available.**
Install the `openvpn3` package (step 1) and reopen the app.

## Building from source

Requirements: Rust, Node.js 20, and the Tauri Linux dependencies (`libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev`).

```sh
npm ci
npx tauri dev                     # run
npx tauri build                   # .deb, .rpm and .AppImage in src-tauri/target/release/bundle
```

From macOS or Windows, `docker/Dockerfile` provides the same build environment. `scripts/e2e/` holds an end-to-end test that imports a profile, signs in, connects to a local OpenVPN server and disconnects. CI runs it on every push.

Publishing a GitHub release builds both architectures and attaches the packages and the `latest.json` update manifest. Release builds need the `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repository secrets.

---

<sub>This project is not affiliated with OpenVPN Inc. OpenVPN is a registered trademark of OpenVPN Inc.</sub>
