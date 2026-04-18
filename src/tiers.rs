use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct TierConfig {
    hold_days: u32,
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
            Tier::One   => write!(f, "Tier 1 (manual sign-off required)"),
            Tier::Two   => write!(f, "Tier 2 (10-day hold)"),
            Tier::Three => write!(f, "Tier 3 (fast-track)"),
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

    pub fn hold_days(&self, package: &str) -> u32 {
        match self.classify(package) {
            Tier::One   => self.pins.tier1.hold_days,
            Tier::Two   => self.pins.tier2.hold_days,
            Tier::Three => self.pins.tier3.hold_days,
        }
    }

    pub fn tier1_packages(&self) -> Vec<String> {
        self.pins.tier1.packages.clone().unwrap_or_default()
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

    // Find the right tier section and add the package
    // Tier 3 is the default — just removing from other tiers is enough
    if tier == 3 {
        let new_contents = lines.join("\n") + "\n";
        fs::write(path, new_contents)
            .map_err(|e| format!("Could not write {}: {}", path, e))?;
        return Ok(());
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

    // Write back
    let new_contents = lines.join("\n") + "\n";
    fs::write(path, new_contents)
        .map_err(|e| format!("Could not write {}: {}", path, e))?;

    Ok(())
}