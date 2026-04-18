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

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::HoldsConfig;
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
    let build_ts = match build_dates.get(package) {
        Some(ts) => *ts,
        None => return HoldStatus::Unknown,
    };

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
}