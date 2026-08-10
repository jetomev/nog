//! v1.0.9 (Ironhold): persisted source kill switches.
//!
//! `nog deactivate aur` / `nog activate aur` flip a per-source flag persisted
//! in `/etc/nog/sources.toml` — a nog-owned file (the tier-pins.toml
//! precedent), written via `sudo tee`, so the user's hand-edited `nog.conf`
//! (comments, helper choice) is never rewritten and `activate` restores
//! exactly the helper that was configured.
//!
//! Semantics:
//!   - Missing file            → every source ACTIVE (old installs unaffected).
//!   - Unparseable file        → every source DEACTIVATED, loud warning. This
//!     is a kill switch: corrupted state must not silently re-open a source
//!     (the cycle's fail-closed doctrine, same as the foreign fence).
//!   - `aur = false`           → every AUR-aware path refuses; nog behaves as
//!     if no helper were configured (detection, install routing, handoff).
//!   - `chaotic-aur = false`   → pacman.conf section toggling (phase A3).

use serde::Deserialize;
use std::fs;

use crate::tiers::write_as_root;

pub const DEFAULT_PATH: &str = "/etc/nog/sources.toml";

#[derive(Debug, Clone, PartialEq)]
pub struct SourceState {
    pub aur: bool,
    pub chaotic_aur: bool,
    pub flatpak: bool,
}

impl Default for SourceState {
    fn default() -> Self {
        SourceState { aur: true, chaotic_aur: true, flatpak: true }
    }
}

fn default_true() -> bool { true }

#[derive(Deserialize, Default)]
struct FileFormat {
    #[serde(default)]
    sources: SourcesSection,
}

#[derive(Deserialize)]
struct SourcesSection {
    #[serde(default = "default_true")]
    aur: bool,
    #[serde(rename = "chaotic-aur", default = "default_true")]
    chaotic_aur: bool,
    #[serde(default = "default_true")]
    flatpak: bool,
}

impl Default for SourcesSection {
    fn default() -> Self {
        SourcesSection { aur: true, chaotic_aur: true, flatpak: true }
    }
}

/// Parse sources.toml text. Missing keys default to active — deactivation is
/// always an explicit statement in the file.
pub fn parse(text: &str) -> Result<SourceState, String> {
    let parsed: FileFormat = toml::from_str(text)
        .map_err(|e| format!("could not parse sources.toml: {}", e))?;
    Ok(SourceState {
        aur: parsed.sources.aur,
        chaotic_aur: parsed.sources.chaotic_aur,
        flatpak: parsed.sources.flatpak,
    })
}

/// Render the canonical file content for a state.
pub fn render(state: &SourceState) -> String {
    format!(
        "# Managed by `nog activate <source>` / `nog deactivate <source>`.\n\
         # A missing file or missing key means the source is ACTIVE.\n\
         [sources]\n\
         aur = {}\n\
         \"chaotic-aur\" = {}\n\
         flatpak = {}\n",
        state.aur, state.chaotic_aur, state.flatpak
    )
}

/// Load the persisted state. Missing file → all active. Unparseable file →
/// all DEACTIVATED (fail closed) with a loud warning; re-running
/// `nog activate <source>` rewrites the file cleanly.
pub fn load(path: &str) -> SourceState {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return SourceState::default(),
    };
    match parse(&text) {
        Ok(state) => state,
        Err(e) => {
            eprintln!("nog: warning — {} is unreadable ({})", path, e);
            eprintln!("     failing CLOSED: treating every source as DEACTIVATED.");
            eprintln!("     `nog activate aur` (etc.) rewrites the file cleanly.");
            SourceState { aur: false, chaotic_aur: false, flatpak: false }
        }
    }
}

/// Persist the state via `sudo tee` (root-owned path, nog stays unprivileged).
pub fn save(path: &str, state: &SourceState) -> Result<(), String> {
    write_as_root(path, &render(state))
}

/// Marker prefix used when nog comments out a pacman.conf repo section.
/// Distinct from a plain `#` so `activate` restores exactly — and only —
/// what nog itself disabled, never a comment the user wrote by hand.
pub const REPO_MARKER: &str = "#nog# ";

/// Outcome of a pacman.conf repo-section toggle.
#[derive(Debug, PartialEq)]
pub enum RepoToggle {
    /// The section was found and flipped; here is the new file content.
    Changed(String),
    /// The section is already in the requested state — nothing to write.
    AlreadyInState,
    /// No `[repo]` section (active or nog-disabled) exists in this file.
    NotFound,
}

