// snap.rs — Snap backend (v1.2.0, the multi-source arc C2)
//
// Same shape as flatpak.rs: nog delegates to the `snap` CLI, never to snapd's
// socket API. Detect → ask what's pending → resolve publish dates so the tier
// hold windows apply → hand off the refresh.
//
// Design rulings (docs/v2-design.md):
//   - Snap is PRAGMATIC, not ideological (ruling #4). snapd is AUR-only on
//     Arch, so nog never demands it: absent snapd = dormant source, never an
//     error. The install-snapd offer belongs to the C3 install chain.
//   - Expected to serve the ~1% tail (ruling #3) — most software is covered
//     long before the chain reaches here.
//   - Tiers apply: snaps age through the same windows, clocked by the
//     publish date of the pending revision in the tracked channel.
//   - Show the work (issue #8 rule): the refresh runs with snap's own
//     progress output intact.
//
// One structural difference from flatpak: `snap refresh` requires root.
// nog stays unprivileged and shells out through `sudo`, the same way
// tiers::write_as_root does for nog-owned files.
//
// CLI surface used:
//   snap refresh --list      pending updates (table; "All snaps up to date.")
//   snap list                installed snaps + tracked channel
//   snap info <name>         channels: block, with per-channel publish dates
//   sudo snap refresh <names…>   apply exactly the named snaps

use std::collections::HashMap;
use std::process::{Command, ExitStatus};

use crate::pacman::PendingUpdate;

/// PATH check, same convention as aur::is_on_path / flatpak::is_available.
pub fn is_available() -> bool {
    Command::new("snap")
        .arg("version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// One pending snap refresh. `channel` is the tracked channel (e.g.
/// "latest/stable") — needed to find the right publish date in `snap info`.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapUpdate {
    pub name: String,
    pub new_version: String,
    pub channel: String,
}

/// Ask snapd what refreshes are pending.
///
/// `snap refresh --list` prints "All snaps up to date." when there is nothing
/// to do (exit 0). A non-zero exit means snapd is unreachable or broken:
/// bubble it up so the caller fails CLOSED, never "assume all quiet".
pub fn pending_updates() -> Result<Vec<SnapUpdate>, String> {
    let output = Command::new("snap")
        .args(["refresh", "--list"])
        .output()
        .map_err(|e| format!("failed to launch snap: {}", e))?;

    if !output.status.success() {
        let msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if msg.is_empty() {
            format!("snap refresh --list exited with status {}", output.status)
        } else {
            msg
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let channels = tracked_channels();
    Ok(parse_refresh_list(&stdout, &channels))
}

/// Parse the `snap refresh --list` table. Columns are whitespace-aligned:
///   Name  Version  Rev  Size  Publisher  Notes
/// The header row and the "All snaps up to date." sentence are skipped.
/// Kept pure for tests; `channels` supplies the tracked channel per snap.
pub fn parse_refresh_list(stdout: &str, channels: &HashMap<String, String>) -> Vec<SnapUpdate> {
    let mut updates = Vec::new();
    for line in stdout.lines() {
        let line = line.trim_end();
        let t = line.trim();
        if t.is_empty() { continue; }
        if t.starts_with("All snaps up to date") { continue; }
        let mut parts = t.split_whitespace();
        let name = match parts.next() {
            Some(s) => s,
            None => continue,
        };
        if name == "Name" { continue; } // header
        let new_version = parts.next().unwrap_or("").to_string();
        updates.push(SnapUpdate {
            name: name.to_string(),
            new_version,
            channel: channels
                .get(name)
                .cloned()
                .unwrap_or_else(|| "latest/stable".to_string()),
        });
    }
    updates
}

/// Installed snaps: name → (version, tracked channel), from `snap list`.
fn list_installed() -> Vec<(String, String, String)> {
    let output = match Command::new("snap").arg("list").output() {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    parse_snap_list(&String::from_utf8_lossy(&output.stdout))
}

/// Parse `snap list`: Name Version Rev Tracking Publisher Notes.
/// Returns (name, version, tracking). Pure, for tests.
pub fn parse_snap_list(stdout: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let t = line.trim();
        if t.is_empty() { continue; }
        let cols: Vec<&str> = t.split_whitespace().collect();
        if cols.len() < 4 || cols[0] == "Name" { continue; }
        out.push((cols[0].to_string(), cols[1].to_string(), cols[3].to_string()));
    }
    out
}

/// Tracked channel per installed snap — the key to picking the right date.
fn tracked_channels() -> HashMap<String, String> {
    list_installed()
        .into_iter()
        .map(|(name, _v, channel)| (name, channel))
        .collect()
}

/// Installed versions, for the Old column of the update table.
pub fn installed_versions() -> HashMap<String, String> {
    list_installed()
        .into_iter()
        .map(|(name, version, _c)| (name, version))
        .collect()
}

/// Resolve each pending update's publish date (Unix time) from `snap info`,
/// reading the tracked channel's line in the `channels:` block. Snaps whose
/// date can't be resolved are omitted → Unknown bucket, matching the AUR and
/// flatpak fallbacks.
pub fn publish_dates_for(updates: &[SnapUpdate]) -> HashMap<String, u64> {
    let mut out = HashMap::new();
    for u in updates {
        let output = match Command::new("snap").args(["info", &u.name]).output() {
            Ok(o) if o.status.success() => o,
            _ => continue,
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(date) = channel_date(&stdout, &u.channel) {
            if let Some(ts) = to_unix(&date) {
                out.insert(u.name.clone(), ts);
            }
        }
    }
    out
}

/// Extract the `YYYY-MM-DD` date for `channel` from `snap info` output.
///
/// The channels block looks like:
///   channels:
///     latest/stable:    1.2.3 2026-07-20 (1234) 45MB -
///     latest/candidate: ↑
/// A `↑` means "same as the channel above" and carries no date — treated as
/// unresolved (Unknown), never guessed.
pub fn channel_date(stdout: &str, channel: &str) -> Option<String> {
    let want = format!("{}:", channel);
    for line in stdout.lines() {
        let t = line.trim();
        if !t.starts_with(&want) { continue; }
        let rest = t[want.len()..].trim();
        for token in rest.split_whitespace() {
            if is_iso_date(token) {
                return Some(token.to_string());
            }
        }
        return None; // matched the channel but it carries no date (e.g. "↑")
    }
    None
}

fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() }
        })
}

