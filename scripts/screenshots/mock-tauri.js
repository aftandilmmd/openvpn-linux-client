// Demo backend for README screenshots: replaces the Tauri bridge with fixed data.
(() => {
  const now = Math.floor(Date.now() / 1000);
  const profiles = [
    { path: "/net/openvpn/v3/configuration/a", name: "Office — Frankfurt", imported: now, last_used: now },
    { path: "/net/openvpn/v3/configuration/b", name: "Home Lab", imported: now, last_used: now },
    { path: "/net/openvpn/v3/configuration/c", name: "Istanbul Edge", imported: now, last_used: now },
  ];
  let bytesIn = 1_482_000_000, bytesOut = 214_000_000;
  const handlers = {
    backend_version: () => "v24",
    repo_url: () => "https://github.com/aftandilmmd/openvpn-linux-client",
    list_profiles: () => profiles,
    check_update: () => null,
    list_sessions: () => {
      bytesIn += 2_350_000; bytesOut += 410_000;
      return [{
        path: "/net/openvpn/v3/sessions/1", config_path: profiles[0].path, config_name: profiles[0].name,
        status: { major: 2, minor: 7, message: "" }, device: "tun0",
        server: { proto: "udp", ip: "203.0.113.42", port: 1194 },
        created: now - (2 * 3600 + 17 * 60 + 42),
        stats: { BYTES_IN: bytesIn, BYTES_OUT: bytesOut },
      }];
    },
  };
  window.__TAURI__ = {
    core: { invoke: async (cmd) => (handlers[cmd] ? handlers[cmd]() : null) },
    event: { listen: async () => () => {} },
  };
})();
