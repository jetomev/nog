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

/// v1.0.9 (Operation Ironhold, finding 2026-08-04): the foreign fence.
///
/// The update handoff (`yay -Syu --ignore <list>`) can only respect holds on
/// packages the ignore list *names*. When the AUR update query fails or comes
/// back empty — precisely what happened during the August 2026 AUR lockdown —
/// held AUR packages vanish from the report, never make the list, and the
/// helper's own resolution upgrades them anyway: the hold fails OPEN.
///
/// The fence closes that door structurally instead of trying to distinguish
/// "no updates" from "couldn't check": every installed foreign package is
/// ignored by default, and only the AUR packages nog explicitly cleared this
/// run (Ready, or a user-approved Unknown) are let through. When everything is
/// healthy the extra ignores are no-ops — a package with no pending update is
/// unaffected by `--ignore`. When the AUR is dark, every foreign package stays
/// exactly where it is.
///
/// Returns the fence additions: foreign names that are neither cleared nor
/// already on the ignore list.
pub fn foreign_fence(
    foreign: &[String],
    cleared: &[String],
    already_ignored: &[String],
) -> Vec<String> {
    let cleared_set: HashSet<&str> = cleared.iter().map(String::as_str).collect();
    let ignored_set: HashSet<&str> = already_ignored.iter().map(String::as_str).collect();
    foreign
        .iter()
        .filter(|name| !cleared_set.contains(name.as_str()))
        .filter(|name| !ignored_set.contains(name.as_str()))
        .cloned()
        .collect()
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

/// One package as the coupling rules see it (issue #11).
///
/// Deliberately owns its strings: the caller rebuilds this list on every
/// fixpoint iteration as packages move between buckets, and borrowing from
/// the buckets it is about to mutate is more trouble than the allocation is
/// worth for a list this size.
#[derive(Debug, Clone)]
pub struct CouplingPkg {
    pub name: String,
    pub old_version: String,
    pub new_version: String,
    pub pkgbase: Option<String>,
    /// Days left on the hold. Meaningless for Ready entries (pass 0); for Held
    /// entries it is what a demoted package inherits so both rows show the same
    /// countdown and clear on the same run.
    pub remaining: u64,
}

/// Strip epoch and pkgrel from a pacman version string, leaving the upstream
/// version: `1:6.11.2-2` -> `6.11.2`.
///
/// pkgrel is what makes the Qt6 case invisible to a naive comparison — Arch
/// shipped the modules as `6.11.2-1` and rebuilt the base as `6.11.2-2` two days
/// later. Both are the same upstream release and must move together.
fn pkgver(version: &str) -> &str {
    let no_epoch = match version.split_once(':') {
        Some((_, rest)) => rest,
        None => version,
    };
    match no_epoch.rsplit_once('-') {
        Some((v, _)) => v,
        None => no_epoch,
    }
}

/// Of the held packages named in `group`, the one a demoted sibling should wait
/// on: the longest remaining countdown, ties broken by name so the choice is
/// stable run-to-run.
fn longest_held<'a>(group: &[&'a CouplingPkg]) -> Option<&'a CouplingPkg> {
    group
        .iter()
        .copied()
        .max_by(|a, b| a.remaining.cmp(&b.remaining).then_with(|| b.name.cmp(&a.name)))
}

/// Couple packages built from the same PKGBUILD (issue #11, defect 1).
///
/// Packages sharing a `%BASE%` are produced by one PKGBUILD and Arch enforces
/// their lockstep with versioned `=` dependencies (`elfutils` depends on
/// `libelf=0.196`). Their hold windows are dated per-package from first
/// sighting, so they can expire on different days and land in different
/// buckets. Releasing half the group makes the `=` dependency unsatisfiable
/// and pacman aborts the whole transaction.
///
/// `tiers.rs` already couples pkgbase siblings, but only to resolve *which
/// tier* a package belongs to. Nothing carried that grouping into the
/// hold-window decision until this rule.
pub fn pkgbase_coupling_demotions(
    ready: &[CouplingPkg],
    held: &[CouplingPkg],
) -> Vec<(String, String)> {
    let mut held_by_base: HashMap<&str, Vec<&CouplingPkg>> = HashMap::new();
    for pkg in held {
        if let Some(base) = pkg.pkgbase.as_deref() {
            held_by_base.entry(base).or_default().push(pkg);
        }
    }

    let mut demotions = Vec::new();
    for pkg in ready {
        let Some(base) = pkg.pkgbase.as_deref() else {
            continue;
        };
        if let Some(group) = held_by_base.get(base) {
            if let Some(partner) = longest_held(group) {
                demotions.push((pkg.name.clone(), partner.name.clone()));
            }
        }
    }
    demotions
}

