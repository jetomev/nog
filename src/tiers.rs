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

/// pkgbase coupling indices for Layer A (v1.0.4).
///
/// Built once at startup from the sync DB metadata: `pkgbase_of` maps each
/// package name to the pkgbase it was built from, and `siblings_of` maps
/// each pkgbase to the list of packages produced by that PKGBUILD. Together
/// they let `classify()` ask "are any of P's split-PKGBUILD siblings on
/// Tier 1/2 hold?" in O(1) per package.
///
/// Empty by default. `commands::update` (and other places that benefit from
/// pkgbase-aware classification) populate it via `TierManager::with_pkgbase_index`
/// during startup; tests use `PkgbaseIndex::empty()` to keep classify
/// behavior limited to direct + name-pattern rules.
#[derive(Debug, Default)]
pub struct PkgbaseIndex {
    pub pkgbase_of: HashMap<String, String>,
    pub siblings_of: HashMap<String, Vec<String>>,
}

impl PkgbaseIndex {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build the indices from a sync-DB package map. Packages without a
    /// `%BASE%` value are skipped — they can't participate in sibling
    /// coupling but still work via the other classification rules.
    pub fn from_packages(packages: &HashMap<String, crate::sync_db::PackageDesc>) -> Self {
        let mut pkgbase_of: HashMap<String, String> = HashMap::new();
        let mut siblings_of: HashMap<String, Vec<String>> = HashMap::new();
        for (name, desc) in packages {
            if let Some(base) = &desc.pkgbase {
                pkgbase_of.insert(name.clone(), base.clone());
                siblings_of.entry(base.clone()).or_default().push(name.clone());
            }
        }
        Self { pkgbase_of, siblings_of }
    }
}

pub struct TierManager {
    pins: TierPins,
    // Built once at load time so classify() doesn't iterate the [groups] table
    // on every call. Maps each package that appears in any group to the full
    // member list of every group it belongs to (duplicates tolerated; iteration
    // is small).
    group_members: HashMap<String, Vec<String>>,
    // v1.0.4 pkgbase coupling indices. Empty by default (tier-pin-only behavior);
    // populated via `with_pkgbase_index` at startup in non-test builds.
    pkgbase_index: PkgbaseIndex,
}

impl TierManager {
    pub fn load(path: &str) -> Result<Self, String> {
        let contents = fs::read_to_string(path)
            .map_err(|e| format!("Could not read {}: {}", path, e))?;
        let pins: TierPins = toml::from_str(&contents)
            .map_err(|e| format!("Could not parse tier-pins.toml: {}", e))?;
        let group_members = build_group_index(&pins.groups);
        Ok(TierManager {
            pins,
            group_members,
            pkgbase_index: PkgbaseIndex::empty(),
        })
    }

    /// Attach a populated pkgbase coupling index. Used by callers that need
    /// split-PKGBUILD-aware classification (e.g., `nog update`, `nog search`).
    /// Without this, `classify()` falls back to direct + name-pattern + groups
    /// rules only — fine for tests and for early load paths that haven't
    /// touched the sync DB yet.
    pub fn with_pkgbase_index(mut self, idx: PkgbaseIndex) -> Self {
        self.pkgbase_index = idx;
        self
    }

