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
    /// v1.6.0 (#27): the window — clocked from the first new version seen —
    /// is over, but the newest build is younger than the tier's safety wait.
    /// Held for `days_remaining` more days, unless a newer build replaces it,
    /// which does not restart the window, only this short wait.
    SafetyWait { days_remaining: u64 },
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
///
/// v1.6.0 (#27): `first_seen` is when an update for the installed version was
/// first seen (see `sightings`). The window is clocked from the earlier of that
/// and the candidate's own build date, so a newer build no longer restarts the
/// countdown. Once the window is over, the candidate must still be at least the
/// tier's safety wait old. With `first_seen = None` this is the old rule
/// exactly, because the safety wait is never longer than a window it follows.
pub fn evaluate_candidate(
    package: &str,
    tier: Tier,
    candidate_version: &str,
    packages: &HashMap<String, PackageDesc>,
    holds: &HoldsConfig,
    first_seen: Option<u64>,
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

    let anchor = first_seen.map_or(desc.builddate, |f| f.min(desc.builddate));
    evaluate_since(anchor, desc.builddate, tier, holds, now)
}

/// The #27 rule: window from `anchor` (the first sighting), then a safety
/// wait on the candidate's own build date.
pub fn evaluate_since(
    anchor: u64,
    candidate_built: u64,
    tier: Tier,
    holds: &HoldsConfig,
    now: SystemTime,
) -> HoldStatus {
    let window = evaluate_ts(anchor, tier.clone(), holds, now);
    if !matches!(window, HoldStatus::Expired { .. }) {
        return window;
    }
    let now_ts = match now.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => return HoldStatus::Unknown,
    };
    let age = days_ceil(now_ts.saturating_sub(candidate_built));
    let safety = match tier {
        Tier::One => holds.tier1_safety_days,
        Tier::Two => holds.tier2_safety_days,
        Tier::Three => holds.tier3_safety_days,
    } as u64;
    if age >= safety {
        window
    } else {
        HoldStatus::SafetyWait { days_remaining: safety - age }
    }
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
    /// soname -> installed packages whose binaries link it, read from the
    /// files rather than the declarations (v1.4.2, issue #16). Filled only
    /// for sonames some pending update drops; see `dropped_sonames`.
    pub linked_by: HashMap<String, Vec<String>>,
    /// soname -> installed packages shipping the library file on the loader
    /// path, declared or not (issue #16).
    pub file_providers: HashMap<String, Vec<String>>,
    /// v2.1.0 (F-25, #61): interpreter -> installed packages with files under its
    /// CURRENT series' module folder (`usr/lib/perl5/5.42/`). Filled only when a
    /// pending update moves that interpreter to a new series; see `interpreter_moves`.
    pub module_dir_owners: HashMap<String, Vec<String>>,
    /// v2.1.1 (F-30, #67): candidate -> `%DEPENDS%` of the version that would be
    /// installed. What the reverse soname rule reads for a declared need.
    pub new_depends: HashMap<String, Vec<String>>,
    /// v2.1.1 (F-30, #67): candidate -> build date of the version that would be
    /// installed. Tells a rebuild made after a new soname existed from one made
    /// before it, for the undeclared need.
    pub new_built: HashMap<String, u64>,
    /// v2.1.3 (F-32, #69): candidate -> versioned libraries it ships today
    /// (`libsimdjson.so=34-64`), read from its file list, for candidates whose
    /// upstream version changes. What the undeclared soname rule reads.
    pub shipped_libs: HashMap<String, Vec<String>>,
}