/// Minimum members before a shared version is treated as a family rather than a
/// coincidence. Pairs are left to the pkgbase and lib32 rules, which are exact.
const COHORT_MIN: usize = 3;

/// Couple version cohorts across pkgbase boundaries (issue #11, defect 3).
///
/// Some families are version-locked by build-time convention with **nothing in
/// the package metadata to prove it**. The Qt6 stack is the reference case:
/// every module is its own pkgbase, and `qt6-declarative` depends on `qt6-base`
/// with no version constraint at all. Yet `libQt6Qml` from 6.11.2 asks qt6-base
/// for a private symbol (`QtPrivate_6_11_2`) that only exists in 6.11.2 — so
/// releasing the modules while holding the base leaves every Qt application
/// unable to start, including the display manager. pacman has no grounds to
/// object and does not.
///
/// The only signal actually present in the data is the version cohort: a set of
/// packages currently on the same upstream version and all moving to the same
/// new one. If such a group is split across buckets, hold the whole group.
///
/// This is a **heuristic**, unlike the other two rules, and it is deliberately
/// conservative — it will sometimes hold a family that did not need holding, at
/// a cost of a few days. That trade is the point of a tool whose premise is that
/// packages should settle before they land.
pub fn cohort_coupling_demotions(
    ready: &[CouplingPkg],
    held: &[CouplingPkg],
) -> Vec<(String, String)> {
    // Key on (current upstream version -> new upstream version). Both halves
    // matter: packages sitting on the same version that are moving to *different*
    // versions are not a family moving in step.
    let mut cohorts: HashMap<(&str, &str), (Vec<&CouplingPkg>, Vec<&CouplingPkg>)> = HashMap::new();
    for pkg in ready {
        cohorts
            .entry((pkgver(&pkg.old_version), pkgver(&pkg.new_version)))
            .or_default()
            .0
            .push(pkg);
    }
    for pkg in held {
        cohorts
            .entry((pkgver(&pkg.old_version), pkgver(&pkg.new_version)))
            .or_default()
            .1
            .push(pkg);
    }

    let mut demotions = Vec::new();
    for (ready_members, held_members) in cohorts.values() {
        if held_members.is_empty() || ready_members.is_empty() {
            continue; // uniform cohort — nothing is split
        }
        if ready_members.len() + held_members.len() < COHORT_MIN {
            continue; // a pair is the exact rules' business, not ours
        }
        let Some(partner) = longest_held(held_members) else {
            continue;
        };
        for pkg in ready_members {
            demotions.push((pkg.name.clone(), partner.name.clone()));
        }
    }
    demotions
}

/// Every coupling rule, applied to one snapshot of the buckets (issue #11).
///
/// Returns the Ready names that must move into Held, each paired with the held
/// package it is waiting on. A name appears at most once even when several rules
/// claim it; the first rule to claim it wins, in the order lib32 -> pkgbase ->
/// cohort (exact rules before the heuristic, so the displayed partner is the
/// most defensible one available).
///
/// This is a single pass and does **not** converge on its own — a demotion made
/// here can create a new split that only a re-run will see. Callers must loop
/// until this returns empty; see `commands::update`.
/// Everything the soname rule needs out of the two pacman databases
/// (issue #13, v1.3.1).
///
/// Owned rather than borrowed: it is built once per run from ~1400 installed
/// packages and read by every fixpoint pass, and the lifetimes needed to
/// borrow it through `coupling_demotions` would buy nothing at this size.
///
/// An empty `SonameData` makes the rule a no-op, which is how the other
/// rules' unit tests opt out of it and how `nog update` behaves if the local
/// database cannot be read.
#[derive(Debug, Default)]
pub struct SonameData {
    /// installed package -> its `%PROVIDES%`
    pub installed_provides: HashMap<String, Vec<String>>,
    /// installed package -> its `%DEPENDS%`
    pub installed_depends: HashMap<String, Vec<String>>,
    /// candidate -> `%PROVIDES%` of the version that would be installed
    pub new_provides: HashMap<String, Vec<String>>,
}

