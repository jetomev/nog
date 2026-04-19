use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
struct TierConfig {
    manual_signoff: bool,
    packages: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct TierPins {
    tier1: TierConfig,
    tier2: TierConfig,
    tier3: TierConfig,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Tier {
    One,
    Two,
    Three,
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Tier::One   => write!(f, "Tier 1"),
            Tier::Two   => write!(f, "Tier 2"),
            Tier::Three => write!(f, "Tier 3"),
        }
    }
}

pub struct TierManager {
    pins: TierPins,
}

impl TierManager {
    pub fn load(path: &str) -> Result<Self, String> {
        let contents = fs::read_to_string(path)
            .map_err(|e| format!("Could not read {}: {}", path, e))?;
        let pins: TierPins = toml::from_str(&contents)
            .map_err(|e| format!("Could not parse tier-pins.toml: {}", e))?;
        Ok(TierManager { pins })
    }

    pub fn classify(&self, package: &str) -> Tier {
        if let Some(pkgs) = &self.pins.tier1.packages {
            if pkgs.iter().any(|p| p == package) {
                return Tier::One;
            }
        }
        if let Some(pkgs) = &self.pins.tier2.packages {
            if pkgs.iter().any(|p| p == package) {
                return Tier::Two;
            }
        }
        Tier::Three
    }

    pub fn is_manual_signoff(&self, package: &str) -> bool {
        self.classify(package) == Tier::One && self.pins.tier1.manual_signoff
    }
}

pub fn pin_package(path: &str, package: &str, tier: u8) -> Result<(), String> {
    let contents = fs::read_to_string(path)
        .map_err(|e| format!("Could not read {}: {}", path, e))?;

    // Remove the package from all tiers first
    let mut lines: Vec<String> = contents
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed != format!("\"{}\"", package).as_str()
                && trimmed != format!("\"{}\",", package).as_str()
        })
        .map(|l| l.to_string())
        .collect();

    // Tier 3 is the default — just removing from other tiers is enough
    if tier == 3 {
        let new_contents = lines.join("\n") + "\n";
        return write_as_root(path, &new_contents);
    }

    // Find the right tier section and add the package
    let section = match tier {
        1 => "[tier1]",
        2 => "[tier2]",
        _ => return Err(format!("Invalid tier: {}. Must be 1, 2, or 3.", tier)),
    };

    // Find the packages = [ line within the correct section
    let mut in_section = false;
    let mut inserted = false;
    for i in 0..lines.len() {
        if lines[i].trim() == section {
            in_section = true;
        }
        if in_section && lines[i].trim().starts_with("packages") && lines[i].contains('[') {
            lines.insert(i + 1, format!("    \"{}\",", package));
            inserted = true;
            break;
        }
    }

    if !inserted {
        return Err(format!("Could not find packages list for tier {}", tier));
    }

    let new_contents = lines.join("\n") + "\n";
    write_as_root(path, &new_contents)
}

/// Write `contents` to a root-owned path without requiring the user to invoke
/// nog via sudo. Pipes the buffer through `sudo tee` — sudo prompts the user
/// once (cached afterwards), tee writes the file with root's credentials. If
/// nog is already running as root, sudo passes through without prompting.
///
/// tee does a truncate-and-write; for a <1 KiB config this is effectively
/// atomic in practice. A crash during the write would corrupt the file, but
/// the same was true of the previous `fs::write` path, so we're not
/// regressing.
fn write_as_root(path: &str, contents: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("sudo")
        .arg("tee")
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("failed to launch sudo tee: {}", e))?;

    {
        let stdin = child.stdin.as_mut()
            .ok_or_else(|| "sudo tee: stdin not captured".to_string())?;
        stdin.write_all(contents.as_bytes())
            .map_err(|e| format!("failed to pipe contents to sudo tee: {}", e))?;
    }

    let status = child.wait()
        .map_err(|e| format!("sudo tee wait failed: {}", e))?;
    if !status.success() {
        return Err(format!("sudo tee {} exited with status {}", path, status));
    }
    Ok(())
}