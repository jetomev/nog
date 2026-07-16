// holds.rs — pure date-comparison logic for the hold system
//
// Given a package, its tier, the sync-DB build dates, and the configured hold
// windows, decide whether that package's hold has expired, is still running,
// or can't be evaluated (no build date available).
//
// This module is intentionally side-effect-free: no filesystem reads, no
// subprocesses, no clock calls. The caller passes in every input, including
// `now`. That keeps the logic trivially testable and makes it impossible to
// accidentally couple the comparison to the rest of the system.
//
// Phase 2 delivers this module. Phase 3 will consume it from `nog update`.

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::HoldsConfig;
use crate::sync_db::PackageDesc;
use crate::tiers::Tier;

/// The result of evaluating a package's hold.
///
/// `days_past_window` and `days_remaining` are always non-negative — callers
/// don't need to reason about signs. The variant tells you which side of the
/// hold window "today" is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoldStatus {
    /// The hold window has elapsed. Package is eligible to update.
    Expired { days_past_window: u64 },
    /// The hold is still active. Package should NOT be updated yet.
    Holding { days_remaining: u64 },
    /// No build date could be found for this package (e.g. not in any sync
    /// database we read). Caller decides how to present this to the user.
    Unknown,
}

const SECONDS_PER_DAY: u64 = 86_400;

/// Evaluate the hold status of a single package.
///
/// Pure function. All inputs explicit; no hidden state.
///
/// # Rounding rule
/// Elapsed time is rounded **up** to the next whole day — e.g. 4.2 days since
/// build counts as 5 elapsed days. This is the conservative/honest choice: we
/// show the user the older "elapsed" value, which produces a shorter "remaining"
/// value and avoids "oh I thought I had another day" surprises.
pub fn evaluate(
    package: &str,
    tier: Tier,
    build_dates: &HashMap<String, u64>,
    holds: &HoldsConfig,
    now: SystemTime,
) -> HoldStatus {
    match build_dates.get(package) {
        Some(ts) => evaluate_ts(*ts, tier, holds, now),
        None => HoldStatus::Unknown,
    }
}

/// Evaluate the hold status of a pending update against the exact candidate
/// it proposes to install.
///
/// Same date math as `evaluate`, plus a version guard: a build date is only
/// meaningful for the version it belongs to. If the DB entry we're reading
/// the date from is NOT the pending candidate's version, evaluating it would
/// clock the hold window from a different build of the package — that is the
/// 2026-07-06 finding, where holds evaluated against the stale system sync
/// DB dated every first-sighting update from its PREDECESSOR's builddate
/// (years old in the worst case) and waved it straight through its window.
/// Mismatches return `Unknown`, which routes to the per-package y/N prompt —
/// conservative, and honest about what we actually know.
///
/// Entries with `version: None` (AUR helper dates, defensive desc fallback)
/// skip the guard and evaluate on build date alone, as before.
pub fn evaluate_candidate(
    package: &str,
    tier: Tier,
    candidate_version: &str,
    packages: &HashMap<String, PackageDesc>,
    holds: &HoldsConfig,
    now: SystemTime,
) -> HoldStatus {
    let desc = match packages.get(package) {
        Some(d) => d,
        None => return HoldStatus::Unknown,
    };

    if let Some(db_version) = &desc.version {
        if db_version != candidate_version {
            return HoldStatus::Unknown;
        }
    }

    evaluate_ts(desc.builddate, tier, holds, now)
}

/// Couple a `lib32-<X>` multilib package to its base `<X>` at hold-release time
/// (issue #1).
///
/// A `lib32-<X>` package hard-depends on its base `<X>` at an exact version
/// (`lib32-nvidia-utils` → `nvidia-utils=<ver>`). Their hold windows are dated
/// per-package from first-sighting, so they can expire on different days and
/// land in different buckets — one Ready, one Held. Releasing only half the pair
/// leaves pacman unable to satisfy the exact-version dependency and the whole
/// transaction aborts. Tier classification already treats them alike; hold
/// *release* did not, which is the gap this closes.
///
/// Given the package names currently in the Ready and Held buckets, return the
/// Ready names that must be demoted into Held so each split pair moves as a unit,
/// each paired with the held partner it is waiting on (for display, and to
/// inherit that partner's countdown). Coupling is bidirectional: it fires
/// whether the `lib32-` half or the base half is the one still held.
pub fn lib32_coupling_demotions(ready: &[String], held: &[String]) -> Vec<(String, String)> {
    let held_set: HashSet<&str> = held.iter().map(String::as_str).collect();
    let mut demotions = Vec::new();
    for name in ready {
        // Direction 1: lib32-<X> is Ready while its base <X> is Held.
        if let Some(base) = name.strip_prefix("lib32-") {
            if held_set.contains(base) {
                demotions.push((name.clone(), base.to_string()));
                continue;
            }
        }
        // Direction 2: base <X> is Ready while its lib32-<X> shim is Held.
        // Upgrading the base alone would break the installed shim's exact-version
        // dependency, so the base waits for the shim.
        let sibling = format!("lib32-{name}");
        if held_set.contains(sibling.as_str()) {
            demotions.push((name.clone(), sibling));
        }
    }
    demotions
}