    /// Classify a package into Tier 1/2/3.
    ///
    /// Resolution chain (first match wins):
    ///   1. Direct membership in `[tier1].packages` or `[tier2].packages`.
    ///   2. `<X>-headers` auto-coupling: a header package inherits Tier 1 when
    ///      its base kernel `<X>` is Tier 1. (v1.0.3 — kernel/headers/DKMS bug.)
    ///   3. `lib32-<X>` auto-coupling (v1.0.4): a multilib package inherits
    ///      the tier of `<X>` if `<X>` is Tier 1 or Tier 2. Bridges the
    ///      cross-PKGBUILD lockstep between e.g. `mesa` (main) and `lib32-mesa`
    ///      (multilib) which share no `pkgbase` but are enforced lockstep.
    ///   4. pkgbase sibling coupling (v1.0.4 — Layer A): packages sharing a
    ///      `pkgbase` are produced by the same PKGBUILD with versioned `=` deps,
    ///      and Arch enforces lockstep. The group inherits the highest tier
    ///      present among siblings. Auto-handles pipewire family, plasma, qt
    ///      coordinated subpackages, etc.
    ///   5. Explicit `[groups]` membership: inherit the highest tier among
    ///      other group members. Escape hatch for non-standard cases (e.g.,
    ///      `linux-cachyos-cacule-headers` which doesn't match the `*-headers`
    ///      pattern or share a pkgbase with `linux-cachyos`).
    ///   6. Default Tier 3.
    pub fn classify(&self, package: &str) -> Tier {
        let t = self.classify_no_groups(package);
        if t != Tier::Three {
            return t;
        }

        if let Some(members) = self.group_members.get(package) {
            let mut highest = Tier::Three;
            for m in members {
                if m == package { continue; }
                let mt = self.classify_no_groups(m);
                if mt.rank() < highest.rank() {
                    highest = mt;
                }
            }
            return highest;
        }

        Tier::Three
    }

    /// Full classification chain except `[groups]`. Used internally by the
    /// group resolver to avoid pathological cycles through the groups table.
    /// Still applies direct, `*-headers`, `lib32-`, and pkgbase coupling — so
    /// group members benefit from those rules transitively.
    fn classify_no_groups(&self, package: &str) -> Tier {
        if self.in_tier1(package) { return Tier::One; }
        if self.in_tier2(package) { return Tier::Two; }

        if let Some(base) = package.strip_suffix("-headers") {
            if self.in_tier1(base) { return Tier::One; }
        }

        if let Some(base) = package.strip_prefix("lib32-") {
            let t = self.classify_direct(base);
            if t != Tier::Three {
                return t;
            }
        }

        if let Some(pkgbase) = self.pkgbase_index.pkgbase_of.get(package) {
            if let Some(siblings) = self.pkgbase_index.siblings_of.get(pkgbase) {
                let mut highest = Tier::Three;
                for sibling in siblings {
                    if sibling == package { continue; }
                    let t = self.classify_direct(sibling);
                    if t.rank() < highest.rank() {
                        highest = t;
                    }
                }
                if highest != Tier::Three {
                    return highest;
                }
            }
        }

        Tier::Three
    }

