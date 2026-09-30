//! v1.6.0 (issue #27) — when an update was first seen.
//!
//! Hold windows used to be clocked from the build date of the *newest*
//! candidate. Arch only carries the newest build of each package, so every new
//! build restarted the countdown, and a package whose updates arrive faster
//! than its window was never released. linux-zen sat at 7.0.5 from May to the
//! end of September while nine newer builds came and went, each one resetting
//! the clock before it reached zero.
//!
//! This module remembers, per package and per installed version, when an
//! update was first seen and which new versions have been seen since. The hold
//! window is then clocked from that first sighting (see
//! `holds::evaluate_candidate`), and a short safety wait on the candidate
//! itself keeps a build from yesterday from walking straight in.
//!
//! The record is a small tab-separated file in user space, like the run logs:
//! nog runs unprivileged. One line per package:
//!
//!   source  name  installed_version  first_seen_unix  v1,v2,…
//!
//! The logic is pure and unit-tested; the file IO soft-fails. A missing or
//! unreadable file only means nog falls back to the old behaviour (clock from
//! the candidate's build date) for this run. It never blocks an update.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

/// What nog remembers about one pending update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sighting {
    /// The version installed when the update was first seen. When this
    /// changes, the package was upgraded and the record starts over.
    pub installed: String,
    /// Unix time the hold window is clocked from: the build date of the first
    /// new version seen, or the day a run first logged it, whichever is
    /// earlier and known.
    pub first_seen: u64,
    /// Every new version seen since, oldest first. The last one is the
    /// current candidate; the rest were skipped.
    pub versions: Vec<String>,
}

impl Sighting {
    /// How many new versions came and went without being installed.
    pub fn skipped(&self) -> usize {
        self.versions.len().saturating_sub(1)
    }
}

/// Keyed by `"<source>:<name>"`: a name is not unique across sources (the
/// Arch package `snapd` and the snap `snapd`, #20).
pub type Sightings = BTreeMap<String, Sighting>;

pub fn key(source: &str, name: &str) -> String {
    format!("{}:{}", source, name)
}

/// One pending update as this module sees it.
pub struct Seen<'a> {
    pub source: &'a str,
    pub name: &'a str,
    pub installed: &'a str,
    pub candidate: &'a str,
    /// The candidate's build (or publish) date, when known.
    pub candidate_built: Option<u64>,
}

/// What the run logs say about an update that was pending before nog kept
/// this record: the earliest day it was logged, and the new versions logged.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LogHistory {
    pub first_logged: Option<u64>,
    pub versions: Vec<String>,
}

/// Bring the record up to date with this run's pending updates.
///
/// * A package still pending at the same installed version keeps its first
///   sighting; the new candidate is added to its versions.
/// * A package pending for the first time (or at a new installed version,
///   meaning it was upgraded) starts a new record. Its first sighting is the
///   earliest of: the candidate's build date, and the first day the run logs
///   show it pending at this installed version (`history`).
/// * A package from a source that was checked this run but is no longer
///   pending is dropped — it was upgraded, or the update was withdrawn.
/// * Records from sources that were NOT checked this run are kept as they
///   are. A failed AUR query must not wipe the AUR clocks (#25).
///
/// `now` is the fallback first sighting when nothing else is known.
pub fn update<F>(
    previous: &Sightings,
    seen: &[Seen],
    checked_sources: &HashSet<&str>,
    mut history: F,
    now: u64,
) -> Sightings
where
    F: FnMut(&str, &str, &str) -> LogHistory,
{
    let mut next = Sightings::new();

    // Keep what this run cannot speak for.
    for (k, s) in previous {
        let source = k.split(':').next().unwrap_or("");
        if !checked_sources.contains(source) {
            next.insert(k.clone(), s.clone());
        }
    }

    for u in seen {
        let k = key(u.source, u.name);
        let record = match previous.get(&k) {
            Some(prev) if prev.installed == u.installed => {
                let mut s = prev.clone();
                if let Some(b) = u.candidate_built {
                    s.first_seen = s.first_seen.min(b);
                }
                push_version(&mut s.versions, u.candidate);
                s
            }
            _ => {
                let h = history(u.source, u.name, u.installed);
                let first_seen = [u.candidate_built, h.first_logged]
                    .into_iter()
                    .flatten()
                    .min()
                    .unwrap_or(now);
                let mut versions = h.versions;
                push_version(&mut versions, u.candidate);
                Sighting { installed: u.installed.to_string(), first_seen, versions }
            }
        };
        next.insert(k, record);
    }
    next
}

