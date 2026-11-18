//! Installs openvpn3-linux when it is missing, so users only install this app.

use std::process::Command;

use serde::Serialize;

const APT_SCRIPT: &str = include_str!("../scripts/install-openvpn3-apt.sh");
const DNF_SCRIPT: &str = include_str!("../scripts/install-openvpn3-dnf.sh");
pub const HELP_URL: &str = "https://community.openvpn.net/openvpn/wiki/OpenVPN3Linux";

#[derive(Debug, PartialEq)]
enum Method {
    Apt { codename: String },
    Dnf,
}

#[derive(Serialize)]
pub struct SetupInfo {
    /// Whether the app can install openvpn3 itself on this distribution.
    pub automatic: bool,
    pub distro: String,
    pub help_url: &'static str,
}

fn os_release_field(text: &str, key: &str) -> String {
    text.lines()
        .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
        .map(|v| v.trim().trim_matches('"').to_string())
        .unwrap_or_default()
}

fn detect(os_release: &str) -> Option<Method> {
    let id = os_release_field(os_release, "ID");
    let like = os_release_field(os_release, "ID_LIKE");
    let family: Vec<&str> = id.split_whitespace().chain(like.split_whitespace()).collect();

    if family.iter().any(|f| *f == "debian" || *f == "ubuntu") {
        // Ubuntu derivatives (Mint, Pop!_OS, ...) name their Ubuntu base here.
        let mut codename = os_release_field(os_release, "UBUNTU_CODENAME");
        if codename.is_empty() {
            codename = os_release_field(os_release, "VERSION_CODENAME");
        }
        let valid = !codename.is_empty()
            && codename.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        return valid.then_some(Method::Apt { codename });
    }
    if id == "fedora" {
        return Some(Method::Dnf);
    }
    None
}

fn read_os_release() -> String {
    std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
        .unwrap_or_default()
}

pub fn info() -> SetupInfo {
    let text = read_os_release();
    let name = os_release_field(&text, "PRETTY_NAME");
    SetupInfo {
        automatic: detect(&text).is_some(),
        distro: if name.is_empty() { "this system".into() } else { name },
        help_url: HELP_URL,
    }
}

/// Runs the install script as root through pkexec, which shows the system's
/// password prompt. Blocks until the installation is finished.
pub fn install() -> Result<(), String> {
    let method = detect(&read_os_release())
        .ok_or("Automatic installation is not available for this distribution.")?;
    let (script, arg) = match method {
        Method::Apt { codename } => (APT_SCRIPT, codename),
        Method::Dnf => (DNF_SCRIPT, String::new()),
    };

    let out = Command::new("pkexec")
        .args(["/bin/sh", "-c", script, "install-openvpn3", &arg])
        .output()
        .map_err(|e| format!("Could not ask for administrator rights (pkexec): {e}"))?;

    match out.status.code() {
        Some(0) => Ok(()),
        // pkexec: 126 = password dialog dismissed, 127 = not authorized
        Some(126) | Some(127) => Err("Installation was cancelled.".into()),
        _ => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let tail: Vec<&str> = stderr.lines().rev().take(6).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            Err(format!("Installing OpenVPN 3 failed.\n{}", tail.join("\n")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ubuntu_uses_its_codename() {
        let os = "ID=ubuntu\nID_LIKE=debian\nVERSION_CODENAME=noble\nUBUNTU_CODENAME=noble\n";
        assert_eq!(detect(os), Some(Method::Apt { codename: "noble".into() }));
    }

    #[test]
    fn mint_uses_the_ubuntu_base() {
        let os = "ID=linuxmint\nID_LIKE=\"ubuntu debian\"\nVERSION_CODENAME=wilma\nUBUNTU_CODENAME=noble\n";
        assert_eq!(detect(os), Some(Method::Apt { codename: "noble".into() }));
    }

    #[test]
    fn debian_and_fedora() {
        assert_eq!(
            detect("ID=debian\nVERSION_CODENAME=bookworm\n"),
            Some(Method::Apt { codename: "bookworm".into() })
        );
        assert_eq!(detect("ID=fedora\nVERSION_ID=41\n"), Some(Method::Dnf));
    }

    #[test]
    fn others_are_manual() {
        assert_eq!(detect("ID=arch\n"), None);
        assert_eq!(detect("ID=kali\nID_LIKE=debian\nVERSION_CODENAME=kali-rolling\n"), None);
    }
}
