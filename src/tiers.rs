use serde::Deserialize;
use std::collections::HashMap;
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
    #[allow(dead_code)]
    tier3: TierConfig,
    #[serde(default)]
    groups: HashMap<String, Vec<String>>,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum Tier {
    One,
    Two,
    Three,
}

impl Tier {
    fn rank(&self) -> u8 {
        match self {
            Tier::One => 1,
            Tier::Two => 2,
            Tier::Three => 3,
        }
    }
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
    // Built once at load time so classify() doesn't iterate the [groups] table
    // on every call. Maps each package that appears in any group to the full
    // member list of every group it belongs to (duplicates tolerated; iteration
    // is small).
    group_members: HashMap<String, Vec<String>>,
}

impl TierManager {
    pub fn load(path: &str) -> Result<Self, String> {
        let contents = fs::read_to_string(path)
            .map_err(|e| format!("Could not read {}: {}", path, e))?;
        let pins: TierPins = toml::from_str(&contents)
            .map_err(|e| format!("Could not parse tier-pins.toml: {}", e))?;
        let group_members = build_group_index(&pins.groups);
        Ok(TierManager { pins, group_members })
    }

    /// Classify a package into Tier 1/2/3.
    ///
    /// Resolution order:
    ///   1. Direct membership in `[tier1].packages` or `[tier2].packages`.
    ///   2. `<X>-headers` auto-coupling: a header package inherits Tier 1 when
    ///      its base kernel `<X>` is Tier 1. Same PKGBUILD produces both, so
    ///      their build dates match; coupling keeps them bucketing together at
    ///      `nog update` plan time. Without this, headers would default to
    ///      Tier 3 and flow ahead of held kernels — the desync that breaks
    ///      DKMS modules (e.g. `nvidia-open-dkms`) after reboot.
    ///   3. Explicit `[groups]` membership: inherit the highest tier present
    ///      among any other group member. The escape hatch for non-standard
    ///      kernel names or custom bundles.
    ///   4. Default Tier 3.
    pub fn classify(&self, package: &str) -> Tier {
        if self.in_tier1(package) { return Tier::One; }
        if self.in_tier2(package) { return Tier::Two; }

        if let Some(base) = package.strip_suffix("-headers") {
            if self.in_tier1(base) { return Tier::One; }
        }

        if let Some(members) = self.group_members.get(package) {
            let mut highest = Tier::Three;
            for m in members {
                if m == package { continue; }
                let t = self.classify_no_groups(m);
                if t.rank() < highest.rank() {
                    highest = t;
                }
            }
            return highest;
        }

        Tier::Three
    }

    /// Inner classifier used during group resolution to avoid recursing back
    /// into the group table (and risking pathological cycles). Considers
    /// direct tier membership and the `*-headers` pattern only.
    fn classify_no_groups(&self, package: &str) -> Tier {
        if self.in_tier1(package) { return Tier::One; }
        if self.in_tier2(package) { return Tier::Two; }
        if let Some(base) = package.strip_suffix("-headers") {
            if self.in_tier1(base) { return Tier::One; }
        }
        Tier::Three
    }

    fn in_tier1(&self, pkg: &str) -> bool {
        self.pins.tier1.packages.as_ref()
            .map(|v| v.iter().any(|p| p == pkg))
            .unwrap_or(false)
    }

    fn in_tier2(&self, pkg: &str) -> bool {
        self.pins.tier2.packages.as_ref()
            .map(|v| v.iter().any(|p| p == pkg))
            .unwrap_or(false)
    }

    pub fn is_manual_signoff(&self, package: &str) -> bool {
        self.classify(package) == Tier::One && self.pins.tier1.manual_signoff
    }

    /// The explicit tier-1 package names from tier-pins.toml. Used by the
    /// desync detector to enumerate kernel/headers pairs to inspect.
    pub fn tier1_packages(&self) -> Vec<String> {
        self.pins.tier1.packages.clone().unwrap_or_default()
    }
}