/// Add a version to the list, moving it to the end if it is already there, so
/// the last entry is always the current candidate.
fn push_version(versions: &mut Vec<String>, v: &str) {
    versions.retain(|x| x != v);
    versions.push(v.to_string());
}

/// Render the record as the file's text.
pub fn render(s: &Sightings) -> String {
    let mut out = String::from(
        "# nog hold record (v1.6.0, #27). One line per pending update:\n\
         # source\tname\tinstalled\tfirst_seen_unix\tversions seen, oldest first\n",
    );
    for (k, v) in s {
        let (source, name) = k.split_once(':').unwrap_or(("pacman", k.as_str()));
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            source, name, v.installed, v.first_seen, v.versions.join(",")
        ));
    }
    out
}

/// Parse the file's text. Malformed lines are skipped, not fatal.
pub fn parse(text: &str) -> Sightings {
    let mut s = Sightings::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 5 {
            continue;
        }
        let first_seen = match f[3].parse::<u64>() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let versions = f[4].split(',').filter(|v| !v.is_empty()).map(String::from).collect();
        s.insert(
            key(f[0], f[1]),
            Sighting { installed: f[2].to_string(), first_seen, versions },
        );
    }
    s
}

pub fn load(path: &str) -> Sightings {
    fs::read_to_string(path).map(|t| parse(&t)).unwrap_or_default()
}

pub fn save(path: &str, s: &Sightings) -> Result<(), String> {
    if let Some(dir) = Path::new(path).parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {}", dir.display(), e))?;
    }
    // Write beside, then rename: a crash mid-write never leaves half a record.
    let tmp = format!("{}.tmp", path);
    fs::write(&tmp, render(s)).map_err(|e| format!("{}: {}", tmp, e))?;
    fs::rename(&tmp, path).map_err(|e| format!("{}: {}", path, e))
}

// ---- Reading the run logs, for holds that began before this record existed ----

/// One run-log row, reduced to what the history needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRow {
    /// Unix time of the row's date, at midnight UTC.
    pub day: u64,
    /// `None` in logs written before v1.4.3, which had no source column.
    pub source: Option<String>,
    pub package: String,
    pub old_version: String,
    pub new_version: String,
}

/// Parse one day's CSV run log, reading its columns from its own header line.
/// Both layouts are real: ten columns before v1.4.3, twelve after, and one day
/// (29 Sep 2026) holds both, each block under its own header.
pub fn parse_log(text: &str) -> Vec<LogRow> {
    let mut rows = Vec::new();
    let mut cols: Option<(usize, Option<usize>, usize, usize, usize)> = None;
    for line in text.lines() {
        let f = split_csv(line);
        if f.first().map(String::as_str) == Some("date") {
            let at = |name: &str| f.iter().position(|c| c == name);
            cols = match (at("package"), at("old_version"), at("new_version")) {
                (Some(p), Some(o), Some(n)) => Some((0, at("source"), p, o, n)),
                _ => None,
            };
            continue;
        }
        let Some((d, s, p, o, n)) = cols else { continue };
        let (Some(date), Some(pkg), Some(old), Some(new)) = (f.get(d), f.get(p), f.get(o), f.get(n)) else {
            continue;
        };
        if pkg.is_empty() || new.is_empty() {
            continue;
        }
        let Some(day) = us_date_to_unix(date) else { continue };
        rows.push(LogRow {
            day,
            source: s.and_then(|i| f.get(i)).cloned(),
            package: pkg.clone(),
            old_version: old.clone(),
            new_version: new.clone(),
        });
    }
    rows
}

/// The history of one package at one installed version, from parsed rows.
/// Rows from the old layout (no source column) match any source.
pub fn history_from(rows: &[LogRow], source: &str, name: &str, installed: &str) -> LogHistory {
    let mut h = LogHistory::default();
    for r in rows {
        if r.package != name || r.old_version != installed {
            continue;
        }
        if let Some(s) = &r.source {
            if s != source {
                continue;
            }
        }
        h.first_logged = Some(h.first_logged.map_or(r.day, |d| d.min(r.day)));
        if !h.versions.contains(&r.new_version) {
            h.versions.push(r.new_version.clone());
        }
    }
    h
}

