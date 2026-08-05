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
}

impl Default for SourceState {
    fn default() -> Self {
        SourceState { aur: true, chaotic_aur: true }
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
}

impl Default for SourcesSection {
    fn default() -> Self {
        SourcesSection { aur: true, chaotic_aur: true }
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
    })
}

/// Render the canonical file content for a state.
pub fn render(state: &SourceState) -> String {
    format!(
        "# Managed by `nog activate <source>` / `nog deactivate <source>`.\n\
         # A missing file or missing key means the source is ACTIVE.\n\
         [sources]\n\
         aur = {}\n\
         \"chaotic-aur\" = {}\n",
        state.aur, state.chaotic_aur
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
            SourceState { aur: false, chaotic_aur: false }
        }
    }
}

/// Persist the state via `sudo tee` (root-owned path, nog stays unprivileged).
pub fn save(path: &str, state: &SourceState) -> Result<(), String> {
    write_as_root(path, &render(state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_keys_default_to_active() {
        let s = parse("[sources]\n").unwrap();
        assert!(s.aur);
        assert!(s.chaotic_aur);
        let s = parse("").unwrap();
        assert_eq!(s, SourceState::default());
    }

    #[test]
    fn deactivation_roundtrips_through_render() {
        let state = SourceState { aur: false, chaotic_aur: true };
        let s = parse(&render(&state)).unwrap();
        assert_eq!(s, state);
        let state = SourceState { aur: true, chaotic_aur: false };
        assert_eq!(parse(&render(&state)).unwrap(), state);
    }

    #[test]
    fn explicit_flags_are_read() {
        let s = parse("[sources]\naur = false\n\"chaotic-aur\" = false\n").unwrap();
        assert!(!s.aur);
        assert!(!s.chaotic_aur);
    }

    #[test]
    fn garbage_is_an_error_for_load_to_fail_closed_on() {
        // parse() reports the error; load() maps it to all-deactivated. The
        // fail-closed mapping itself lives in load(), which needs a real file
        // — here we pin the contract that garbage does NOT silently parse.
        assert!(parse("sources = 5\n[sources").is_err());
    }
}