/// Every soname some pending candidate would stop providing.
///
/// This is the trigger for the file scan in issue #16, computed once per run
/// over *all* pending candidates — Ready and Held — because the fixpoint loop
/// can move a candidate between buckets but never changes what it provides.
/// Measured at about one pending update in 130; on most runs it is empty and
/// nothing is scanned.
pub fn dropped_sonames(data: &SonameData, candidates: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = candidates
        .iter()
        // A candidate with no known future (an AUR package: the sync
        // database does not describe it) has not dropped anything nog can
        // see. Treating unknown as empty would scan on every run.
        .filter(|c| data.new_provides.contains_key(**c))
        .flat_map(|c| {
            let before = soname_set(data.installed_provides.get(*c));
            let after = soname_set(data.new_provides.get(*c));
            before
                .difference(&after)
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
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
                .unwrap_or(false)
                // Issue #16: a package that ships the file without declaring
                // it keeps the library on disk just the same, as long as it
                // is not the one moving.
                || data
                    .file_providers
                    .get(*dropped)
                    .map(|ps| ps.iter().any(|p| p != &cand.name && !moving.contains(p.as_str())))
                    .unwrap_or(false);
            if survives {
                continue;
            }

            // Everyone still requiring it who is not moving alongside —
            // declared in `%DEPENDS%`, or (issue #16) linked in a binary.
            let mut broken: Vec<&str> = requirers
                .get(dropped)
                .into_iter()
                .flatten()
                .copied()
                .chain(
                    data.linked_by
                        .get(*dropped)
                        .into_iter()
                        .flatten()
                        .map(|s| s.as_str()),
                )
                .filter(|r| !moving.contains(r) && *r != cand.name)
                .collect();
            if broken.is_empty() {
                continue;
            }
            broken.sort_unstable();
            broken.dedup();

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

/// Is this an exact versioned entry (`libcrypto.so=4-64`, `java-runtime=21`)?
/// Ranges are left out for the reason `is_soname` gives.
fn is_versioned(entry: &str) -> bool {
    entry.contains('=') && !entry.contains('<') && !entry.contains('>')
}

/// A soname entry split into its library and its word size:
/// `libcrypto.so=3-64` -> (`libcrypto.so`, `64`). The word size is what keeps
/// a 32-bit bump from ever standing in for a 64-bit one.
fn soname_lib_and_arch(entry: &str) -> Option<(&str, &str)> {
    let (base, ver) = entry.split_once('=')?;
    let (_, arch) = ver.rsplit_once('-')?;
    Some((base, arch))
}

/// Couple a Ready package to the held package that alone supplies a library
/// its new version needs (F-30, #67): the soname rule turned around.
///
/// Seen on 2026-11-10: `openssl` 3.6.5 -> 4.1.0 was held (coupled to `kmod`
/// by the soname rule), while Arch's rebuilds against OpenSSL 4 had cleared
/// their own windows and sat in Ready. `curl` 8.23.0-2 declares
/// `libcrypto.so=4-64`, which only the held openssl 4.1.0 provides, so pacman
/// refused the whole transaction. The soname rule asks whether a Ready package
/// drops something still needed; nothing asked whether one needs something
/// not yet there.
///
/// Two ways a need is known:
///
/// * **Declared**: an exact versioned entry in the candidate's new
///   `%DEPENDS%`. Exact, straight from the metadata, and what pacman itself
///   refuses on.
/// * **Undeclared**: `git`, `krb5`, `libssh2` were rebuilt for OpenSSL 4 too
///   and say only `openssl`, or nothing. pacman would let them through and
///   they would land linking `libcrypto.so.4` beside a `.so.3`: ssh and git
///   broken at run time, which is worse than a refused update. So when a held
///   package **bumps** a soname (drops `libcrypto.so=3-64`, adds
///   `libcrypto.so=4-64`), every Ready candidate that uses the old one today
///   (declared, or linked per the #16 scan) and whose new build is no older
///   than the library's first new build is taken to need the new one. The
///   linchpin rule's reasoning (#5): built after it existed, probably linked
///   to it. It errs toward a few days' wait, never toward a broken system.
///
/// Either way, a need that something staying or something moving will still
/// provide holds nothing; only a need that a held candidate alone supplies
/// couples, to that candidate, which lends its countdown.
pub fn needs_coupling_demotions(
    ready: &[CouplingPkg],
    held: &[CouplingPkg],
    data: &SonameData,
) -> Vec<(String, String)> {
    if ready.is_empty() || held.is_empty() || data.installed_provides.is_empty() {
        return Vec::new();
    }
    let moving: HashSet<&str> = ready.iter().map(|p| p.name.as_str()).collect();

    // Will anything not held provide `need` after this run?
    let provided = |need: &str| {
        data.installed_provides.iter().any(|(p, list)| {
            !moving.contains(p.as_str()) && list.iter().any(|e| e == need)
        }) || ready.iter().any(|r| {
            data.new_provides.get(&r.name).is_some_and(|l| l.iter().any(|e| e == need))
        })
    };

    // The soname bumps held packages carry: old -> (new, library). The first
    // new build of each library dates when anything could link the new one;
    // `lib32-openssl` was built later than `openssl`, and the 32-bit
    // rebuilds were made against the same release.
    let mut bumps: Vec<(&str, &str)> = Vec::new();
    let mut first_built: HashMap<&str, u64> = HashMap::new();
    for h in held {
        let before = soname_set(data.installed_provides.get(&h.name));
        let after = soname_set(data.new_provides.get(&h.name));
        for old in before.difference(&after) {
            let Some((lib, arch)) = soname_lib_and_arch(old) else { continue };
            let Some(new) = after
                .iter()
                .find(|n| soname_lib_and_arch(n) == Some((lib, arch)))
            else {
                continue;
            };
            bumps.push((old, new));
            if let Some(b) = data.new_built.get(&h.name) {
                first_built.entry(lib).and_modify(|t| *t = (*t).min(*b)).or_insert(*b);
            }
        }
    }

    let mut out: Vec<(String, String)> = Vec::new();
    for cand in ready {
        let declared = data
            .new_depends
            .get(&cand.name)
            .into_iter()
            .flatten()
            .map(|s| s.as_str())
            .filter(|s| is_versioned(s));
        let uses = |old: &str| {
            data.installed_depends.get(&cand.name).is_some_and(|l| l.iter().any(|e| e == old))
                || data.linked_by.get(old).is_some_and(|l| l.iter().any(|p| p == &cand.name))
        };
        let undeclared = bumps.iter().filter_map(|(old, new)| {
            let lib = soname_lib_and_arch(old)?.0;
            let since = *first_built.get(lib)?;
            let built = *data.new_built.get(&cand.name)?;
            (built >= since && uses(old)).then_some(*new)
        });

        for need in declared.chain(undeclared) {
            if provided(need) {
                continue;
            }
            let suppliers: Vec<&CouplingPkg> = held
                .iter()
                .filter(|h| data.new_provides.get(&h.name).is_some_and(|l| l.iter().any(|e| e == need)))
                .collect();
            if let Some(h) = longest_held(&suppliers) {
                out.push((cand.name.clone(), h.name.clone()));
                break; // one note per candidate, as the soname rule
            }
        }
    }
    out
}

/// The name a dependency entry asks for, its version constraint dropped:
/// `openssl-3`, `libcrypto.so=4-64` -> `libcrypto.so`, `glibc>=2.44` -> `glibc`.
fn dep_name(entry: &str) -> &str {
    entry.split(['<', '>', '=']).next().unwrap_or(entry)
}

/// Couple a Ready package to the held package whose files a package it pulls
/// in would take over (F-31, #68).
///
/// Seen on 2026-11-11, Javier's run of 2.1.1: openssl 3.6.5 -> 4.1.0 held,
/// and with it everything rebuilt for OpenSSL 4. But Arch also rebuilt
/// `dotnet-runtime` to need `openssl-3`, a brand-new compat package carrying
/// the OpenSSL 3 libraries. `dotnet-runtime` was Ready, so pacman pulled
/// `openssl-3` in, and refused everything:
///
/// ```text
/// openssl-3: /usr/lib/libcrypto.so.3 exists in filesystem (owned by openssl)
/// ```
///
/// Those files move from `openssl` to `openssl-3` only when openssl itself
/// moves to 4.1.0, which no longer ships them. No rule saw it: the need is a
/// package name, not a soname, and the package is not installed at all.
///
/// The files are not in the sync database, so the sign read is the provides:
/// a package the update would install new (a dependency nothing installed
/// satisfies, followed through its own dependencies) that provides a soname a
/// held package's installed version provides and its new one drops is that
/// library's new home. It can only arrive in the same run as the held package
/// that leaves, so the Ready package pulling it in waits for that one and
/// shares its countdown. Its pkgbase siblings (`dotnet-host`) follow through
/// the fixpoint.
pub fn pulled_in_coupling_demotions(
    ready: &[CouplingPkg],
    held: &[CouplingPkg],
    data: &SonameData,
) -> Vec<(String, String)> {
    if ready.is_empty() || held.is_empty() || data.installed_provides.is_empty() {
        return Vec::new();
    }

    // What each held package leaves behind, by soname.
    let dropped: Vec<(&CouplingPkg, HashSet<&str>)> = held
        .iter()
        .filter_map(|h| {
            let before = soname_set(data.installed_provides.get(&h.name));
            let after = soname_set(data.new_provides.get(&h.name));
            let gone: HashSet<&str> = before.difference(&after).copied().collect();
            (!gone.is_empty()).then_some((h, gone))
        })
        .collect();
    if dropped.is_empty() {
        return Vec::new();
    }

    // Does something installed already answer this dependency? Then pacman
    // pulls nothing new for it.
    let satisfied = |entry: &str| {
        let name = dep_name(entry);
        let exact = is_versioned(entry);
        data.installed_provides.contains_key(name) && !exact
            || data.installed_provides.values().flatten().any(|e| {
                if exact { e == entry } else { dep_name(e) == name }
            })
    };
    // The packages pacman would install new for this dependency: the one of
    // that name, or failing that whatever provides it.
    fn pulled_for<'a>(data: &'a SonameData, entry: &str) -> Vec<&'a str> {
        let name = dep_name(entry);
        if let Some((k, _)) = data.new_provides.get_key_value(name) {
            if !data.installed_provides.contains_key(name) {
                return vec![k.as_str()];
            }
        }
        data.new_provides
            .iter()
            .filter(|(p, list)| {
                !data.installed_provides.contains_key(p.as_str())
                    && list.iter().any(|e| if is_versioned(entry) { e == entry } else { dep_name(e) == name })
            })
            .map(|(p, _)| p.as_str())
            .collect()
    }

    let mut out: Vec<(String, String)> = Vec::new();
    for cand in ready {
        // Everything this candidate's new version would bring in, followed
        // through the new packages' own dependencies.
        let mut seen: HashSet<&str> = HashSet::new();
        let mut queue: Vec<&str> = data
            .new_depends
            .get(&cand.name)
            .into_iter()
            .flatten()
            .filter(|e| !satisfied(e))
            .flat_map(|e| pulled_for(data, e))
            .collect();
        let mut takers: Vec<&CouplingPkg> = Vec::new();
        while let Some(n) = queue.pop() {
            if !seen.insert(n) || seen.len() > 64 {
                continue;
            }
            let provides = soname_set(data.new_provides.get(n));
            for (h, gone) in &dropped {
                if !provides.is_disjoint(gone) {
                    takers.push(h);
                }
            }
            queue.extend(
                data.new_depends
                    .get(n)
                    .into_iter()
                    .flatten()
                    .filter(|e| !satisfied(e))
                    .flat_map(|e| pulled_for(data, e)),
            );
        }
        if let Some(h) = longest_held(&takers) {
            out.push((cand.name.clone(), h.name.clone()));
        }
    }
    out
}

/// Pure. Of the libraries a package ships today, those it does not declare: no
/// `%PROVIDES%` entry for the same library at the same word size. A declared
/// library is the soname rule's (#13), which reads the new version's provides;
/// only for an undeclared one is there nothing to compare against.
pub fn undeclared_libs(shipped: &[String], declared: &[String]) -> Vec<String> {
    shipped
        .iter()
        .filter(|s| {
            let Some(key) = soname_lib_and_arch(s) else { return false };
            !declared.iter().any(|d| soname_lib_and_arch(d) == Some(key))
        })
        .cloned()
        .collect()
}

/// Couple a Ready library package that declares no sonames to the held
/// package rebuilt for its new version (F-32, #69).
///
/// Seen on 2026-11-11, Javier's run of 2.1.2: simdjson 1:5.0.5 -> 2:5.1.1
/// went in (Ready) and replaced `libsimdjson.so.34` with `.so.36`, while
/// nodejs, its only user, was held with openssl. `node` stopped starting:
/// *"libsimdjson.so.34: cannot open shared object file"*. The soname rule
/// (#13) compares the provides of the installed and the new version, and
/// simdjson declares none, so it saw no library go; the sync database does
/// not list files, so the new `.so` number is not knowable in advance.
///
/// What is knowable: the library files the package ships today (its file
/// list), who links them (the #16 scan), and build dates. A Ready package
/// whose upstream version changes, shipping a library it does not declare,
/// waits for a held package that links that library today and whose new
/// build is no older than the library's: rebuilt after it, so most likely
/// rebuilt for it (nodejs 26.13.1 was built a week after simdjson 5.1.1).
/// A user with no update pending, or one built before, holds nothing: Arch
/// rebuilds every user of a library whose number changes. It errs toward a
/// wait when the number did not change, never toward a broken program.
pub fn undeclared_soname_demotions(
    ready: &[CouplingPkg],
    held: &[CouplingPkg],
    data: &SonameData,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for cand in ready {
        let Some(libs) = data.shipped_libs.get(&cand.name) else { continue };
        if upstream(&cand.old_version) == upstream(&cand.new_version) {
            continue;
        }
        let declared = data.installed_provides.get(&cand.name).cloned().unwrap_or_default();
        let libs = undeclared_libs(libs, &declared);
        let Some(built) = data.new_built.get(&cand.name) else { continue };
        let users: Vec<&CouplingPkg> = held
            .iter()
            .filter(|h| h.name != cand.name)
            .filter(|h| data.new_built.get(&h.name).is_some_and(|b| b >= built))
            .filter(|h| {
                libs.iter().any(|l| data.linked_by.get(l).is_some_and(|by| by.contains(&h.name)))
            })
            .collect();
        if let Some(h) = longest_held(&users) {
            out.push((cand.name.clone(), h.name.clone()));
        }
    }
    out
}

/// A pacman version without its pkgrel, the epoch kept: `1:5.0.5-1` ->
/// `1:5.0.5`. A rebuild (pkgrel only) never changes what a library is called.
fn upstream(version: &str) -> &str {
    version.rsplit_once('-').map(|(v, _)| v).unwrap_or(version)
}

/// v2.1.0 (F-25, #61): the interpreters whose compiled modules install into a
/// folder named for the interpreter's version series — `usr/lib/perl5/5.42/`,
/// `usr/lib/python3.14/` — with how many version components make the series.
pub const INTERPRETERS: [(&str, &str, usize); 2] = [("perl", "usr/lib/perl5/", 2), ("python", "usr/lib/python", 2)];

/// Pure. The module folder an interpreter at `version` reads (`usr/lib/perl5/5.42/`), or
/// `None` when `name` is not one of `INTERPRETERS`.
pub fn module_dir(name: &str, version: &str) -> Option<String> {
    INTERPRETERS.iter().find(|(n, _, _)| *n == name)
        .map(|(_, prefix, depth)| format!("{}{}/", prefix, series(version, *depth)))
}

/// Pure. Of the pending updates `(name, old, new)`, the interpreters that change
/// series, each with the folder it leaves behind. Empty on most runs, so the file
/// lists are read only when this is not.
pub fn interpreter_moves(pending: &[(&str, &str, &str)]) -> Vec<(String, String)> {
    pending.iter().filter_map(|(n, old, new)| {
        let (from, to) = (module_dir(n, old)?, module_dir(n, new)?);
        (from != to).then(|| (n.to_string(), from))
    }).collect()
}

/// Couple an interpreter that changes series to the installed packages still
/// using its old module folder (F-25, #61).
///
/// Seen on 2026-10-23: perl went 5.42 → 5.44 when its hold ended, while
/// apparmor and subversion, already rebuilt for 5.44, were held a day longer
/// (built after perl, so their windows end later). perl's own hook said
/// `'/usr/lib/perl5/5.42' contains data from at least 2 packages which will NOT
/// be used`. Arch states this link nowhere but in the paths: no versioned
/// dependency, no ABI provide. python has the same shape, far larger.
///
/// The soname rule's direction: a Ready interpreter that leaves its folder
/// behind waits for every installed package with files there that is not
/// moving with it. A held one lends its countdown; one with no update at all
/// (not rebuilt yet, or foreign) reads `blocked by`. `--promote` is the door.
pub fn module_dir_coupling_demotions(
    ready: &[CouplingPkg],
    held: &[CouplingPkg],
    data: &SonameData,
) -> Vec<(String, String)> {
    let moving: HashSet<&str> = ready.iter().map(|p| p.name.as_str()).collect();
    let mut out = Vec::new();
    for cand in ready {
        let (Some(from), Some(to)) = (module_dir(&cand.name, &cand.old_version), module_dir(&cand.name, &cand.new_version)) else { continue };
        if from == to {
            continue;
        }
        let mut left: Vec<&str> = data.module_dir_owners.get(&cand.name).into_iter().flatten()
            .map(|s| s.as_str())
            .filter(|o| *o != cand.name && !moving.contains(o))
            .collect();
        if left.is_empty() {
            continue;
        }
        left.sort_unstable();
        left.dedup();
        // The same partner choice as the soname rule: a held one, so both rows
        // share a countdown; otherwise the first by name (`blocked by`).
        let held_match: Vec<&CouplingPkg> = held.iter().filter(|h| left.contains(&h.name.as_str())).collect();
        let partner = longest_held(&held_match).map(|h| h.name.clone()).unwrap_or_else(|| left[0].to_string());
        out.push((cand.name.clone(), partner));
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
        .chain(needs_coupling_demotions(ready, held, soname))
        .chain(pulled_in_coupling_demotions(ready, held, soname))
        .chain(undeclared_soname_demotions(ready, held, soname))
        .chain(module_dir_coupling_demotions(ready, held, soname))
    {
        if claimed.insert(name.clone()) {
            out.push((name, partner));
        }
    }
    out
}

/// Convert seconds to days, rounding **up**. 0s -> 0d, 1s -> 1d, 86400s -> 1d,
/// 86401s -> 2d. Matches the spec's "4.x is 5 automatically" rule.
/// v2.0.0 (#5): the linchpins — the foundation packages a fast-lane package is
/// compiled against. When one of them has a pending update that changes its
/// **series** (glibc 2.44 → 2.45: new `GLIBC_2.45` symbol versions; gcc-libs
/// 15 → 16: new `GLIBCXX_3.4.xx` ones), anything built after that new build
/// was probably linked against it and would fail at runtime with
/// `GLIBC_2.45 not found` if it landed while the linchpin is still held — the
/// 2026-08-04 spectacle breakage. The number is how many leading version
/// components make the series.
pub const LINCHPINS: [(&str, usize); 2] = [("glibc", 2), ("gcc-libs", 1)];

/// Pure. The series of a pacman version: the first `depth` dotted components
/// of its pkgver, epoch and pkgrel dropped — `2.44+r24+g16be…-1` → `2.44`,
/// `15.2.1+r22-1` with depth 1 → `15`.
pub fn series(version: &str, depth: usize) -> String {
    let v = pkgver(version);
    let numeric: String = v.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    numeric.split('.').filter(|s| !s.is_empty()).take(depth.max(1)).collect::<Vec<_>>().join(".")
}

/// A pending update as the linchpin rule sees it: its name, the versions, when
/// the candidate was built, and (held ones) the days left on its hold.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    pub name: String,
    pub old_version: String,
    pub new_version: String,
    pub built: u64,
    pub remaining: u64,
}

/// Pure. The Ready packages to hold back with a held linchpin, each with the
/// linchpin's name and its countdown. Fires only when the linchpin's pending
/// update changes its series — a same-series rebuild (2.44 → 2.44, most
/// months) cannot add a symbol version, so nothing moves — and only for
/// candidates built **after** the new linchpin existed. Measured on the test
/// desktop on 2026-10-14: 0 of 160 pending packages would have moved (glibc
/// was a same-series rebuild), which is right.
pub fn linchpin_demotions(ready: &[Built], held: &[Built]) -> Vec<(String, String, u64)> {
    let mut out = Vec::new();
    for (name, depth) in LINCHPINS {
        let Some(l) = held.iter().find(|h| h.name == name) else { continue };
        if series(&l.old_version, depth) == series(&l.new_version, depth) {
            continue;
        }
        for r in ready {
            if r.built > l.built && !out.iter().any(|(n, _, _): &(String, String, u64)| n == &r.name) {
                out.push((r.name.clone(), l.name.clone(), l.remaining));
            }
        }
    }
    out
}

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
            tier1_safety_days: 7,
            tier2_safety_days: 3,
            tier3_safety_days: 1,
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
            depends: Vec::new(),
            repo: None,
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
            None,
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
            None,
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
            None,
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
            None,
            at_days_after_epoch(10),
        );
        assert_eq!(got, HoldStatus::Unknown);
    }

    // --- v1.6.0: a new build no longer restarts the hold (issue #27) ---

    #[test]
    fn the_treadmill_never_released_under_the_old_rule() {
        // linux-zen, Tier 1 (30 days): a new build every 5 days for 60 days.
        // Clocked from the newest build, it is always <= 5 days old -> held.
        for day in (0..60).step_by(5) {
            let pkgs = pkg_map("linux-zen", day * SECONDS_PER_DAY, Some("v"));
            let got = evaluate_candidate("linux-zen", Tier::One, "v", &pkgs, &holds_default(), None,
                at_days_after_epoch(day + 4));
            assert!(matches!(got, HoldStatus::Holding { .. }), "day {day}: {got:?}");
        }
    }

    #[test]
    fn counted_from_the_first_sighting_it_is_released() {
        // First seen day 0; today day 61; newest build day 52 (9 days old).
        // Window 30 over, safety wait 7 passed -> Ready. The #27 worked example.
        let pkgs = pkg_map("linux-zen", 52 * SECONDS_PER_DAY, Some("7.2.7"));
        let got = evaluate_candidate("linux-zen", Tier::One, "7.2.7", &pkgs, &holds_default(),
            Some(0), at_days_after_epoch(61));
        assert_eq!(got, HoldStatus::Expired { days_past_window: 31 });
    }

    #[test]
    fn a_build_from_yesterday_waits_out_the_safety_wait() {
        // Window over (first seen day 0, today day 40), but the newest build
        // is from day 38 -> 2 days old, Tier 1 safety 7 -> 5 more days.
        let pkgs = pkg_map("mesa", 38 * SECONDS_PER_DAY, Some("26.2.3"));
        let got = evaluate_candidate("mesa", Tier::One, "26.2.3", &pkgs, &holds_default(),
            Some(0), at_days_after_epoch(40));
        assert_eq!(got, HoldStatus::SafetyWait { days_remaining: 5 });
    }

    #[test]
    fn the_window_still_runs_from_the_first_sighting() {
        // First seen day 10, today day 25, Tier 1: 15 days left on the window,
        // whatever the newest build's date.
        let pkgs = pkg_map("systemd", 24 * SECONDS_PER_DAY, Some("262"));
        let got = evaluate_candidate("systemd", Tier::One, "262", &pkgs, &holds_default(),
            Some(10 * SECONDS_PER_DAY), at_days_after_epoch(25));
        assert_eq!(got, HoldStatus::Holding { days_remaining: 15 });
    }

    #[test]
    fn a_later_sighting_never_makes_a_hold_longer_than_before() {
        // A record newer than the candidate's own build (e.g. first run after
        // an upgrade of nog) is clamped to the build date: old rule at worst.
        let pkgs = pkg_map("vim", 0, Some("9.2"));
        let got = evaluate_candidate("vim", Tier::Three, "9.2", &pkgs, &holds_default(),
            Some(15 * SECONDS_PER_DAY), at_days_after_epoch(20));
        assert_eq!(got, HoldStatus::Expired { days_past_window: 13 });
    }

    #[test]
    fn the_version_guard_still_wins() {
        let pkgs = pkg_map("lib32-brotli", 0, Some("1.1.0-1"));
        let got = evaluate_candidate("lib32-brotli", Tier::Three, "1.2.0-2", &pkgs, &holds_default(),
            Some(0), at_days_after_epoch(975));
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

    // ---- issue #16: linkage read from the binaries ------------------------
    //
    // The 2026-08-29 break, as it stood that day: libbluray 1.5.0-1 Ready
    // (soname 3 -> 4), ffmpeg-obs Held with 4 days left, and ffmpeg-obs's
    // %DEPENDS% saying plain `libbluray` with no soname at all.

    fn bluray_incident() -> (Vec<CouplingPkg>, Vec<CouplingPkg>, SonameData) {
        let data = sd(
            &[
                ("libbluray", &["libbluray.so=3-64"], &[]),
                ("ffmpeg-obs", &["libavformat.so=63-64"], &["libbluray"]),
            ],
            &[("libbluray", &["libbluray.so=4-64"])],
        );
        let ready = vec![pkg("libbluray", "1.4.1-1", "1.5.0-1", None, 0)];
        let held = vec![pkg("ffmpeg-obs", "9.0-1", "9.0.1-1.2", None, 4)];
        (ready, held, data)
    }

    #[test]
    fn declarations_alone_miss_the_ffmpeg_obs_break() {
        // The failing direction first: without the binary scan, the v1.3.1
        // rule sees nothing. If this ever starts demoting, the fixture no
        // longer describes the bug and the test below proves nothing.
        let (ready, held, data) = bluray_incident();
        assert!(soname_coupling_demotions(&ready, &held, &data).is_empty());
    }

    #[test]
    fn the_binary_scan_catches_the_ffmpeg_obs_break() {
        let (ready, held, mut data) = bluray_incident();
        data.linked_by
            .insert("libbluray.so=3-64".into(), vec!["ffmpeg-obs".into()]);
        assert_eq!(
            soname_coupling_demotions(&ready, &held, &data),
            vec![("libbluray".to_string(), "ffmpeg-obs".to_string())]
        );
    }

    #[test]
    fn an_undeclared_copy_of_the_library_keeps_it_alive() {
        // A compat package ships libbluray.so.3 without declaring the provide.
        // It stays, so the old soname stays, and nothing needs holding.
        let (ready, held, mut data) = bluray_incident();
        data.linked_by
            .insert("libbluray.so=3-64".into(), vec!["ffmpeg-obs".into()]);
        data.file_providers.insert(
            "libbluray.so=3-64".into(),
            vec!["libbluray".into(), "libbluray-compat".into()],
        );
        assert!(soname_coupling_demotions(&ready, &held, &data).is_empty());
    }

    #[test]
    fn the_moving_library_does_not_count_as_its_own_survivor() {
        // libbluray owns usr/lib/libbluray.so.3 today, but it is the one
        // moving away from it. Counting it would switch the rule off.
        let (ready, held, mut data) = bluray_incident();
        data.linked_by
            .insert("libbluray.so=3-64".into(), vec!["ffmpeg-obs".into()]);
        data.file_providers
            .insert("libbluray.so=3-64".into(), vec!["libbluray".into()]);
        assert_eq!(soname_coupling_demotions(&ready, &held, &data).len(), 1);
    }

    #[test]
    fn dropped_sonames_is_the_scan_trigger() {
        let (_, _, data) = bluray_incident();
        assert_eq!(
            dropped_sonames(&data, &["libbluray", "ffmpeg-obs"]),
            vec!["libbluray.so=3-64".to_string()]
        );
        // ffmpeg-obs above had no known new version; an unknown future is
        // not a drop. With one that keeps its soname, still nothing.
        let mut d2 = sd(
            &[("ffmpeg-obs", &["libavformat.so=63-64"], &[])],
            &[("ffmpeg-obs", &["libavformat.so=63-64"])],
        );
        assert!(dropped_sonames(&d2, &["ffmpeg-obs"]).is_empty());
        d2.new_provides.insert("ffmpeg-obs".into(), vec![]);
        assert_eq!(dropped_sonames(&d2, &["ffmpeg-obs"]).len(), 1);
        // Nothing dropped, nothing to scan.
        let quiet = sd(
            &[("zlib", &["libz.so=1-64"], &[])],
            &[("zlib", &["libz.so=1-64"])],
        );
        assert!(dropped_sonames(&quiet, &["zlib"]).is_empty());
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

    fn built(name: &str, old: &str, new: &str, built: u64, remaining: u64) -> Built {
        Built { name: name.into(), old_version: old.into(), new_version: new.into(), built, remaining }
    }

    #[test]
    fn a_series_is_the_leading_components_of_the_pkgver() {
        assert_eq!(series("2.44+r24+g16be1518495f-1", 2), "2.44");
        assert_eq!(series("2.45-1", 2), "2.45");
        assert_eq!(series("15.2.1+r22+gabc-1", 1), "15");
        assert_eq!(series("1:2.44-3", 2), "2.44", "the epoch is not part of it");
        assert_eq!(series("16-1", 1), "16");
    }

    #[test]
    fn a_same_series_rebuild_of_glibc_holds_nothing() {
        // The 2026-10-14 measurement: glibc 2.44 → 2.44 (a point rebuild), built after everything.
        let held = vec![built("glibc", "2.44+r24-1", "2.44+r50-1", 1_000, 13)];
        let ready = vec![built("ffmpeg", "7-1", "8-1", 2_000, 0), built("gcc-libs", "15.1-1", "15.2-1", 3_000, 0)];
        assert!(linchpin_demotions(&ready, &held).is_empty());
    }

    #[test]
    fn a_glibc_series_change_holds_what_was_built_after_it() {
        // The 2026-08-04 spectacle case: ffmpeg and gcc-libs built against the new glibc.
        let held = vec![built("glibc", "2.43-1", "2.44-1", 1_000, 22)];
        let ready = vec![
            built("ffmpeg", "7-1", "8-1", 2_000, 0),
            built("gcc-libs", "15.1-1", "15.2-1", 3_000, 0),
            built("cowsay", "3.8.3-1", "3.8.4-1", 500, 0),   // built BEFORE the new glibc: untouched
        ];
        let got = linchpin_demotions(&ready, &held);
        assert_eq!(got, vec![
            ("ffmpeg".to_string(), "glibc".to_string(), 22),
            ("gcc-libs".to_string(), "glibc".to_string(), 22),
        ]);
    }

    #[test]
    fn gcc_libs_is_a_linchpin_by_its_major_and_a_package_is_claimed_once() {
        let held = vec![built("glibc", "2.43-1", "2.44-1", 1_000, 22), built("gcc-libs", "15.2-1", "16.1-1", 1_500, 9)];
        let ready = vec![built("spectacle", "1-1", "2-1", 2_000, 0)];
        let got = linchpin_demotions(&ready, &held);
        assert_eq!(got.len(), 1, "claimed by the first linchpin only");
        assert_eq!(got[0], ("spectacle".to_string(), "glibc".to_string(), 22));
        let held = vec![built("gcc-libs", "15.2-1", "16.1-1", 1_500, 9)];
        assert_eq!(linchpin_demotions(&ready, &held), vec![("spectacle".to_string(), "gcc-libs".to_string(), 9)]);
    }

    #[test]
    fn a_promoted_or_ready_linchpin_holds_nothing() {
        // glibc is not in Held (promoted, or its window is over): the rule has no anchor.
        let ready = vec![built("glibc", "2.43-1", "2.44-1", 1_000, 0), built("ffmpeg", "7-1", "8-1", 2_000, 0)];
        assert!(linchpin_demotions(&ready, &[]).is_empty());
    }

    // ---- F-25 (#61): interpreters and their module folders ----------------
    // Real data from 2026-10-23: perl 5.42.3-1 → 5.44.0-1 Ready, apparmor
    // 4.1.8-2 and subversion 1.14.5-6 (rebuilt for 5.44) held 1 day.

    fn cp(name: &str, old: &str, new: &str, remaining: u64) -> CouplingPkg {
        CouplingPkg { name: name.into(), old_version: old.into(), new_version: new.into(), pkgbase: None, remaining }
    }

    fn owners(interp: &str, pkgs: &[&str]) -> SonameData {
        let mut d = SonameData::default();
        d.module_dir_owners.insert(interp.into(), pkgs.iter().map(|s| s.to_string()).collect());
        d
    }

    #[test]
    fn interpreters_name_the_folder_they_leave() {
        assert_eq!(module_dir("perl", "5.42.3-1").as_deref(), Some("usr/lib/perl5/5.42/"));
        assert_eq!(module_dir("python", "3.14.2-1").as_deref(), Some("usr/lib/python3.14/"));
        assert_eq!(module_dir("bash", "5.3.3-1"), None);
        assert_eq!(interpreter_moves(&[("perl", "5.42.3-1", "5.44.0-1"), ("perl-foo", "1-1", "2-1")]),
            vec![("perl".to_string(), "usr/lib/perl5/5.42/".to_string())]);
        assert!(interpreter_moves(&[("perl", "5.44.0-1", "5.44.1-1")]).is_empty(), "same series: nothing moves");
        assert!(interpreter_moves(&[("python", "3.14.2-1", "3.14.3-1")]).is_empty());
    }

    #[test]
    fn perl_waits_for_the_packages_rebuilt_for_it() {
        let ready = [cp("perl", "5.42.3-1", "5.44.0-1", 0)];
        let held = [cp("apparmor", "4.1.7-1", "4.1.8-2", 1), cp("subversion", "1.14.5-5", "1.14.5-6", 1)];
        let d = owners("perl", &["apparmor", "subversion"]);
        assert_eq!(module_dir_coupling_demotions(&ready, &held, &d), vec![("perl".to_string(), "apparmor".to_string())]);
        // and through the whole chain
        assert_eq!(coupling_demotions(&ready, &held, &d), vec![("perl".to_string(), "apparmor".to_string())]);
    }

    #[test]
    fn perl_moves_when_its_partners_move_too() {
        let ready = [cp("perl", "5.42.3-1", "5.44.0-1", 0), cp("apparmor", "4.1.7-1", "4.1.8-2", 0),
                     cp("subversion", "1.14.5-5", "1.14.5-6", 0)];
        let d = owners("perl", &["apparmor", "subversion"]);
        assert!(module_dir_coupling_demotions(&ready, &[], &d).is_empty());
    }

    #[test]
    fn a_module_with_no_rebuild_blocks_the_interpreter() {
        // a foreign perl module built for 5.42, nothing pending for it
        let ready = [cp("perl", "5.42.3-1", "5.44.0-1", 0)];
        let d = owners("perl", &["perl-foo-aur"]);
        assert_eq!(module_dir_coupling_demotions(&ready, &[], &d), vec![("perl".to_string(), "perl-foo-aur".to_string())]);
    }

    #[test]
    fn a_same_series_interpreter_update_moves_nothing() {
        let ready = [cp("perl", "5.44.0-1", "5.44.1-1", 0)];
        let d = owners("perl", &["apparmor"]);
        assert!(module_dir_coupling_demotions(&ready, &[], &d).is_empty());
        let ready = [cp("python", "3.14.2-1", "3.15.0-1", 0)];
        let held = [cp("python-requests", "2.32.5-1", "2.32.5-2", 3)];
        let d = owners("python", &["python-requests", "python"]);
        assert_eq!(module_dir_coupling_demotions(&ready, &held, &d), vec![("python".to_string(), "python-requests".to_string())]);
    }

    // ---- F-30 (#67): the reverse soname rule --------------------------------
    //
    // The 2026-11-10 desktop, as `nog update --json` saw it: openssl 4.1.0-1 and
    // lib32-openssl 1:4.1.0-2 held 18 days (kmod still links libcrypto.so.3),
    // Arch's OpenSSL 4 rebuilds Ready. Build dates are the real ones.

    const OPENSSL4_BUILT: u64 = 1792835125;

    fn openssl_desktop() -> (Vec<CouplingPkg>, Vec<CouplingPkg>, SonameData) {
        let mut data = sd(
            &[
                ("openssl", &["libcrypto.so=3-64", "libssl.so=3-64"], &[]),
                ("lib32-openssl", &["libcrypto.so=3-32", "libssl.so=3-32"], &["openssl"]),
                ("kmod", &["libkmod.so=2-64"], &["openssl"]),
                ("curl", &["libcurl.so=4-64"], &["openssl", "libcrypto.so=3-64", "libssl.so=3-64"]),
                ("sudo", &[], &["openssl", "libcrypto.so=3-64", "libssl.so=3-64"]),
                ("lib32-curl", &["libcurl.so=4-32"], &["lib32-openssl", "libcrypto.so=3-32", "libssl.so=3-32"]),
                ("git", &[], &["curl", "openssl"]),
                ("krb5", &["libkrb5.so=3-64"], &["openssl"]),
                ("lib32-krb5", &["libkrb5.so=3-32"], &["lib32-openssl"]),
                ("qca-qt6", &[], &["qt6-base"]),
                ("libbpf", &["libbpf.so=1-64"], &["libelf"]),
            ],
            &[
                ("openssl", &["libcrypto.so=4-64", "libssl.so=4-64"]),
                ("lib32-openssl", &["libcrypto.so=4-32", "libssl.so=4-32"]),
                ("curl", &["libcurl.so=4-64"]),
                ("sudo", &[]),
                ("lib32-curl", &["libcurl.so=4-32"]),
                ("git", &[]),
                ("krb5", &["libkrb5.so=3-64"]),
                ("lib32-krb5", &["libkrb5.so=3-32"]),
                ("qca-qt6", &[]),
                ("libbpf", &["libbpf.so=1-64"]),
            ],
        );
        let deps = |n: &str, d: &[&str]| (n.to_string(), d.iter().map(|s| s.to_string()).collect());
        data.new_depends = [
            deps("curl", &["openssl", "libcrypto.so=4-64", "libssl.so=4-64"]),
            deps("sudo", &["openssl", "libcrypto.so=4-64", "libssl.so=4-64"]),
            deps("lib32-curl", &["lib32-openssl", "libcrypto.so=4-32", "libssl.so=4-32"]),
            deps("git", &["curl", "openssl"]),
            deps("krb5", &["openssl"]),
            deps("lib32-krb5", &["lib32-openssl"]),
            deps("qca-qt6", &["qt6-base"]),
            deps("libbpf", &["libelf"]),
        ]
        .into_iter()
        .collect();
        data.new_built = [
            ("openssl", OPENSSL4_BUILT),
            ("lib32-openssl", 1793651187),
            ("curl", 1792846868),
            ("sudo", 1792850456),
            ("lib32-curl", 1793653757),
            ("git", 1793009959),
            ("krb5", 1792851019),
            ("lib32-krb5", 1793655523),
            ("qca-qt6", 1793814123),
            ("libbpf", 1791726446), // built before OpenSSL 4 existed
        ]
        .into_iter()
        .map(|(n, t)| (n.to_string(), t))
        .collect();
        // The #16 scan: who links the sonames openssl drops, today.
        let linked = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        data.linked_by.insert("libcrypto.so=3-64".into(), linked(&["curl", "git", "kmod", "krb5", "libbpf", "qca-qt6", "sudo"]));
        data.linked_by.insert("libcrypto.so=3-32".into(), linked(&["lib32-curl", "lib32-krb5"]));

        let ready = vec![
            pkg("curl", "8.23.0-1", "8.23.0-2", None, 0),
            pkg("sudo", "1.9.17.p2-6", "1.9.17.p2-7", None, 0),
            pkg("lib32-curl", "8.23.0-1", "8.23.0-2", None, 0),
            pkg("git", "2.56.0-1", "2.56.0-2", None, 0),
            pkg("krb5", "1.22.2-1", "1.22.2-2", None, 0),
            pkg("lib32-krb5", "1.22.2-1", "1.22.2-2", None, 0),
            pkg("qca-qt6", "2.3.12-1", "2.3.12-2", None, 0),
            pkg("libbpf", "1.7.0-1", "1.8.0-1", None, 0),
        ];
        let held = vec![
            pkg("kmod", "34.2-1", "34.3-2", None, 18),
            pkg("openssl", "3.6.5-1", "4.1.0-1", None, 18),
            pkg("lib32-openssl", "1:3.6.5-1", "1:4.1.0-2", None, 18),
        ];
        (ready, held, data)
    }

    #[test]
    fn the_old_rules_let_the_openssl_4_rebuilds_through() {
        // The failing direction: every rule nog had before F-30, on today's
        // data, leaves all eight Ready, which is the transaction pacman refused.
        let (ready, held, data) = openssl_desktop();
        let old_rules: Vec<_> = lib32_coupling_demotions(
            &ready.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
            &held.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
        )
        .into_iter()
        .chain(pkgbase_coupling_demotions(&ready, &held))
        .chain(cohort_coupling_demotions(&ready, &held))
        .chain(soname_coupling_demotions(&ready, &held, &data))
        .chain(module_dir_coupling_demotions(&ready, &held, &data))
        .collect();
        assert!(old_rules.is_empty(), "the old rules already held {old_rules:?}");
    }

    #[test]
    fn the_openssl_4_rebuilds_wait_for_openssl() {
        let (ready, held, data) = openssl_desktop();
        let mut d = coupling_demotions(&ready, &held, &data);
        d.sort();
        assert_eq!(
            d,
            vec![
                ("curl".to_string(), "openssl".to_string()),       // declared =4-64
                ("git".into(), "openssl".into()),                  // links .so.3, built after
                ("krb5".into(), "openssl".into()),
                ("lib32-curl".into(), "lib32-openssl".into()),     // declared =4-32
                ("lib32-krb5".into(), "lib32-openssl".into()),     // the 32-bit scan
                ("qca-qt6".into(), "openssl".into()),
                ("sudo".into(), "openssl".into()),
            ],
            "libbpf, built before OpenSSL 4 existed, is the only one left Ready"
        );
    }

    #[test]
    fn a_declared_need_alone_is_enough() {
        // No scan, no dates: the metadata by itself holds curl.
        let (ready, held, mut data) = openssl_desktop();
        data.linked_by.clear();
        data.new_built.clear();
        let names = demoted_names(needs_coupling_demotions(&ready, &held, &data));
        assert_eq!(names, vec!["curl", "lib32-curl", "sudo"]);
    }

    #[test]
    fn nothing_extra_is_held_when_the_provider_is_ready() {
        // openssl's own hold is over: everything moves together.
        let (mut ready, held, data) = openssl_desktop();
        ready.push(pkg("openssl", "3.6.5-1", "4.1.0-1", None, 0));
        ready.push(pkg("lib32-openssl", "1:3.6.5-1", "1:4.1.0-2", None, 0));
        let held: Vec<CouplingPkg> = held.into_iter().filter(|h| h.name == "kmod").collect();
        assert!(needs_coupling_demotions(&ready, &held, &data).is_empty());
    }

    #[test]
    fn a_need_something_else_still_provides_holds_nothing() {
        // An installed package that stays already provides the new soname
        // (a compatibility build, say): the rebuilds have what they need.
        let (ready, held, mut data) = openssl_desktop();
        data.installed_provides.insert(
            "openssl-next".into(),
            vec!["libcrypto.so=4-64".into(), "libssl.so=4-64".into()],
        );
        let names = demoted_names(needs_coupling_demotions(&ready, &held, &data));
        assert_eq!(names, vec!["lib32-curl", "lib32-krb5"], "only the 32-bit side still waits");
    }

    #[test]
    fn a_held_package_that_drops_a_soname_without_a_new_one_infers_nothing() {
        // No bump, only a drop: nothing can have been built against a newer
        // one, so the undeclared half stays quiet (the forward rule owns this).
        let (ready, held, mut data) = openssl_desktop();
        data.new_provides.insert("openssl".into(), vec![]);
        data.new_provides.insert("lib32-openssl".into(), vec![]);
        assert!(needs_coupling_demotions(&ready, &held, &data).is_empty());
    }

    #[test]
    fn a_32_bit_bump_never_stands_in_for_a_64_bit_one() {
        // Only lib32-openssl is held. git links the 64-bit libcrypto.so.3,
        // which nothing held is bumping, so git moves.
        let (ready, held, data) = openssl_desktop();
        let held: Vec<CouplingPkg> = held.into_iter().filter(|h| h.name == "lib32-openssl").collect();
        let names = demoted_names(needs_coupling_demotions(&ready, &held, &data));
        assert_eq!(names, vec!["lib32-curl", "lib32-krb5"]);
    }

    #[test]
    fn the_reverse_rule_chains_through_the_fixpoint() {
        // A library held by the new rule can in turn be needed: `foo` needs
        // `libcurl.so=5-64`, which only curl's new build would bring, once curl
        // is held for openssl.
        let (mut ready, mut held, mut data) = openssl_desktop();
        data.installed_provides.insert("curl".into(), vec!["libcurl.so=4-64".into()]);
        data.new_provides.insert("curl".into(), vec!["libcurl.so=5-64".into()]);
        data.installed_provides.insert("foo".into(), vec![]);
        data.new_depends.insert("foo".into(), vec!["libcurl.so=5-64".into()]);
        ready.push(pkg("foo", "1-1", "1-2", None, 0));

        let mut passes = 0;
        loop {
            let d = coupling_demotions(&ready, &held, &data);
            if d.is_empty() {
                break;
            }
            passes += 1;
            assert!(passes < 16);
            let names: HashSet<&str> = d.iter().map(|(n, _)| n.as_str()).collect();
            let (out, keep): (Vec<_>, Vec<_>) = ready.drain(..).partition(|p| names.contains(p.name.as_str()));
            ready = keep;
            held.extend(out.into_iter().map(|mut p| { p.remaining = 18; p }));
        }
        assert!(held.iter().any(|h| h.name == "foo"), "foo waits for curl");
        assert_eq!(ready.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["libbpf"]);
    }

    /// Javier's 2.1.1 run, 2026-11-11 20:02: the openssl desktop plus the
    /// dotnet pair and Arch's new `openssl-3` compat package, from the real
    /// sync and local databases.
    fn openssl_3_desktop() -> (Vec<CouplingPkg>, Vec<CouplingPkg>, SonameData) {
        let (mut ready, held, mut data) = openssl_desktop();
        let v = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        data.installed_provides.insert("dotnet-host".into(), v(&[]));
        data.installed_provides.insert("dotnet-runtime".into(), v(&["dotnet-runtime=10.0", "dotnet-runtime-10.0"]));
        data.installed_depends.insert("dotnet-host".into(), v(&["glibc", "libgcc", "libstdc++"]));
        data.installed_depends.insert(
            "dotnet-runtime".into(),
            v(&["dotnet-host", "glibc", "icu", "krb5", "libgcc", "libstdc++", "libunwind", "zlib", "openssl"]),
        );
        data.new_provides.insert("dotnet-host".into(), v(&[]));
        data.new_provides.insert("dotnet-runtime".into(), v(&["dotnet-runtime=10.0", "dotnet-runtime-10.0"]));
        data.new_provides.insert("openssl-3".into(), v(&["libcrypto.so=3-64", "libssl.so=3-64"]));
        data.new_depends.insert("dotnet-host".into(), v(&["glibc", "libgcc", "libstdc++"]));
        data.new_depends.insert(
            "dotnet-runtime".into(),
            v(&["dotnet-host", "glibc", "icu", "krb5", "libgcc", "libstdc++", "libunwind", "zlib", "openssl-3"]),
        );
        data.new_depends.insert("openssl-3".into(), v(&["brotli", "glibc", "zlib", "zstd"]));
        data.new_built.insert("dotnet-host".into(), 1794033591);
        data.new_built.insert("dotnet-runtime".into(), 1794033591);
        data.new_built.insert("openssl-3".into(), 1793476754);
        let core = Some("dotnet-core");
        ready.push(pkg("dotnet-host", "10.0.13.sdk113-1", "10.0.14.sdk114-2", core, 0));
        ready.push(pkg("dotnet-runtime", "10.0.13.sdk113-1", "10.0.14.sdk114-2", core, 0));
        (ready, held, data)
    }

    /// The fixpoint as `commands::update` runs it; returns what stays Ready.
    fn settle(ready: &mut Vec<CouplingPkg>, held: &mut Vec<CouplingPkg>, data: &SonameData) -> HashMap<String, String> {
        let mut why: HashMap<String, String> = HashMap::new();
        let mut passes = 0;
        loop {
            let d = coupling_demotions(ready, held, data);
            if d.is_empty() {
                break;
            }
            passes += 1;
            assert!(passes < 16);
            let names: HashSet<&str> = d.iter().map(|(n, _)| n.as_str()).collect();
            let (out, keep): (Vec<_>, Vec<_>) = ready.drain(..).partition(|p| names.contains(p.name.as_str()));
            *ready = keep;
            held.extend(out.into_iter().map(|mut p| { p.remaining = 18; p }));
            why.extend(d);
        }
        why
    }

    #[test]
    fn the_2_1_1_rules_let_dotnet_pull_openssl_3_in() {
        // The failing direction: every rule 2.1.1 had leaves dotnet Ready, and
        // pacman refused the run on openssl-3's files.
        let (ready, held, data) = openssl_3_desktop();
        let old: Vec<(String, String)> = pkgbase_coupling_demotions(&ready, &held)
            .into_iter()
            .chain(cohort_coupling_demotions(&ready, &held))
            .chain(soname_coupling_demotions(&ready, &held, &data))
            .chain(needs_coupling_demotions(&ready, &held, &data))
            .chain(module_dir_coupling_demotions(&ready, &held, &data))
            .collect();
        assert!(!old.iter().any(|(n, _)| n.starts_with("dotnet")), "{old:?}");
    }

    #[test]
    fn dotnet_waits_for_openssl_and_dotnet_host_follows() {
        let (mut ready, mut held, data) = openssl_3_desktop();
        assert!(pulled_in_coupling_demotions(&ready, &held, &data)
            .contains(&("dotnet-runtime".to_string(), "openssl".to_string())));
        let why = settle(&mut ready, &mut held, &data);
        assert_eq!(why.get("dotnet-runtime").map(String::as_str), Some("openssl"));
        assert_eq!(why.get("dotnet-host").map(String::as_str), Some("dotnet-runtime"), "the pkgbase rule");
        assert_eq!(ready.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["libbpf"]);
    }

    #[test]
    fn nothing_is_pulled_in_when_openssl_is_ready() {
        // openssl moving in the same run frees the files: openssl-3 may come.
        let (mut ready, mut held, data) = openssl_3_desktop();
        let moved: Vec<CouplingPkg> = held.drain(..).collect();
        ready.extend(moved.into_iter().map(|mut p| { p.remaining = 0; p }));
        assert!(pulled_in_coupling_demotions(&ready, &held, &data).is_empty());
    }

    #[test]
    fn a_new_package_taking_nothing_from_a_held_one_holds_nothing() {
        let (ready, held, mut data) = openssl_3_desktop();
        data.new_provides.insert("openssl-3".into(), vec!["libfoo.so=1-64".into()]);
        assert!(pulled_in_coupling_demotions(&ready, &held, &data).is_empty());
    }

    #[test]
    fn a_dependency_already_installed_pulls_nothing() {
        let (ready, held, mut data) = openssl_3_desktop();
        data.installed_provides.insert("openssl-3".into(), vec![]);
        assert!(pulled_in_coupling_demotions(&ready, &held, &data).is_empty());
    }

    #[test]
    fn the_takeover_is_found_through_another_new_package() {
        // dotnet-runtime -> dotnet-crypto (new) -> openssl-3 (new).
        let (ready, held, mut data) = openssl_3_desktop();
        let deps = data.new_depends.get_mut("dotnet-runtime").unwrap();
        deps.retain(|d| d != "openssl-3");
        deps.push("dotnet-crypto".into());
        data.new_provides.insert("dotnet-crypto".into(), vec![]);
        data.new_depends.insert("dotnet-crypto".into(), vec!["openssl-3".into()]);
        assert!(pulled_in_coupling_demotions(&ready, &held, &data)
            .contains(&("dotnet-runtime".to_string(), "openssl".to_string())));
    }

    /// Javier's 2.1.2 run, 2026-11-11 20:34: simdjson (no sonames declared)
    /// Ready on its way to `.so.36`, nodejs, its only user, held with openssl.
    fn simdjson_desktop() -> (Vec<CouplingPkg>, Vec<CouplingPkg>, SonameData) {
        let (mut ready, mut held, mut data) = openssl_3_desktop();
        let v = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        data.installed_provides.insert("simdjson".into(), v(&[]));
        data.installed_provides.insert("nodejs".into(), v(&[]));
        data.installed_depends.insert("simdjson".into(), v(&["libgcc", "libstdc++"]));
        data.installed_depends.insert("nodejs".into(), v(&["openssl", "simdjson", "zlib"]));
        data.new_provides.insert("simdjson".into(), v(&[]));
        data.new_provides.insert("nodejs".into(), v(&[]));
        data.new_depends.insert("simdjson".into(), v(&["libgcc", "libstdc++"]));
        data.new_depends.insert("nodejs".into(), v(&["openssl", "simdjson", "zlib"]));
        data.new_built.insert("simdjson".into(), 1793810820);
        data.new_built.insert("nodejs".into(), 1794421552);
        data.shipped_libs.insert("simdjson".into(), v(&["libsimdjson.so=34-64", "libsimdjson.so=34.0.0-64"]));
        data.linked_by.insert("libsimdjson.so=34-64".into(), v(&["nodejs"]));
        ready.push(pkg("simdjson", "1:5.0.5-1", "2:5.1.1-1", None, 0));
        held.push(pkg("nodejs", "26.12.1-1", "26.13.1-1", None, 17));
        (ready, held, data)
    }

    #[test]
    fn the_2_1_2_rules_let_simdjson_through() {
        // The failing direction: every rule 2.1.2 had leaves simdjson Ready,
        // and node lost libsimdjson.so.34.
        let (ready, held, data) = simdjson_desktop();
        let old: Vec<(String, String)> = pkgbase_coupling_demotions(&ready, &held)
            .into_iter()
            .chain(cohort_coupling_demotions(&ready, &held))
            .chain(soname_coupling_demotions(&ready, &held, &data))
            .chain(needs_coupling_demotions(&ready, &held, &data))
            .chain(pulled_in_coupling_demotions(&ready, &held, &data))
            .chain(module_dir_coupling_demotions(&ready, &held, &data))
            .collect();
        assert!(!old.iter().any(|(n, _)| n == "simdjson"), "{old:?}");
    }

    #[test]
    fn simdjson_waits_for_nodejs() {
        let (mut ready, mut held, data) = simdjson_desktop();
        let why = settle(&mut ready, &mut held, &data);
        assert_eq!(why.get("simdjson").map(String::as_str), Some("nodejs"));
        assert_eq!(ready.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["libbpf"]);
    }

    #[test]
    fn a_rebuild_of_the_same_version_holds_nothing() {
        let (mut ready, held, data) = simdjson_desktop();
        let s = ready.iter_mut().find(|p| p.name == "simdjson").unwrap();
        s.old_version = "2:5.1.1-1".into();
        s.new_version = "2:5.1.1-2".into();
        assert!(undeclared_soname_demotions(&ready, &held, &data).is_empty());
    }

    #[test]
    fn a_user_built_before_the_library_holds_nothing() {
        let (ready, held, mut data) = simdjson_desktop();
        data.new_built.insert("nodejs".into(), 1793810819);
        assert!(undeclared_soname_demotions(&ready, &held, &data).is_empty());
    }

    #[test]
    fn a_user_moving_in_the_same_run_holds_nothing() {
        let (mut ready, mut held, data) = simdjson_desktop();
        let n = held.iter().position(|h| h.name == "nodejs").unwrap();
        ready.push(held.remove(n));
        assert!(undeclared_soname_demotions(&ready, &held, &data).is_empty());
    }

    #[test]
    fn a_declared_library_is_left_to_the_soname_rule() {
        let (ready, held, mut data) = simdjson_desktop();
        data.installed_provides.insert("simdjson".into(), vec!["libsimdjson.so=34-64".into()]);
        assert!(undeclared_soname_demotions(&ready, &held, &data).is_empty());
        assert_eq!(
            undeclared_libs(&["libsimdjson.so=34-64".into(), "libfoo.so=1-32".into()], &["libsimdjson.so=34-64".into()]),
            vec!["libfoo.so=1-32".to_string()]
        );
    }

    #[test]
    fn libmupdf_waits_for_zathura_pdf_mupdf() {
        // The second break the same night: zathura's PDF plugin linked
        // libmupdf.so.28.5; libmupdf 1.28.6 (no sonames declared) took it.
        let (mut ready, mut held, mut data) = simdjson_desktop();
        let v = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        data.installed_provides.insert("libmupdf".into(), v(&[]));
        data.installed_provides.insert("zathura-pdf-mupdf".into(), v(&[]));
        data.shipped_libs.insert("libmupdf".into(), v(&["libmupdf.so=28.5-64", "libmupdfcpp.so=28.5-64"]));
        data.linked_by.insert("libmupdf.so=28.5-64".into(), v(&["zathura-pdf-mupdf"]));
        data.new_built.insert("libmupdf".into(), 1794353233);
        data.new_built.insert("zathura-pdf-mupdf".into(), 1794353942);
        ready.push(pkg("libmupdf", "1.28.5-2", "1.28.6-1", None, 0));
        held.push(pkg("zathura-pdf-mupdf", "2026.07.18-6", "2026.07.18-7", None, 5));
        let why = settle(&mut ready, &mut held, &data);
        assert_eq!(why.get("libmupdf").map(String::as_str), Some("zathura-pdf-mupdf"));
    }
}