    /// Innermost classifier — direct tier checks + the hardcoded name-pattern
    /// rules (`*-headers`, `lib32-`) only. Does NOT consult `[groups]` or
    /// pkgbase siblings. Used by the recursive resolvers (group, pkgbase) to
    /// classify referenced packages without risking infinite chains.
    fn classify_direct(&self, package: &str) -> Tier {
        if self.in_tier1(package) { return Tier::One; }
        if self.in_tier2(package) { return Tier::Two; }
        if let Some(base) = package.strip_suffix("-headers") {
            if self.in_tier1(base) { return Tier::One; }
        }
        if let Some(base) = package.strip_prefix("lib32-") {
            if self.in_tier1(base) { return Tier::One; }
            if self.in_tier2(base) { return Tier::Two; }
            if let Some(inner) = base.strip_suffix("-headers") {
                if self.in_tier1(inner) { return Tier::One; }
            }
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
    use crate::sync_db::PackageDesc;

    fn make_manager(toml_src: &str) -> TierManager {
        let pins: TierPins = toml::from_str(toml_src).expect("test toml parses");
        let group_members = build_group_index(&pins.groups);
        TierManager {
            pins,
            group_members,
            pkgbase_index: PkgbaseIndex::empty(),
        }
    }

    fn make_manager_with_pkgbases(
        toml_src: &str,
        pkgbases: &[(&str, &str)],
    ) -> TierManager {
        let mut pkgs: HashMap<String, PackageDesc> = HashMap::new();
        for (name, base) in pkgbases {
            pkgs.insert(name.to_string(), PackageDesc {
                builddate: 0,
                pkgbase: Some(base.to_string()),
            });
        }
        let pins: TierPins = toml::from_str(toml_src).expect("test toml parses");
        let group_members = build_group_index(&pins.groups);
        let pkgbase_index = PkgbaseIndex::from_packages(&pkgs);
        TierManager { pins, group_members, pkgbase_index }
    }

    const BASE_TOML: &str = r#"
[tier1]
manual_signoff = false
packages = ["linux", "linux-zen", "linux-lts", "glibc", "systemd", "mesa"]

[tier2]
manual_signoff = false
packages = ["firefox", "plasma-desktop", "pipewire", "pipewire-pulse"]

[tier3]
manual_signoff = false
"#;

    // ── v1.0.3 baseline behavior preserved ──────────────────────────────────

    #[test]
    fn direct_tier_lookup_unchanged() {
        let tm = make_manager(BASE_TOML);
        assert_eq!(tm.classify("linux"), Tier::One);
        assert_eq!(tm.classify("firefox"), Tier::Two);
        assert_eq!(tm.classify("htop"), Tier::Three);
    }

    #[test]
    fn headers_auto_couple_to_tier1_kernel() {
        let tm = make_manager(BASE_TOML);
        assert_eq!(tm.classify("linux-headers"), Tier::One);
        assert_eq!(tm.classify("linux-zen-headers"), Tier::One);
        assert_eq!(tm.classify("linux-lts-headers"), Tier::One);
    }

    #[test]
    fn headers_for_non_tier1_falls_through() {
        let tm = make_manager(BASE_TOML);
        assert_eq!(tm.classify("firefox-headers"), Tier::Three);
        assert_eq!(tm.classify("htop-headers"), Tier::Three);
    }

    #[test]
    fn unrelated_headers_pattern_is_tier3() {
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
        assert_eq!(tm.classify("linux-cachyos"), Tier::One);
        assert_eq!(tm.classify("linux-cachyos-cacule-headers"), Tier::One);
        assert_eq!(tm.classify("cachyos-tools"), Tier::One);
    }

    #[test]
    fn group_with_no_tier1_member_stays_tier3() {
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
        let tm = make_manager(BASE_TOML);
        assert_eq!(tm.classify("linux"), Tier::One);
    }

    #[test]
    fn tier1_packages_exposes_the_explicit_list() {
        let tm = make_manager(BASE_TOML);
        let names = tm.tier1_packages();
        assert!(names.contains(&"linux".to_string()));
        assert!(names.contains(&"glibc".to_string()));
        assert!(names.contains(&"mesa".to_string()));
        assert_eq!(names.len(), 6);
    }

    // ── v1.0.4 Layer B — lib32- prefix auto-coupling ───────────────────────

    #[test]
    fn lib32_inherits_tier1_when_base_is_tier1() {
        // mesa is Tier 1; lib32-mesa should bucket as Tier 1.
        // This covers the multilib lockstep case where the lib32- PKGBUILD is
        // separate (different pkgbase) but Arch ships them version-pinned.
        let tm = make_manager(BASE_TOML);
        assert_eq!(tm.classify("lib32-mesa"), Tier::One);
    }

    #[test]
    fn lib32_inherits_tier2_when_base_is_tier2() {
        let tm = make_manager(BASE_TOML);
        assert_eq!(tm.classify("lib32-firefox"), Tier::Two);
    }

    #[test]
    fn lib32_of_tier3_stays_tier3() {
        let tm = make_manager(BASE_TOML);
        assert_eq!(tm.classify("lib32-htop"), Tier::Three);
    }

    #[test]
    fn lib32_of_headers_inherits_via_inner_pattern() {
        // lib32-linux-headers → strip lib32- → "linux-headers" → strip -headers →
        // "linux" is Tier 1 → Tier 1. (Hypothetical chain — Arch doesn't ship
        // lib32 kernel headers, but the rule composes correctly.)
        let tm = make_manager(BASE_TOML);
        assert_eq!(tm.classify("lib32-linux-headers"), Tier::One);
    }

    // ── v1.0.4 Layer A — pkgbase sibling coupling ─────────────────────────

    #[test]
    fn pkgbase_sibling_inherits_tier2_from_base() {
        // The bug that surfaced 2026-05-25: pipewire family. `pipewire` is
        // Tier 2; siblings (libpipewire, pipewire-audio, etc.) share
        // pkgbase=pipewire and should bucket with pipewire.
        let pkgbases = [
            ("pipewire",          "pipewire"),
            ("pipewire-pulse",    "pipewire"),
            ("pipewire-jack",     "pipewire"),
            ("pipewire-audio",    "pipewire"),
            ("pipewire-alsa",     "pipewire"),
            ("libpipewire",       "pipewire"),
            ("gst-plugin-pipewire", "pipewire"),
            ("alsa-card-profiles", "pipewire"),
        ];
        let tm = make_manager_with_pkgbases(BASE_TOML, &pkgbases);
        // pipewire / pipewire-pulse — direct lookup, already Tier 2.
        assert_eq!(tm.classify("pipewire"), Tier::Two);
        assert_eq!(tm.classify("pipewire-pulse"), Tier::Two);
        // The split-PKGBUILD siblings — Tier 2 via pkgbase coupling.
        assert_eq!(tm.classify("libpipewire"), Tier::Two);
        assert_eq!(tm.classify("pipewire-audio"), Tier::Two);
        assert_eq!(tm.classify("pipewire-alsa"), Tier::Two);
        assert_eq!(tm.classify("pipewire-jack"), Tier::Two);
        assert_eq!(tm.classify("gst-plugin-pipewire"), Tier::Two);
        assert_eq!(tm.classify("alsa-card-profiles"), Tier::Two);
    }

    #[test]
    fn pkgbase_sibling_with_no_tier_pinned_member_stays_tier3() {
        // None of the siblings is in any tier — group classification stays
        // at default Tier 3. (Coupling only kicks in when at least one sibling
        // is explicitly pinned.)
        let pkgbases = [
            ("foo",     "foo"),
            ("foo-doc", "foo"),
        ];
        let tm = make_manager_with_pkgbases(BASE_TOML, &pkgbases);
        assert_eq!(tm.classify("foo-doc"), Tier::Three);
    }

    #[test]
    fn empty_pkgbase_index_falls_through_to_tier3() {
        // No pkgbase data attached (the default) — pkgbase rule never fires,
        // classification reduces to v1.0.3 behavior.
        let tm = make_manager(BASE_TOML);
        assert_eq!(tm.classify("libpipewire"), Tier::Three);
        assert_eq!(tm.classify("pipewire-audio"), Tier::Three);
    }

    // ── v1.0.4 Layer A + B compose — lib32 of a pkgbase-coupled package ───

    #[test]
    fn lib32_of_pkgbase_sibling_resolves_via_own_multilib_pkgbase() {
        // lib32-libpipewire (multilib build) has its own pkgbase = lib32-pipewire,
        // which produces lib32-pipewire (main) and lib32-libpipewire as siblings.
        // Coupling: lib32-libpipewire's sibling is lib32-pipewire; lib32-pipewire
        // is classified Tier 2 via Layer B (lib32- prefix → pipewire → Tier 2).
        // So lib32-libpipewire transitively gets Tier 2.
        let pkgbases = [
            ("pipewire",          "pipewire"),
            ("pipewire-pulse",    "pipewire"),
            ("libpipewire",       "pipewire"),
            ("lib32-pipewire",    "lib32-pipewire"),
            ("lib32-libpipewire", "lib32-pipewire"),
        ];
        let tm = make_manager_with_pkgbases(BASE_TOML, &pkgbases);
        // The multilib base: lib32-pipewire → strip lib32- → pipewire → Tier 2.
        assert_eq!(tm.classify("lib32-pipewire"), Tier::Two);
        // The interesting case: lib32-libpipewire couples to its lib32-pipewire
        // sibling via pkgbase, which itself classifies Tier 2 via the lib32 rule.
        assert_eq!(tm.classify("lib32-libpipewire"), Tier::Two);
    }
}
