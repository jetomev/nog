use std::process::{Command, ExitStatus, Output};

/// One pending upgrade reported by `checkupdates`.
#[derive(Debug, Clone)]
pub struct PendingUpdate {
    pub name: String,
    pub old_version: String,
    pub new_version: String,
}

/// Invoke pacman through sudo so `nog` itself never needs to run as root.
/// sudo prompts for the user's password the first time and caches the session
/// timestamp; subsequent calls within the cache window are pass-through. If
/// the caller is already root (e.g. legacy `sudo nog install`), sudo is a
/// no-op, so this is fully backwards-compatible.
pub fn run(args: &[&str]) -> ExitStatus {
    Command::new("sudo")
        .arg("pacman")
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("nog: failed to launch sudo pacman: {}", e))
}

/// Run `checkupdates` (from pacman-contrib) and return the list of pending
/// upgrades. We prefer this over `pacman -Qu` because checkupdates syncs to a
/// temporary DB, so it doesn't touch the system's real /var/lib/pacman/sync
/// state — no surprise `-Sy` side effect before we've decided anything.
///
/// Errors distinguish two cases:
///   - `Err(Missing)` — the binary isn't installed. Actionable: the user needs
///     pacman-contrib. Caller should print a pointer.
///   - `Err(Other(msg))` — some other failure (network, DB corruption, etc.).
pub enum CheckUpdatesError {
    Missing,
    Other(String),
}

pub fn checkupdates_capture() -> Result<Vec<PendingUpdate>, CheckUpdatesError> {
    let output = match Command::new("checkupdates").output() {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(CheckUpdatesError::Missing);
        }
        Err(e) => return Err(CheckUpdatesError::Other(format!("{}", e))),
    };

    // checkupdates conventions:
    //   exit 0 — updates available (stdout has lines)
    //   exit 2 — no updates available (stdout empty)
    //   exit 1 — some other failure; stderr has the reason
    if let Some(code) = output.status.code() {
        if code == 2 {
            return Ok(Vec::new());
        }
        if code != 0 {
            let msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(CheckUpdatesError::Other(if msg.is_empty() {
                format!("checkupdates exited with status {}", code)
            } else {
                msg
            }));
        }
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut updates = Vec::new();
    // Line format: "pkgname oldver -> newver"
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let name = match parts.next() {
            Some(s) => s.to_string(),
            None => continue,
        };
        let old_version = parts.next().unwrap_or("").to_string();
        let _arrow = parts.next(); // "->"
        let new_version = parts.next().unwrap_or("").to_string();
        updates.push(PendingUpdate { name, old_version, new_version });
    }

    Ok(updates)
}

pub fn install(packages: &[String]) -> ExitStatus {
    let pkgs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
    let mut args = vec!["-S", "--noconfirm"];
    args.extend_from_slice(&pkgs);
    run(&args)
}

pub fn remove(packages: &[String]) -> ExitStatus {
    let pkgs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
    let mut args = vec!["-Rs", "--noconfirm"];
    args.extend_from_slice(&pkgs);
    run(&args)
}

pub fn update() -> ExitStatus {
    run(&["-Syu"])
}

pub fn update_excluding(excluded: &[String]) -> ExitStatus {
    if excluded.is_empty() {
        return run(&["-Syu"]);
    }
    let ignore_list = excluded.join(",");
    run(&["-Syu", "--ignore", &ignore_list])
}

pub fn search(query: &str) -> ExitStatus {
    run(&["-Ss", query])
}

pub fn search_capture(query: &str) -> Output {
    Command::new("pacman")
        .args(["-Ss", query])
        .output()
        .unwrap_or_else(|e| panic!("nog: failed to launch pacman: {}", e))
}