/// The shared date math: elapsed days since `build_ts` (rounded up) compared
/// against the tier's hold window.
fn evaluate_ts(build_ts: u64, tier: Tier, holds: &HoldsConfig, now: SystemTime) -> HoldStatus {
    let now_ts = match now.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        // Clock is before 1970 — absurd, but bail safely rather than panic.
        Err(_) => return HoldStatus::Unknown,
    };

    // Clock skew, mirrors serving future-dated packages, etc. Treat "built in
    // the future" as zero elapsed time rather than producing a negative value.
    let elapsed_secs = now_ts.saturating_sub(build_ts);
    let elapsed_days = days_ceil(elapsed_secs);

    let window_days = match tier {
        Tier::One => holds.tier1_days as u64,
        Tier::Two => holds.tier2_days as u64,
        Tier::Three => holds.tier3_days as u64,
    };

    if elapsed_days >= window_days {
        HoldStatus::Expired {
            days_past_window: elapsed_days - window_days,
        }
    } else {
        HoldStatus::Holding {
            days_remaining: window_days - elapsed_days,
        }
    }
}

/// Convert seconds to days, rounding **up**. 0s -> 0d, 1s -> 1d, 86400s -> 1d,
/// 86401s -> 2d. Matches the spec's "4.x is 5 automatically" rule.
fn days_ceil(seconds: u64) -> u64 {
    if seconds == 0 {
        0
    } else {
        (seconds + SECONDS_PER_DAY - 1) / SECONDS_PER_DAY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn holds_default() -> HoldsConfig {
        HoldsConfig {
            tier1_days: 30,
            tier2_days: 15,
            tier3_days: 7,
        }
    }

    fn at_days_after_epoch(days: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(days * SECONDS_PER_DAY)
    }

    #[test]
    fn unknown_when_package_not_in_build_dates() {
        let dates: HashMap<String, u64> = HashMap::new();
        let got = evaluate(
            "ghost",
            Tier::Three,
            &dates,
            &holds_default(),
            at_days_after_epoch(100),
        );
        assert_eq!(got, HoldStatus::Unknown);
    }

    #[test]
    fn expired_when_elapsed_exceeds_window() {
        // Built at day 0, checked at day 20, Tier 3 window = 7 days.
        // Elapsed 20d, past window by 13d.
        let mut dates = HashMap::new();
        dates.insert("firefox".to_string(), 0);
        let got = evaluate(
            "firefox",
            Tier::Three,
            &dates,
            &holds_default(),
            at_days_after_epoch(20),
        );
        assert_eq!(got, HoldStatus::Expired { days_past_window: 13 });
    }

    #[test]
    fn holding_when_within_window() {
        // Built at day 10, checked at day 12, Tier 1 window = 30 days.
        // Elapsed 2d, remaining 28d.
        let mut dates = HashMap::new();
        dates.insert("linux".to_string(), 10 * SECONDS_PER_DAY);
        let got = evaluate(
            "linux",
            Tier::One,
            &dates,
            &holds_default(),
            at_days_after_epoch(12),
        );
        assert_eq!(got, HoldStatus::Holding { days_remaining: 28 });
    }

    #[test]
    fn partial_day_rounds_up_per_spec() {
        // Built at t=0, checked at t = 4.2 days. Spec: 4.x -> 5 elapsed days.
        // Tier 2 window = 15. Remaining should be 15 - 5 = 10.
        let mut dates = HashMap::new();
        dates.insert("plasma-desktop".to_string(), 0);

        // 4.2 days = 362880 seconds
        let now = UNIX_EPOCH + Duration::from_secs(362_880);
        let got = evaluate(
            "plasma-desktop",
            Tier::Two,
            &dates,
            &holds_default(),
            now,
        );
        assert_eq!(got, HoldStatus::Holding { days_remaining: 10 });
    }

    #[test]
    fn boundary_exactly_one_window_is_expired_not_holding() {
        // Built at day 0, checked at exactly day 7, Tier 3 window = 7.
        // elapsed_days (7) >= window_days (7) -> Expired with 0 past window.
        let mut dates = HashMap::new();
        dates.insert("htop".to_string(), 0);
        let got = evaluate(
            "htop",
            Tier::Three,
            &dates,
            &holds_default(),
            at_days_after_epoch(7),
        );
        assert_eq!(got, HoldStatus::Expired { days_past_window: 0 });
    }

    #[test]
    fn built_in_the_future_treated_as_zero_elapsed() {
        // Package claims build at day 20, we check at day 10. Clock skew or
        // a mirror serving future-dated metadata. Should behave as day 0 elapsed.
        let mut dates = HashMap::new();
        dates.insert("weird".to_string(), 20 * SECONDS_PER_DAY);
        let got = evaluate(
            "weird",
            Tier::One,
            &dates,
            &holds_default(),
            at_days_after_epoch(10),
        );
        // Elapsed = 0, Tier 1 window = 30, remaining = 30.
        assert_eq!(got, HoldStatus::Holding { days_remaining: 30 });
    }

    // --- evaluate_candidate: the v1.0.5 version guard ---

    fn pkg_map(name: &str, builddate: u64, version: Option<&str>) -> HashMap<String, PackageDesc> {
        let mut m = HashMap::new();
        m.insert(name.to_string(), PackageDesc {
            builddate,
            pkgbase: None,
            version: version.map(|v| v.to_string()),
        });
        m
    }

    #[test]
    fn candidate_version_mismatch_returns_unknown() {
        // The 2026-07-06 failure shape: the DB entry is the PREDECESSOR
        // (1.1.0-1, built ~day 0 = ancient) but the pending candidate is
        // 1.2.0-2. Old behavior: Expired by ~968 days -> installed with zero
        // hold. Guarded behavior: Unknown -> per-package prompt.
        let pkgs = pkg_map("lib32-brotli", 0, Some("1.1.0-1"));
        let got = evaluate_candidate(
            "lib32-brotli",
            Tier::Three,
            "1.2.0-2",
            &pkgs,
            &holds_default(),
            at_days_after_epoch(975),
        );
        assert_eq!(got, HoldStatus::Unknown);
    }

    #[test]
    fn candidate_version_match_evaluates_normally() {
        // Fresh DB entry IS the candidate: built at day 20, checked at day
        // 21, Tier 3 window = 7 -> Holding with 6 remaining. This is what a
        // 1-day-old package should look like.
        let pkgs = pkg_map("lib32-brotli", 20 * SECONDS_PER_DAY, Some("1.2.0-2"));
        let got = evaluate_candidate(
            "lib32-brotli",
            Tier::Three,
            "1.2.0-2",
            &pkgs,
            &holds_default(),
            at_days_after_epoch(21),
        );
        assert_eq!(got, HoldStatus::Holding { days_remaining: 6 });
    }

    #[test]
    fn candidate_without_db_version_skips_guard() {
        // AUR helper dates carry no version — evaluate on build date alone,
        // exactly as pre-v1.0.5. Built day 0, checked day 20, Tier 3 window 7
        // -> Expired 13 past.
        let pkgs = pkg_map("fresh-editor-bin", 0, None);
        let got = evaluate_candidate(
            "fresh-editor-bin",
            Tier::Three,
            "0.4.3-1",
            &pkgs,
            &holds_default(),
            at_days_after_epoch(20),
        );
        assert_eq!(got, HoldStatus::Expired { days_past_window: 13 });
    }

    #[test]
    fn candidate_missing_from_map_is_unknown() {
        let pkgs: HashMap<String, PackageDesc> = HashMap::new();
        let got = evaluate_candidate(
            "ghost",
            Tier::Three,
            "1.0-1",
            &pkgs,
            &holds_default(),
            at_days_after_epoch(10),
        );
        assert_eq!(got, HoldStatus::Unknown);
    }

    // --- v1.0.6 lib32/base hold coupling (issue #1) ---

    fn owned(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn couples_lib32_ready_to_held_base() {
        // The reported nvidia case: lib32-nvidia-utils is Ready, nvidia-utils
        // is Held. The shim must be demoted and coupled to its base.
        let ready = owned(&["lib32-nvidia-utils", "poppler"]);
        let held = owned(&["nvidia-utils"]);
        let got = lib32_coupling_demotions(&ready, &held);
        assert_eq!(
            got,
            vec![("lib32-nvidia-utils".to_string(), "nvidia-utils".to_string())]
        );
    }

    #[test]
    fn couples_base_ready_to_held_lib32() {
        // Mirror direction: base is Ready, the lib32 shim is Held. Upgrading the
        // base alone would break the installed shim's exact-version dependency,
        // so the base is demoted and coupled to the shim.
        let ready = owned(&["nvidia-utils"]);
        let held = owned(&["lib32-nvidia-utils"]);
        let got = lib32_coupling_demotions(&ready, &held);
        assert_eq!(
            got,
            vec![("nvidia-utils".to_string(), "lib32-nvidia-utils".to_string())]
        );
    }

    #[test]
    fn no_coupling_when_pair_not_split() {
        // Both halves Ready (nothing Held) → the pair already moves together, so
        // there is nothing to demote.
        let ready = owned(&["lib32-mesa", "mesa"]);
        let held: Vec<String> = Vec::new();
        assert!(lib32_coupling_demotions(&ready, &held).is_empty());
    }

    #[test]
    fn non_lib32_ready_without_shim_is_untouched() {
        // A plain package whose lib32 sibling isn't in the update set at all is
        // never demoted, even when unrelated packages are Held.
        let ready = owned(&["firefox"]);
        let held = owned(&["nvidia-utils"]);
        assert!(lib32_coupling_demotions(&ready, &held).is_empty());
    }
}