fn to_unix(date: &str) -> Option<u64> {
    let out = Command::new("date").arg("-d").arg(date).arg("+%s").output().ok()?;
    if !out.status.success() { return None; }
    String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().ok()
}

/// Which snaps may be refreshed this run — the same naming rule as flatpak:
/// `snap refresh <names>` touches exactly what it is told, so passing only
/// cleared snaps IS the hold mechanism. Single decision point, so the rule is
/// testable rather than merely true.
pub fn apply_list(
    snap_names: &[String],
    ready: &[String],
    unknown: &[String],
    ignore: &[String],
) -> Vec<String> {
    let is_snap = |n: &String| snap_names.iter().any(|s| s == n);
    let mut out: Vec<String> = ready.iter().filter(|n| is_snap(n)).cloned().collect();
    out.extend(
        unknown.iter()
            .filter(|n| is_snap(n) && !ignore.iter().any(|i| i == *n))
            .cloned(),
    );
    out
}

/// snapd's own auto-refresh runs on ITS timer, not nog's -- four times a day
/// by default. Listing only cleared snaps at refresh time therefore held
/// nothing: on 2026-09-15 nog classified `core20` as held with "1 day
/// remaining" and snapd auto-refreshed it in the same minute (issue #18).
///
/// So a tier hold has to be placed WHERE IT IS ENFORCED -- in snapd:
///     snap refresh --hold=<duration> <names...>
///
/// The asymmetry in snapd's own help is what makes this work cleanly:
/// a named hold blocks auto-refreshes and blanket `snap refresh`, but
/// "specific snap requests from 'snap refresh target-snap' remain unblocked".
/// nog always names the snaps it refreshes, so it never has to unhold first --
/// the hold stops snapd and steps aside for nog.
///
/// Durations are Go-style: hours, because `d` is not a unit snapd parses.
/// Clamped to [1h, 90d] -- 90 days is snapd's own ceiling for a named hold,
/// and a 0-day window (a Tier 1 package awaiting manual signoff) means
/// "until a human says so", which is the ceiling rather than no hold at all.
///
/// Pure and grouped so one call covers every snap sharing a window.
pub const SNAP_HOLD_MAX_HOURS: u64 = 90 * 24;
pub fn hold_args(held: &[(String, u64)]) -> Vec<(u64, Vec<String>)> {
    let mut by_hours: HashMap<u64, Vec<String>> = HashMap::new();
    for (name, days) in held {
        // 0 days is NOT a short window -- it is the Tier 1 "awaiting manual
        // signoff" placeholder, i.e. held until a human says otherwise. Left to
        // arithmetic it clamped to ONE HOUR, which is a hold in name only and
        // would have reproduced #18 for exactly the packages that matter most.
        let hours = if *days == 0 {
            SNAP_HOLD_MAX_HOURS
        } else {
            days.saturating_mul(24).clamp(1, SNAP_HOLD_MAX_HOURS)
        };
        by_hours.entry(hours).or_default().push(name.clone());
    }
    let mut out: Vec<(u64, Vec<String>)> = by_hours
        .into_iter()
        .map(|(h, mut names)| { names.sort(); names.dedup(); (h, names) })
        .collect();
    out.sort_by_key(|(h, _)| *h);          // deterministic, shortest first
    out
}