/// v1.0.9 (A3): comment a `[repo]` section in or out of pacman.conf text.
///
/// `enable = false` prefixes every non-blank line of the section (header
/// through the line before the next section header) with `#nog# `;
/// `enable = true` strips exactly that prefix. Blank lines and every byte
/// outside the section are preserved untouched. Pure — the caller owns
/// backups and the root write.
pub fn toggle_repo_section(conf: &str, repo: &str, enable: bool) -> RepoToggle {
    let active_header = format!("[{}]", repo);
    let disabled_header = format!("{}{}", REPO_MARKER, active_header);

    let lines: Vec<&str> = conf.lines().collect();
    let mut start = None;
    let mut currently_active = false;
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t == active_header {
            start = Some(i);
            currently_active = true;
            break;
        }
        if t == disabled_header {
            start = Some(i);
            currently_active = false;
            break;
        }
    }
    let Some(start) = start else { return RepoToggle::NotFound };
    if currently_active == enable {
        return RepoToggle::AlreadyInState;
    }

    let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    for i in start..lines.len() {
        let raw = lines[i];
        // The "logical" line ignores our own marker, so a disabled section's
        // body doesn't hide the NEXT section's header from the scan.
        let logical = raw.trim();
        let logical = logical.strip_prefix(REPO_MARKER).unwrap_or(logical).trim();
        if i > start && logical.starts_with('[') && logical.ends_with(']') {
            break; // next repo section — ours ended on the previous line
        }
        if enable {
            if let Some(rest) = raw.trim_start().strip_prefix(REPO_MARKER) {
                out[i] = rest.to_string();
            }
        } else if !raw.trim().is_empty() {
            out[i] = format!("{}{}", REPO_MARKER, raw);
        }
    }

    RepoToggle::Changed(out.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_keys_default_to_active() {
        let s = parse("[sources]\n").unwrap();
        assert!(s.aur);
        assert!(s.chaotic_aur);
        assert!(s.flatpak);
        let s = parse("").unwrap();
        assert_eq!(s, SourceState::default());
    }

    #[test]
    fn deactivation_roundtrips_through_render() {
        let state = SourceState { aur: false, chaotic_aur: true, flatpak: true };
        let s = parse(&render(&state)).unwrap();
        assert_eq!(s, state);
        let state = SourceState { aur: true, chaotic_aur: false, flatpak: false };
        assert_eq!(parse(&render(&state)).unwrap(), state);
    }

    #[test]
    fn explicit_flags_are_read() {
        let s = parse("[sources]\naur = false\n\"chaotic-aur\" = false\nflatpak = false\n").unwrap();
        assert!(!s.aur);
        assert!(!s.chaotic_aur);
        assert!(!s.flatpak);
    }

    const CONF: &str = "\
[options]
HoldPkg = pacman glibc

[core]
Include = /etc/pacman.d/mirrorlist

[chaotic-aur]
Include = /etc/pacman.d/chaotic-mirrorlist

[multilib]
Include = /etc/pacman.d/mirrorlist
";

    #[test]
    fn deactivate_comments_exactly_the_chaotic_section() {
        let got = toggle_repo_section(CONF, "chaotic-aur", false);
        let RepoToggle::Changed(new) = got else { panic!("expected Changed, got {:?}", got) };
        assert!(new.contains("#nog# [chaotic-aur]"));
        assert!(new.contains("#nog# Include = /etc/pacman.d/chaotic-mirrorlist"));
        // Neighbors untouched:
        assert!(new.contains("\n[core]\n"));
        assert!(new.contains("\n[multilib]\n"));
        assert!(new.contains("HoldPkg = pacman glibc"));
    }

    #[test]
    fn activate_restores_the_original_bytes() {
        let RepoToggle::Changed(disabled) = toggle_repo_section(CONF, "chaotic-aur", false)
            else { panic!() };
        let RepoToggle::Changed(restored) = toggle_repo_section(&disabled, "chaotic-aur", true)
            else { panic!() };
        assert_eq!(restored, CONF);
    }

    #[test]
    fn toggle_is_idempotent_per_direction() {
        assert_eq!(toggle_repo_section(CONF, "chaotic-aur", true), RepoToggle::AlreadyInState);
        let RepoToggle::Changed(disabled) = toggle_repo_section(CONF, "chaotic-aur", false)
            else { panic!() };
        assert_eq!(toggle_repo_section(&disabled, "chaotic-aur", false), RepoToggle::AlreadyInState);
    }

    #[test]
    fn missing_section_is_not_found() {
        assert_eq!(toggle_repo_section(CONF, "ghost-repo", false), RepoToggle::NotFound);
    }

    #[test]
    fn section_at_end_of_file_and_user_comments_survive() {
        let conf = "[core]\nInclude = /etc/pacman.d/mirrorlist\n\n# my note\n[chaotic-aur]\n# user comment inside\nInclude = /etc/pacman.d/chaotic-mirrorlist\n";
        let RepoToggle::Changed(disabled) = toggle_repo_section(conf, "chaotic-aur", false)
            else { panic!() };
        // The user's note ABOVE the section is untouched; the comment INSIDE
        // the section gets the marker (and is restored on activate).
        assert!(disabled.contains("\n# my note\n"));
        assert!(disabled.contains("#nog# # user comment inside"));
        let RepoToggle::Changed(restored) = toggle_repo_section(&disabled, "chaotic-aur", true)
            else { panic!() };
        assert_eq!(restored, conf);
    }

    #[test]
    fn garbage_is_an_error_for_load_to_fail_closed_on() {
        // parse() reports the error; load() maps it to all-deactivated. The
        // fail-closed mapping itself lives in load(), which needs a real file
        // — here we pin the contract that garbage does NOT silently parse.
        assert!(parse("sources = 5\n[sources").is_err());
    }
}