/// Read every run log in `dir`, oldest first. Unreadable files are skipped.
pub fn load_logs(dir: &str) -> Vec<LogRow> {
    let mut names: Vec<_> = match fs::read_dir(dir) {
        Ok(it) => it.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(_) => return Vec::new(),
    };
    names.retain(|p| p.to_string_lossy().ends_with("nog-update.csv"));
    names.sort();
    names.iter()
        .filter_map(|p| fs::read_to_string(p).ok())
        .flat_map(|t| parse_log(&t))
        .collect()
}

/// Split one CSV line (RFC 4180 quoting, as runlog writes it).
fn split_csv(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => { cur.push('"'); chars.next(); }
            ('"', _) => quoted = !quoted,
            (',', false) => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// "MM/DD/YYYY" → Unix time at midnight UTC. Days-from-civil (Howard Hinnant),
/// so no date crate and no subprocess.
fn us_date_to_unix(s: &str) -> Option<u64> {
    let mut it = s.split('/');
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    let y: i64 = it.next()?.parse().ok()?;
    if it.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400).ok()
}

/// Unix time → "Jul 29", for the tables ("waiting since Jul 29"). UTC, which
/// is at most a day off the local date and needs no date crate.
pub fn short_date(ts: u64) -> String {
    let days = (ts / 86_400) as i64 + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    const NAMES: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!("{} {}", NAMES[(m - 1) as usize], d)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: u64 = 86_400;

    fn seen<'a>(name: &'a str, installed: &'a str, cand: &'a str, built: u64) -> Seen<'a> {
        Seen { source: "pacman", name, installed, candidate: cand, candidate_built: Some(built) }
    }

    fn no_history(_: &str, _: &str, _: &str) -> LogHistory {
        LogHistory::default()
    }

    fn pacman_checked() -> HashSet<&'static str> {
        ["pacman"].into_iter().collect()
    }

    #[test]
    fn a_new_build_does_not_restart_the_clock() {
        // The #27 treadmill: linux-zen, a new build every 5 days.
        let mut rec = Sightings::new();
        for (i, v) in ["7.1.5", "7.1.6", "7.1.7", "7.1.8", "7.1.9"].iter().enumerate() {
            let built = 100 * DAY + i as u64 * 5 * DAY;
            rec = update(&rec, &[seen("linux-zen", "7.0.5", v, built)], &pacman_checked(), no_history, built);
        }
        let s = &rec["pacman:linux-zen"];
        assert_eq!(s.first_seen, 100 * DAY, "the first sighting must hold");
        assert_eq!(s.versions.last().unwrap(), "7.1.9");
        assert_eq!(s.skipped(), 4);
    }

    #[test]
    fn an_upgrade_starts_a_new_record() {
        let mut rec = Sightings::new();
        rec = update(&rec, &[seen("mesa", "26.1", "26.2", 10 * DAY)], &pacman_checked(), no_history, 10 * DAY);
        rec = update(&rec, &[seen("mesa", "26.2", "26.3", 50 * DAY)], &pacman_checked(), no_history, 50 * DAY);
        assert_eq!(rec["pacman:mesa"].first_seen, 50 * DAY);
        assert_eq!(rec["pacman:mesa"].skipped(), 0);
    }

    #[test]
    fn an_installed_package_leaves_the_record() {
        let mut rec = Sightings::new();
        rec = update(&rec, &[seen("vim", "9.1", "9.2", DAY)], &pacman_checked(), no_history, DAY);
        rec = update(&rec, &[], &pacman_checked(), no_history, 2 * DAY);
        assert!(rec.is_empty());
    }

    #[test]
    fn an_unchecked_source_keeps_its_clocks() {
        // #25: the AUR query failed this run. Its records must survive.
        let mut rec = Sightings::new();
        let aur = Seen { source: "aur", name: "walker-bin", installed: "2.16", candidate: "2.17", candidate_built: Some(DAY) };
        let both: HashSet<&str> = ["pacman", "aur"].into_iter().collect();
        rec = update(&rec, &[aur], &both, no_history, DAY);
        rec = update(&rec, &[], &pacman_checked(), no_history, 9 * DAY);
        assert_eq!(rec["aur:walker-bin"].first_seen, DAY);
    }

    #[test]
    fn a_hold_that_began_before_the_record_is_dated_from_the_logs() {
        let hist = |_: &str, name: &str, installed: &str| {
            assert_eq!((name, installed), ("linux-zen", "7.0.5"));
            LogHistory { first_logged: Some(20 * DAY), versions: vec!["7.1.5".into(), "7.2.6".into()] }
        };
        let rec = update(&Sightings::new(), &[seen("linux-zen", "7.0.5", "7.2.7", 80 * DAY)], &pacman_checked(), hist, 90 * DAY);
        let s = &rec["pacman:linux-zen"];
        assert_eq!(s.first_seen, 20 * DAY);
        assert_eq!(s.versions, vec!["7.1.5", "7.2.6", "7.2.7"]);
    }

    #[test]
    fn nothing_known_falls_back_to_now() {
        let u = Seen { source: "flatpak", name: "org.x", installed: "1", candidate: "2", candidate_built: None };
        let fl: HashSet<&str> = ["flatpak"].into_iter().collect();
        let rec = update(&Sightings::new(), &[u], &fl, no_history, 7 * DAY);
        assert_eq!(rec["flatpak:org.x"].first_seen, 7 * DAY);
    }

    #[test]
    fn the_file_round_trips() {
        let mut s = Sightings::new();
        s.insert(key("pacman", "linux-zen"), Sighting {
            installed: "7.0.5.zen1-1".into(), first_seen: 1_785_000_000,
            versions: vec!["7.1.5.zen1-2".into(), "7.2.7.zen1-1".into()],
        });
        s.insert(key("snap", "snapd"), Sighting { installed: "2.76".into(), first_seen: 5, versions: vec!["2.77".into()] });
        assert_eq!(parse(&render(&s)), s);
    }

    #[test]
    fn a_damaged_line_is_skipped_not_fatal() {
        let text = "pacman\tvim\t9.1\tnot-a-number\t9.2\nbroken line\npacman\tgit\t2.1\t100\t2.2\n";
        let s = parse(text);
        assert_eq!(s.len(), 1);
        assert!(s.contains_key("pacman:git"));
    }

    #[test]
    fn both_log_layouts_are_read_by_their_own_header() {
        let text = "\
date,time,user,bucket,package,old_version,new_version,tier,note,outcome
07/29/2026,08:32 PM,jetomev,held,linux-zen,7.0.5.zen1-1,7.1.5.zen1-2,1,28 days remaining,cancelled
date,time,user,source,bucket,package,old_version,new_version,tier,note,outcome,detail
09/30/2026,03:00 PM,jetomev,pacman,held,linux-zen,7.0.5.zen1-1,7.2.7.zen1-1,1,20 days remaining,held,
09/30/2026,03:00 PM,jetomev,aur,held,x,1,2,3,\"a, quoted note\",held,
";
        let rows = parse_log(text);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].source, None);
        assert_eq!(rows[0].new_version, "7.1.5.zen1-2");
        assert_eq!(rows[1].source.as_deref(), Some("pacman"));
        assert_eq!(rows[2].package, "x");
        let h = history_from(&rows, "pacman", "linux-zen", "7.0.5.zen1-1");
        assert_eq!(h.first_logged, us_date_to_unix("07/29/2026"));
        assert_eq!(h.versions, vec!["7.1.5.zen1-2", "7.2.7.zen1-1"]);
    }

    #[test]
    fn the_logged_history_respects_source_when_the_log_has_one() {
        let text = "\
date,time,user,source,bucket,package,old_version,new_version,tier,note,outcome,detail
09/30/2026,03:00 PM,u,snap,held,snapd,2.76,2.77,3,,held,
";
        let rows = parse_log(text);
        assert_eq!(history_from(&rows, "pacman", "snapd", "2.76"), LogHistory::default());
    }

    #[test]
    fn short_dates_read_like_a_calendar() {
        assert_eq!(short_date(us_date_to_unix("07/29/2026").unwrap()), "Jul 29");
        assert_eq!(short_date(us_date_to_unix("02/29/2028").unwrap() + 3600), "Feb 29");
        assert_eq!(short_date(0), "Jan 1");
    }

    #[test]
    fn us_dates_convert_to_utc_midnight() {
        assert_eq!(us_date_to_unix("01/01/1970"), Some(0));
        assert_eq!(us_date_to_unix("09/30/2026"), Some(1_790_726_400));
        assert_eq!(us_date_to_unix("13/01/2026"), None);
        assert_eq!(us_date_to_unix("garbage"), None);
    }
}