fn build_group_index(groups: &HashMap<String, Vec<String>>) -> HashMap<String, Vec<String>> {
    let mut idx: HashMap<String, Vec<String>> = HashMap::new();
    for members in groups.values() {
        for pkg in members {
            idx.entry(pkg.clone()).or_default().extend(members.iter().cloned());
        }
    }
    idx
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager(toml_src: &str) -> TierManager {
        let pins: TierPins = toml::from_str(toml_src).expect("test toml parses");
        let group_members = build_group_index(&pins.groups);
        TierManager { pins, group_members }
    }

    const BASE_TOML: &str = r#"
[tier1]
manual_signoff = false
packages = ["linux", "linux-zen", "linux-lts", "glibc", "systemd"]

[tier2]
manual_signoff = false
packages = ["firefox", "plasma-desktop"]

[tier3]
manual_signoff = false
"#;

    #[test]
    fn direct_tier_lookup_unchanged() {
        let tm = make_manager(BASE_TOML);
        assert_eq!(tm.classify("linux"), Tier::One);
        assert_eq!(tm.classify("firefox"), Tier::Two);
        assert_eq!(tm.classify("htop"), Tier::Three);
    }

    #[test]
    fn headers_auto_couple_to_tier1_kernel() {
        // The bug we're fixing: linux-zen-headers should bucket with linux-zen.
        let tm = make_manager(BASE_TOML);
        assert_eq!(tm.classify("linux-headers"), Tier::One);
        assert_eq!(tm.classify("linux-zen-headers"), Tier::One);
        assert_eq!(tm.classify("linux-lts-headers"), Tier::One);
    }

    #[test]
    fn headers_for_non_tier1_falls_through() {
        // Headers for a Tier 2 or Tier 3 package don't inherit — the bug is
        // specific to kernel/headers/DKMS coupling, and no Tier 2/3 package
        // produces a -headers companion in practice. Stay conservative.
        let tm = make_manager(BASE_TOML);
        assert_eq!(tm.classify("firefox-headers"), Tier::Three);
        assert_eq!(tm.classify("htop-headers"), Tier::Three);
    }

    #[test]
    fn unrelated_headers_pattern_is_tier3() {
        // Packages that happen to end in "-headers" but whose base name isn't
        // a tier-1 package stay in Tier 3.
        let tm = make_manager(BASE_TOML);
        assert_eq!(tm.classify("ghost-headers"), Tier::Three);
    }

    #[test]
    fn group_inherits_highest_tier_among_members() {
        let toml_src = r#"
[tier1]
manual_signoff = false
packages = ["linux-cachyos"]

[tier2]
manual_signoff = false
packages = []

[tier3]
manual_signoff = false

[groups]
cachyos-bundle = ["linux-cachyos", "linux-cachyos-cacule-headers", "cachyos-tools"]
"#;
        let tm = make_manager(toml_src);
        // linux-cachyos is directly Tier 1 — covered by direct lookup.
        assert_eq!(tm.classify("linux-cachyos"), Tier::One);
        // -headers pattern alone wouldn't catch cacule-headers (base name is
        // "linux-cachyos-cacule", not "linux-cachyos"). The group bundles it.
        assert_eq!(tm.classify("linux-cachyos-cacule-headers"), Tier::One);
        // Non-kernel member of the group also gets pulled up.
        assert_eq!(tm.classify("cachyos-tools"), Tier::One);
    }

    #[test]
    fn group_with_no_tier1_member_stays_tier3() {
        // A group with only Tier 2/3 members lands at the highest of those.
        let toml_src = r#"
[tier1]
manual_signoff = false
packages = []

[tier2]
manual_signoff = false
packages = ["firefox"]

[tier3]
manual_signoff = false

[groups]
browser-bundle = ["firefox", "firefox-extension-mailvelope"]
"#;
        let tm = make_manager(toml_src);
        assert_eq!(tm.classify("firefox"), Tier::Two);
        assert_eq!(tm.classify("firefox-extension-mailvelope"), Tier::Two);
    }

    #[test]
    fn empty_groups_table_is_fine() {
        // Missing [groups] section (the common case) shouldn't break anything.
        let tm = make_manager(BASE_TOML);
        assert_eq!(tm.classify("linux"), Tier::One);
    }

    #[test]
    fn tier1_packages_exposes_the_explicit_list() {
        let tm = make_manager(BASE_TOML);
        let names = tm.tier1_packages();
        assert!(names.contains(&"linux".to_string()));
        assert!(names.contains(&"glibc".to_string()));
        assert_eq!(names.len(), 5);
    }
}