/// Is this dependency entry a versioned soname (`libbluray.so=3-64`)?
///
/// Matched on the **whole string**, deliberately. The architecture suffix is
/// what separates `libEGL.so=1-32` from `libEGL.so=1-64`, and 118 soname bases
/// exist at two versions at once on the machine this was written for. A rule
/// that compared base names would couple every one of those pairs and wedge
/// the update queue permanently.
///
/// Entries carrying `<` or `>` are ranges rather than exact soname matches and
/// are left alone: pacman can satisfy those from more than one version, so a
/// bump does not necessarily break them.
fn is_soname(entry: &str) -> bool {
    if entry.contains('<') || entry.contains('>') {
        return false;
    }
    match entry.split_once('=') {
        Some((base, _)) => base.ends_with(".so"),
        None => false,
    }
}

fn soname_set(list: Option<&Vec<String>>) -> HashSet<&str> {
    list.map(|v| v.iter().map(|s| s.as_str()).filter(|s| is_soname(s)).collect())
        .unwrap_or_default()
}

/// Couple a Ready package to a package it would break by dropping a soname
/// (issue #13).
///
/// The failure this exists for, seen live on 2026-08-28:
///
/// ```text
/// error: failed to prepare transaction (could not satisfy dependencies)
/// :: installing libbluray (1.5.0-1) breaks dependency 'libbluray.so=3-64'
///    required by ffmpeg4.4
/// ```
///
/// `libbluray` had cleared its hold and moves `libbluray.so` from 3 to 4.
/// `ffmpeg4.4` was still held with a day to run and still links the old one.
/// pacman cannot split the pair, so it refused all seventy-eight packages in
/// the transaction.
///
/// The other three rules cannot see this: the two packages share no pkgbase,
/// no `lib32-` name pattern, and no version cohort. The relationship exists
/// only in the dependency graph.
///
/// The test is: for each soname a Ready candidate would stop providing, will
/// **anything** still provide it afterwards? A provider that is not itself
/// moving keeps it, and so does one that moves but still provides it. If
/// nothing will, every installed package that still requires it — and is not
/// moving to a version that wants the new soname — would break, so the
/// candidate is held back until they can move together.
///
/// Dependents are drawn from **all installed packages**, not just the pending
/// ones. A foreign or AUR package built against the old soname has no
/// repository update to wait for, and would break exactly the same way.
pub fn soname_coupling_demotions(
    ready: &[CouplingPkg],
    held: &[CouplingPkg],
    data: &SonameData,
) -> Vec<(String, String)> {
    if ready.is_empty() || data.installed_provides.is_empty() {
        return Vec::new();
    }
    let moving: HashSet<&str> = ready.iter().map(|p| p.name.as_str()).collect();

    // soname -> installed packages providing it / requiring it
    let mut providers: HashMap<&str, Vec<&str>> = HashMap::new();
    for (pkg, list) in &data.installed_provides {
        for s in list.iter().filter(|s| is_soname(s)) {
            providers.entry(s.as_str()).or_default().push(pkg.as_str());
        }
    }
    let mut requirers: HashMap<&str, Vec<&str>> = HashMap::new();
    for (pkg, list) in &data.installed_depends {
        for s in list.iter().filter(|s| is_soname(s)) {
            requirers.entry(s.as_str()).or_default().push(pkg.as_str());
        }
    }

    let mut out: Vec<(String, String)> = Vec::new();
    for cand in ready {
        let before = soname_set(data.installed_provides.get(&cand.name));
        let after = soname_set(data.new_provides.get(&cand.name));

        for dropped in before.difference(&after) {
            let survives = providers
                .get(dropped)
                .map(|ps| {
                    ps.iter().any(|p| {
                        !moving.contains(p) || soname_set(data.new_provides.get(*p)).contains(dropped)
                    })
                })
                .unwrap_or(false);
            if survives {
                continue;
            }

            // Everyone still requiring it who is not moving alongside.
            let mut broken: Vec<&str> = requirers
                .get(dropped)
                .map(|rs| rs.iter().copied().filter(|r| !moving.contains(r)).collect())
                .unwrap_or_default();
            if broken.is_empty() {
                continue;
            }
            broken.sort_unstable();

            // Name the partner the user can act on: prefer one that is itself
            // a held candidate, so the row inherits a real countdown and both
            // clear on the same run. Otherwise the first by name, which keeps
            // the note stable run-to-run.
            let held_match: Vec<&CouplingPkg> = held
                .iter()
                .filter(|h| broken.contains(&h.name.as_str()))
                .collect();
            let partner = longest_held(&held_match)
                .map(|h| h.name.clone())
                .unwrap_or_else(|| broken[0].to_string());

            out.push((cand.name.clone(), partner));
            break; // one note per candidate is enough to explain the hold
        }
    }
    out
}