/// Which held packages are snaps, and how long each still has to run.
/// Split out of the update flow so the SELECTION is testable on its own --
/// the bug in #18 was never in the arithmetic, it was in what nog did (and
/// did not do) with the set it had already computed correctly.
///
/// KNOWN LIMIT (issue #20): sources are matched by name, because the run has
/// no source column. `snapd` exists both as an Arch package and as a snap, so
/// if the Arch one is held while the snap one is ready, this selects it and a
/// hold is placed on a snap that was cleared. Erring toward holding is the
/// safe direction -- nog's own explicit refresh is never blocked by a hold --
/// but the ambiguity is real and goes away when #20 lands a source column.
pub fn held_snap_windows(all_held: &[(String, u64)], snap_names: &[String]) -> Vec<(String, u64)> {
    all_held
        .iter()
        .filter(|(name, _)| snap_names.iter().any(|s| s == name))
        .cloned()
        .collect()
}

/// Place one hold. Requires root, so nog escalates exactly as `refresh` does.
pub fn place_hold(hours: u64, names: &[String]) -> ExitStatus {
    let dur = format!("--hold={}h", hours);
    let mut args: Vec<&str> = vec!["snap", "refresh", &dur];
    args.extend(names.iter().map(|s| s.as_str()));
    Command::new("sudo")
        .args(&args)
        .status()
        .unwrap_or_else(|e| panic!("nog: failed to launch sudo snap refresh --hold: {}", e))
}

/// Refresh exactly the named snaps. Requires root — nog stays unprivileged
/// and escalates through `sudo`, as it does for its own root-owned files.
/// snap's progress output is left intact (issue #8: show the work).
pub fn refresh(names: &[String]) -> ExitStatus {
    let mut args: Vec<&str> = vec!["snap", "refresh"];
    args.extend(names.iter().map(|s| s.as_str()));
    Command::new("sudo")
        .args(&args)
        .status()
        .unwrap_or_else(|e| panic!("nog: failed to launch sudo snap refresh: {}", e))
}

/// Convert a SnapUpdate into the shared PendingUpdate shape.
pub fn to_pending(u: &SnapUpdate, installed: &HashMap<String, String>) -> PendingUpdate {
    let old = installed.get(&u.name).map(|s| s.as_str()).unwrap_or("");
    PendingUpdate {
        name: u.name.clone(),
        old_version: if old.is_empty() { "?".to_string() } else { old.to_string() },
        new_version: if u.new_version.is_empty() { "?".to_string() } else { u.new_version.clone() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channels() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("bitwarden".to_string(), "latest/stable".to_string());
        m
    }

    #[test]
    fn parses_the_refresh_list_table() {
        let out = "Name       Version  Rev   Size   Publisher   Notes\n\
                   bitwarden  2026.8.1 145   120MB  bitwarden✓  -\n\
                   core22     20260801 1710  77MB   canonical✓  base\n";
        let got = parse_refresh_list(out, &channels());
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].name, "bitwarden");
        assert_eq!(got[0].new_version, "2026.8.1");
        assert_eq!(got[0].channel, "latest/stable");
        // Unknown tracking falls back to the conventional default:
        assert_eq!(got[1].channel, "latest/stable");
    }

    #[test]
    fn all_up_to_date_means_no_updates() {
        assert!(parse_refresh_list("All snaps up to date.\n", &HashMap::new()).is_empty());
        assert!(parse_refresh_list("", &HashMap::new()).is_empty());
    }

    #[test]
    fn parses_snap_list_tracking_column() {
        let out = "Name       Version   Rev   Tracking       Publisher   Notes\n\
                   bitwarden  2026.7.2  144   latest/stable  bitwarden✓  -\n";
        let got = parse_snap_list(out);
        assert_eq!(got, vec![("bitwarden".to_string(), "2026.7.2".to_string(),
                              "latest/stable".to_string())]);
    }

    #[test]
    fn finds_the_tracked_channels_date() {
        let info = "name:      bitwarden\nchannels:\n  \
                    latest/stable:    2026.8.1 2026-08-04 (145) 120MB -\n  \
                    latest/candidate: 2026.8.2 2026-08-09 (146) 120MB -\n";
        assert_eq!(channel_date(info, "latest/stable").as_deref(), Some("2026-08-04"));
        assert_eq!(channel_date(info, "latest/candidate").as_deref(), Some("2026-08-09"));
        assert_eq!(channel_date(info, "latest/edge"), None);
    }

    #[test]
    fn channel_without_a_date_is_unresolved_not_guessed() {
        // "↑" means "same as above" — no date of its own.
        let info = "channels:\n  latest/stable:    1.0 2026-01-02 (5) 1MB -\n  \
                    latest/candidate: ↑\n";
        assert_eq!(channel_date(info, "latest/candidate"), None);
    }

    #[test]
    fn apply_list_names_only_cleared_snaps() {
        let snaps = vec!["bitwarden".to_string(), "core22".to_string(), "hello".to_string()];
        let ready = vec!["bitwarden".to_string(), "vlc".to_string()]; // vlc = pacman
        let unknown = vec!["core22".to_string(), "hello".to_string()];
        let ignore = vec!["hello".to_string()]; // user skipped it
        let got = apply_list(&snaps, &ready, &unknown, &ignore);
        assert_eq!(got, vec!["bitwarden".to_string(), "core22".to_string()]);
        assert!(!got.contains(&"vlc".to_string()));
        assert!(!got.contains(&"hello".to_string()));
    }

    #[test]
    fn nog_itself_never_refreshes_a_held_snap() {
        // Renamed from `held_snaps_can_never_be_refreshed`, which claimed far
        // more than it proved. It showed only that NOG does not refresh a held
        // snap; it never asked whether anything else would, and snapd did --
        // four times a day (issue #18). The name now matches the assertion.
        let snaps = vec!["bitwarden".to_string()];
        assert!(apply_list(&snaps, &[], &[], &[]).is_empty());
    }

    #[test]
    fn hold_args_groups_snaps_sharing_a_window() {
        let held = vec![
            ("core20".to_string(), 1),
            ("hello".to_string(), 2),
            ("bitwarden".to_string(), 1),
        ];
        let got = hold_args(&held);
        assert_eq!(got, vec![
            (24, vec!["bitwarden".to_string(), "core20".to_string()]),
            (48, vec!["hello".to_string()]),
        ]);
    }

    #[test]
    fn hold_args_never_emits_a_zero_length_hold() {
        // A 0-day window is Tier 1 awaiting manual signoff. `--hold=0h` is not
        // a hold, and silently placing one would reproduce issue #18 exactly:
        // nog reporting a hold that snapd does not enforce.
        let got = hold_args(&[("core20".to_string(), 0)]);
        assert_eq!(got[0].0, SNAP_HOLD_MAX_HOURS);
    }

    #[test]
    fn hold_args_clamps_to_snapd_ceiling() {
        // snapd refuses a named hold longer than 90 days. Asking for more
        // fails the whole call, which would leave the snap UNHELD.
        let got = hold_args(&[("linux-thing".to_string(), 3650)]);
        assert_eq!(got[0].0, SNAP_HOLD_MAX_HOURS);
    }

    #[test]
    fn held_snap_windows_ignores_held_packages_that_are_not_snaps() {
        let all_held = vec![
            ("gimp".to_string(), 1),          // pacman
            ("core20".to_string(), 2),        // snap
            ("linux-zen".to_string(), 28),    // pacman
        ];
        let snaps = vec!["core20".to_string(), "hello".to_string()];
        assert_eq!(held_snap_windows(&all_held, &snaps),
                   vec![("core20".to_string(), 2)]);
    }

    #[test]
    fn held_snap_windows_is_empty_when_no_snap_is_held() {
        // The common case, and the one that must NOT prompt for root:
        // no held snaps means no hold call at all.
        let all_held = vec![("gimp".to_string(), 1)];
        let snaps = vec!["core20".to_string()];
        assert!(held_snap_windows(&all_held, &snaps).is_empty());
        assert!(hold_args(&held_snap_windows(&all_held, &snaps)).is_empty());
    }

    #[test]
    fn held_snap_windows_matches_by_name_and_says_so() {
        // Documents the #20 ambiguity rather than hiding it: `snapd` is both an
        // Arch package and a snap, and with no source column they are the same
        // string. Selecting it is the safe direction -- an explicit `snap
        // refresh snapd` from nog is never blocked by nog's own hold.
        let all_held = vec![("snapd".to_string(), 3)];
        let snaps = vec!["snapd".to_string()];
        assert_eq!(held_snap_windows(&all_held, &snaps).len(), 1);
    }

    #[test]
    fn hold_args_is_deterministic() {
        let a = hold_args(&[("b".into(), 2), ("a".into(), 2)]);
        let b = hold_args(&[("a".into(), 2), ("b".into(), 2)]);
        assert_eq!(a, b);
        assert_eq!(a[0].1, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn to_pending_marks_missing_versions_honestly() {
        let u = SnapUpdate { name: "hello".into(), new_version: "".into(),
                             channel: "latest/stable".into() };
        let p = to_pending(&u, &HashMap::new());
        assert_eq!(p.old_version, "?");
        assert_eq!(p.new_version, "?");
    }
}