pub fn coupling_demotions(
    ready: &[CouplingPkg],
    held: &[CouplingPkg],
    soname: &SonameData,
) -> Vec<(String, String)> {
    let ready_names: Vec<String> = ready.iter().map(|p| p.name.clone()).collect();
    let held_names: Vec<String> = held.iter().map(|p| p.name.clone()).collect();

    let mut claimed: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for (name, partner) in lib32_coupling_demotions(&ready_names, &held_names)
        .into_iter()
        .chain(pkgbase_coupling_demotions(ready, held))
        .chain(cohort_coupling_demotions(ready, held))
        .chain(soname_coupling_demotions(ready, held, soname))
    {
        if claimed.insert(name.clone()) {
            out.push((name, partner));
        }
    }
    out
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
            provides: Vec::new(),
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

    #[test]
    fn fence_replays_the_august_first_bypass() {
        // 2026-08-01: the AUR was mid-lockdown, `yay -Qua` came back empty, and
        // sparrow-wallet + fresh-editor-bin — held with 5 days remaining the
        // night before — vanished from the report and were upgraded by the
        // handoff anyway. With the fence, an empty detection clears nothing, so
        // every foreign package lands on the ignore list.
        let foreign = owned(&["sparrow-wallet", "fresh-editor-bin", "nog", "yay-bin"]);
        let cleared: Vec<String> = Vec::new(); // detection saw nothing
        let ignored: Vec<String> = Vec::new(); // so nothing was held either
        let fence = foreign_fence(&foreign, &cleared, &ignored);
        assert_eq!(fence, foreign);
    }

    #[test]
    fn fence_lets_cleared_aur_packages_through() {
        // A healthy run: detection saw sparrow-wallet, its hold expired, nog
        // marked it Ready. The fence must not re-ignore it — only the foreign
        // packages nog did NOT clear stay fenced.
        let foreign = owned(&["sparrow-wallet", "fresh-editor-bin", "nog"]);
        let cleared = owned(&["sparrow-wallet"]);
        let ignored: Vec<String> = Vec::new();
        let fence = foreign_fence(&foreign, &cleared, &ignored);
        assert_eq!(fence, owned(&["fresh-editor-bin", "nog"]));
    }

    #[test]
    fn fence_skips_names_already_on_the_ignore_list() {
        // A held AUR package is already ignored by the normal hold path; the
        // fence adds only what is missing, so the final list stays duplicate-free.
        let foreign = owned(&["sparrow-wallet", "fresh-editor-bin"]);
        let cleared: Vec<String> = Vec::new();
        let ignored = owned(&["sparrow-wallet"]);
        let fence = foreign_fence(&foreign, &cleared, &ignored);
        assert_eq!(fence, owned(&["fresh-editor-bin"]));
    }

    // ---- issue #11: family coupling at hold-release time -------------------

    /// Terse constructor: `pkg("qt6-base", "6.11.1-1", "6.11.2-2", None, 1)`.
    fn pkg(
        name: &str,
        old: &str,
        new: &str,
        base: Option<&str>,
        remaining: u64,
    ) -> CouplingPkg {
        CouplingPkg {
            name: name.to_string(),
            old_version: old.to_string(),
            new_version: new.to_string(),
            pkgbase: base.map(str::to_string),
            remaining,
        }
    }

    fn demoted_names(mut d: Vec<(String, String)>) -> Vec<String> {
        d.sort();
        d.into_iter().map(|(n, _)| n).collect()
    }

    // ---- issue #13: soname coupling -------------------------------------
    //
    // Every fixture below is real data taken from the machine where the bug
    // was found, on 2026-08-28. The negative cases are not invented: eleven
    // soname bases genuinely coexist at two versions on that system, and a
    // rule matching base names rather than whole strings would couple all of
    // them and wedge the update queue.

    fn sd(
        installed: &[(&str, &[&str], &[&str])],
        new: &[(&str, &[&str])],
    ) -> SonameData {
        let mut d = SonameData::default();
        for (n, prov, deps) in installed {
            d.installed_provides.insert(
                n.to_string(),
                prov.iter().map(|s| s.to_string()).collect(),
            );
            d.installed_depends.insert(
                n.to_string(),
                deps.iter().map(|s| s.to_string()).collect(),
            );
        }
        for (n, prov) in new {
            d.new_provides.insert(
                n.to_string(),
                prov.iter().map(|s| s.to_string()).collect(),
            );
        }
        d
    }

    #[test]
    fn soname_rule_replays_the_libbluray_transaction_failure() {
        // pacman's own words, 2026-08-28:
        //   installing libbluray (1.5.0-1) breaks dependency
        //   'libbluray.so=3-64' required by ffmpeg4.4
        let data = sd(
            &[
                ("libbluray", &["libbluray.so=3-64"], &[]),
                ("ffmpeg4.4", &["libavcodec.so=58-64"], &["libbluray.so=3-64"]),
            ],
            &[("libbluray", &["libbluray.so=4-64"])],
        );
        let ready = vec![pkg("libbluray", "1.4.1-1", "1.5.0-1", None, 0)];
        let held = vec![pkg("ffmpeg4.4", "4.4.8-3", "4.4.8-5", None, 1)];
        assert_eq!(
            soname_coupling_demotions(&ready, &held, &data),
            vec![("libbluray".to_string(), "ffmpeg4.4".to_string())]
        );
    }

    #[test]
    fn soname_rule_ignores_same_base_at_different_versions() {
        // ffmpeg4.4 provides libavcodec.so=58, ffmpeg-obs provides =63, and
        // both are installed side by side. Matching on the base name would
        // couple them to each other forever.
        let data = sd(
            &[
                ("ffmpeg4.4", &["libavcodec.so=58-64"], &[]),
                ("ffmpeg-obs", &["libavcodec.so=63-64"], &[]),
                ("obs-studio", &[], &["libavcodec.so=63-64"]),
            ],
            &[("ffmpeg4.4", &["libavcodec.so=58-64"])],
        );
        let ready = vec![pkg("ffmpeg4.4", "4.4.8-3", "4.4.8-5", None, 0)];
        let held = vec![pkg("ffmpeg-obs", "9.0-1", "9.0.1-1.2", None, 4)];
        assert!(soname_coupling_demotions(&ready, &held, &data).is_empty());
    }

    #[test]
    fn soname_rule_ignores_the_32_and_64_bit_pair() {
        // 118 bases coexist as -32/-64 on the reference machine. The suffix is
        // part of the string, so they never collide.
        let data = sd(
            &[
                ("libglvnd", &["libEGL.so=1-64"], &[]),
                ("lib32-libglvnd", &["libEGL.so=1-32"], &[]),
                ("wine", &[], &["libEGL.so=1-32"]),
            ],
            &[("libglvnd", &["libEGL.so=2-64"])],
        );
        let ready = vec![pkg("libglvnd", "1.7-1", "1.8-1", None, 0)];
        let held: Vec<CouplingPkg> = vec![];
        assert!(soname_coupling_demotions(&ready, &held, &data).is_empty());
    }

    #[test]
    fn soname_rule_lets_a_pair_move_together() {
        // The dependent is Ready too, so its new version wants the new soname.
        // Holding either would be wrong — this is the state after `nog install
        // libbluray ffmpeg4.4`, and the workaround must not be undone.
        let data = sd(
            &[
                ("libbluray", &["libbluray.so=3-64"], &[]),
                ("ffmpeg4.4", &[], &["libbluray.so=3-64"]),
            ],
            &[("libbluray", &["libbluray.so=4-64"]), ("ffmpeg4.4", &[])],
        );
        let ready = vec![
            pkg("libbluray", "1.4.1-1", "1.5.0-1", None, 0),
            pkg("ffmpeg4.4", "4.4.8-3", "4.4.8-5", None, 0),
        ];
        assert!(soname_coupling_demotions(&ready, &[], &data).is_empty());
    }

    #[test]
    fn soname_rule_accepts_a_surviving_second_provider() {
        // Something else still provides the soname and is not moving, so the
        // dependency stays satisfiable and nothing needs holding.
        let data = sd(
            &[
                ("libxcrypt", &["libcrypt.so=2-64"], &[]),
                ("libxcrypt-compat", &["libcrypt.so=1-64"], &[]),
                ("oldapp", &[], &["libcrypt.so=1-64"]),
            ],
            &[("libxcrypt", &["libcrypt.so=3-64"])],
        );
        let ready = vec![pkg("libxcrypt", "4.4-1", "4.5-1", None, 0)];
        assert!(soname_coupling_demotions(&ready, &[], &data).is_empty());
    }

    #[test]
    fn soname_rule_holds_for_a_foreign_package_with_no_update() {
        // The dependent is an AUR package: nothing pending, no countdown to
        // inherit, and it would break exactly the same way. It must still be
        // named, or the user cannot tell why the hold exists.
        let data = sd(
            &[
                ("libbluray", &["libbluray.so=3-64"], &[]),
                ("some-aur-thing", &[], &["libbluray.so=3-64"]),
            ],
            &[("libbluray", &["libbluray.so=4-64"])],
        );
        let ready = vec![pkg("libbluray", "1.4.1-1", "1.5.0-1", None, 0)];
        assert_eq!(
            soname_coupling_demotions(&ready, &[], &data),
            vec![("libbluray".to_string(), "some-aur-thing".to_string())]
        );
    }

    #[test]
    fn soname_rule_prefers_a_held_partner_with_the_longest_wait() {
        // Two packages break. Name the one with the longest countdown, so the
        // row's inherited countdown is the one that actually gates the release.
        let data = sd(
            &[
                ("libbluray", &["libbluray.so=3-64"], &[]),
                ("dep-soon", &[], &["libbluray.so=3-64"]),
                ("dep-later", &[], &["libbluray.so=3-64"]),
            ],
            &[("libbluray", &["libbluray.so=4-64"])],
        );
        let ready = vec![pkg("libbluray", "1.4.1-1", "1.5.0-1", None, 0)];
        let held = vec![
            pkg("dep-soon", "1-1", "2-1", None, 1),
            pkg("dep-later", "1-1", "2-1", None, 6),
        ];
        assert_eq!(
            soname_coupling_demotions(&ready, &held, &data)[0].1,
            "dep-later"
        );
    }

    #[test]
    fn is_soname_accepts_exact_matches_and_rejects_ranges() {
        assert!(is_soname("libbluray.so=3-64"));
        assert!(is_soname("libEGL.so=1-32"));
        // A range can be satisfied by more than one version, so a bump does
        // not necessarily break it — out of scope for this rule.
        assert!(!is_soname("libfoo.so>=3"));
        assert!(!is_soname("libfoo.so<4"));
        // Plain package names and unversioned provides are not sonames.
        assert!(!is_soname("glibc"));
        assert!(!is_soname("sh"));
        assert!(!is_soname("libfoo.so"));
    }

    #[test]
    fn soname_rule_is_inert_without_a_local_database() {
        // load_installed() soft-fails to an empty map. nog must then behave
        // exactly as it did before v1.3.1 rather than holding everything.
        let ready = vec![pkg("libbluray", "1.4.1-1", "1.5.0-1", None, 0)];
        assert!(soname_coupling_demotions(&ready, &[], &SonameData::default()).is_empty());
    }

    #[test]
    fn pkgver_strips_epoch_and_pkgrel() {
        assert_eq!(pkgver("6.11.2-2"), "6.11.2");
        assert_eq!(pkgver("1:3.6.4-1"), "3.6.4");
        assert_eq!(pkgver("26.04.3-1"), "26.04.3");
        // The Qt6 case turns on this: -1 and -2 are the same upstream release.
        assert_eq!(pkgver("6.11.2-1"), pkgver("6.11.2-2"));
        // Degenerate inputs must not panic or truncate wrongly.
        assert_eq!(pkgver("1.0"), "1.0");
        assert_eq!(pkgver(""), "");
    }

    #[test]
    fn pkgbase_rule_couples_same_pkgbase_siblings() {
        // The 2026-08-23 incident: elfutils and libelf share pkgbase `elfutils`
        // and are joined by `libelf=0.196`. Holding one must hold the other.
        let ready = vec![pkg("elfutils", "0.195-1", "0.196-1", Some("elfutils"), 0)];
        let held = vec![pkg("libelf", "0.195-1", "0.196-1", Some("elfutils"), 6)];
        assert_eq!(demoted_names(pkgbase_coupling_demotions(&ready, &held)), vec!["elfutils"]);
    }

    #[test]
    fn pkgbase_rule_ignores_packages_without_a_pkgbase() {
        // Sync-DB entries can lack %BASE%. They simply do not participate.
        let ready = vec![pkg("orphan", "1.0-1", "1.1-1", None, 0)];
        let held = vec![pkg("other", "1.0-1", "1.1-1", None, 3)];
        assert!(pkgbase_coupling_demotions(&ready, &held).is_empty());
    }

    #[test]
    fn cohort_rule_catches_the_qt6_split() {
        // The 2026-08-25 incident, reduced. Every qt6 package is its own
        // pkgbase and the dependency on qt6-base is unversioned, so neither
        // exact rule sees anything. Only the shared 6.11.1 -> 6.11.2 move does.
        let ready = vec![
            pkg("qt6-declarative", "6.11.1-3", "6.11.2-1", Some("qt6-declarative"), 0),
            pkg("qt6-svg", "6.11.1-1", "6.11.2-1", Some("qt6-svg"), 0),
            pkg("qt6-5compat", "6.11.1-1", "6.11.2-1", Some("qt6-5compat"), 0),
        ];
        let held = vec![pkg("qt6-base", "6.11.1-1", "6.11.2-2", Some("qt6-base"), 1)];

        // The exact rules are blind to this — that is the whole point of the bug.
        assert!(pkgbase_coupling_demotions(&ready, &held).is_empty());

        assert_eq!(
            demoted_names(cohort_coupling_demotions(&ready, &held)),
            vec!["qt6-5compat", "qt6-declarative", "qt6-svg"]
        );
    }

    #[test]
    fn cohort_rule_is_silent_on_a_uniform_cohort() {
        // Regression fixture from the 08-25 run: 67 nerd fonts all moving
        // 3.5.0 -> 3.5.1, none held. A rule that fired here would be useless.
        let ready: Vec<CouplingPkg> = (0..67)
            .map(|i| pkg(&format!("ttf-{i}-nerd"), "3.5.0-1", "3.5.1-2", None, 0))
            .collect();
        assert!(cohort_coupling_demotions(&ready, &[]).is_empty());

        // ...and the same set entirely held is equally uninteresting.
        assert!(cohort_coupling_demotions(&[], &ready).is_empty());
    }

    #[test]
    fn cohort_rule_leaves_pairs_to_the_exact_rules() {
        // Two packages sharing a version is a coincidence often enough that the
        // heuristic stays out of it; pkgbase and lib32 handle real pairs exactly.
        let ready = vec![pkg("alpha", "1.0-1", "1.1-1", Some("alpha"), 0)];
        let held = vec![pkg("beta", "1.0-1", "1.1-1", Some("beta"), 4)];
        assert!(cohort_coupling_demotions(&ready, &held).is_empty());
    }

    #[test]
    fn cohort_rule_requires_the_same_destination() {
        // Three packages on 1.0 but heading to different releases are not a
        // family moving in step, however alike their current versions look.
        let ready = vec![
            pkg("a", "1.0-1", "1.1-1", None, 0),
            pkg("b", "1.0-1", "1.2-1", None, 0),
        ];
        let held = vec![pkg("c", "1.0-1", "1.3-1", None, 5)];
        assert!(cohort_coupling_demotions(&ready, &held).is_empty());
    }

    #[test]
    fn demoted_package_waits_on_the_longest_held_partner() {
        // Inheriting the *longest* countdown is what makes the group clear on a
        // single later run instead of dribbling out over several.
        let ready = vec![pkg("app", "5.0-1", "5.1-1", None, 0)];
        let held = vec![
            pkg("lib-a", "5.0-1", "5.1-1", None, 2),
            pkg("lib-b", "5.0-1", "5.1-1", None, 9),
            pkg("lib-c", "5.0-1", "5.1-1", None, 6),
        ];
        let d = cohort_coupling_demotions(&ready, &held);
        assert_eq!(d, vec![("app".to_string(), "lib-b".to_string())]);
    }

    #[test]
    fn a_package_is_claimed_by_only_one_rule() {
        // libelf is a lib32 partner *and* a pkgbase sibling *and* a cohort
        // member. It must appear once, or it would be demoted repeatedly.
        let ready = vec![
            pkg("libelf", "0.195-1", "0.196-1", Some("elfutils"), 0),
            pkg("elfutils", "0.195-1", "0.196-1", Some("elfutils"), 0),
        ];
        let held = vec![pkg("lib32-libelf", "0.195-1", "0.196-2", Some("lib32-libelf"), 3)];

        let d = coupling_demotions(&ready, &held, &SonameData::default());
        let names = demoted_names(d.clone());
        assert_eq!(names, vec!["elfutils", "libelf"]);
        assert_eq!(d.len(), 2, "each package demoted exactly once");
        // The exact lib32 rule wins the name over the cohort heuristic.
        assert_eq!(
            d.iter().find(|(n, _)| n == "libelf").map(|(_, p)| p.as_str()),
            Some("lib32-libelf")
        );
    }

    #[test]
    fn one_pass_is_not_enough_to_converge() {
        // The structural half of #11. `bar` is coupled to `foo` by pkgbase, but
        // on the first pass `foo` is still in Ready, so nothing links them. Only
        // after `foo` is demoted does `bar`'s coupling become visible. Versions
        // are chosen so the cohort rule cannot short-circuit the chain.
        let held = vec![pkg("lib32-foo", "1.0-1", "1.1-1", Some("lib32-foo"), 5)];
        let ready = vec![
            pkg("foo", "2.0-1", "2.1-1", Some("foo-base"), 0),
            pkg("bar", "3.0-1", "3.1-1", Some("foo-base"), 0),
        ];

        let first = coupling_demotions(&ready, &held, &SonameData::default());
        assert_eq!(demoted_names(first), vec!["foo"], "pass 1 sees only the lib32 pair");

        // Apply it, exactly as commands::update does, and look again.
        let held2 = vec![
            held[0].clone(),
            pkg("foo", "2.0-1", "2.1-1", Some("foo-base"), 5),
        ];
        let ready2 = vec![ready[1].clone()];

        let second = coupling_demotions(&ready2, &held2, &SonameData::default());
        assert_eq!(
            demoted_names(second),
            vec!["bar"],
            "pass 2 finds what the demotion in pass 1 created"
        );
    }

    #[test]
    fn coupling_reaches_a_fixpoint() {
        // Whatever the rules do, iterating must terminate with Ready and Held
        // partitioning the original set — no package lost, none duplicated.
        let mut ready = vec![
            pkg("qt6-declarative", "6.11.1-3", "6.11.2-1", Some("qt6-declarative"), 0),
            pkg("qt6-svg", "6.11.1-1", "6.11.2-1", Some("qt6-svg"), 0),
            pkg("qt6-tools", "6.11.1-4", "6.11.2-1", Some("qt6-tools"), 0),
            pkg("unrelated", "9.9-1", "9.9-2", Some("unrelated"), 0),
        ];
        let mut held = vec![pkg("qt6-base", "6.11.1-1", "6.11.2-2", Some("qt6-base"), 1)];
        let total = ready.len() + held.len();

        let mut passes = 0;
        loop {
            let d = coupling_demotions(&ready, &held, &SonameData::default());
            if d.is_empty() {
                break;
            }
            passes += 1;
            assert!(passes < 16, "coupling failed to converge");
            let names: HashSet<&str> = d.iter().map(|(n, _)| n.as_str()).collect();
            let (move_out, keep): (Vec<_>, Vec<_>) =
                ready.drain(..).partition(|p| names.contains(p.name.as_str()));
            ready = keep;
            held.extend(move_out);
        }

        assert_eq!(ready.len() + held.len(), total, "no package lost or duplicated");
        assert_eq!(demoted_names(ready.iter().map(|p| (p.name.clone(), String::new())).collect()),
                   vec!["unrelated"], "only the unrelated package still releases");
        assert_eq!(held.len(), 4, "the whole qt6 cohort moved together");
    }
}