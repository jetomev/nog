//! v1.0.8 — CSV run-logging.
//!
//! Every `nog update` run appends a CSV record of the report it presented
//! (mirroring the Ready / Held / Unknown table columns) plus the run's
//! outcome, to a per-day log file — `YYYYMMDD nog-update.csv` — under the
//! `[paths] run_logs` directory. Files older than the retention window
//! (3 months) are pruned after each successful write.
//!
//! Design mirrors `format_table()`: CSV rendering and the retention decision
//! are pure, unit-tested functions; the thin IO wrappers soft-fail (callers
//! warn and continue) — logging must never block or abort an update.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Retention window. "Keep 3 months of history" — measured in days because
/// the pruning cutoff is computed by `date -d "90 days ago"`.
pub const RETENTION_DAYS: u32 = 90;

/// Column header written once when a day's log file is created. Every data
/// line carries the full run context (date/time/user) so a single file with
/// multiple runs — or a `cat` across files — stays self-describing.
///
/// v1.4.3 changed it, once, for three issues together:
///   * `source` (#20) — which package manager the row belongs to. A name is
///     not unique across sources: `snapd` the Arch package and `snapd` the
///     snap were two identical-looking rows.
///   * `outcome` is now **per package** (#19). It used to be the run's
///     verdict copied onto every row, so 3,782 held packages were recorded
///     as `installed`.
///   * `detail` (#21) — why a step did not complete, taken from the tool's
///     own error output. Empty when there is nothing to explain.
///
/// Files written before v1.4.3 keep their old ten-column header and remain
/// readable on their own terms; see `append_run` for the one day that holds
/// both.
pub const CSV_HEADER: &str =
    "date,time,user,source,bucket,package,old_version,new_version,tier,note,outcome,detail";

/// One package row of the run record — the update-table columns, the bucket
/// the package landed in ("ready" / "held" / "unknown"), and what became of
/// it.
#[derive(Clone)]
pub struct RunRow {
    /// "pacman", "aur", "flatpak" or "snap".
    pub source: String,
    pub bucket: String,
    pub package: String,
    pub old_version: String,
    pub new_version: String,
    /// The tier digit as text; empty for the no-pending-updates marker row.
    pub tier: String,
    pub note: String,
    /// What happened to this package: "installed", "held", "skipped",
    /// "cancelled", "not run", or "did not complete (status N)".
    pub outcome: String,
    /// Why, when the outcome needs a why. Empty otherwise.
    pub detail: String,
}

/// A full `nog update` run: the banner context and one row per package.
pub struct RunRecord {
    pub date: String,
    pub time: String,
    pub user: String,
    pub rows: Vec<RunRow>,
    /// Written as the outcome of the single marker line a run with no rows
    /// leaves behind ("up to date"). Unused when there are rows.
    pub marker: String,
}

/// Escape one CSV field per RFC 4180: quote when the value contains a comma,
/// a double quote, or a line break; double any embedded quotes.
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Render a run as CSV data lines (no header — `append_run` owns that, since
/// the header belongs to the file, not the run). A run with no pending
/// updates still emits one marker line with empty package columns, so the
/// log remains a complete history of every `nog update` invocation.
pub fn render_run(record: &RunRecord) -> String {
    let line = |fields: [&str; 9]| {
        let mut out = vec![
            csv_field(&record.date),
            csv_field(&record.time),
            csv_field(&record.user),
        ];
        out.extend(fields.iter().map(|f| csv_field(f)));
        out.join(",") + "\n"
    };

    if record.rows.is_empty() {
        return line(["", "", "", "", "", "", "", &record.marker, ""]);
    }
    record.rows.iter()
        .map(|r| line([
            &r.source, &r.bucket, &r.package, &r.old_version, &r.new_version,
            &r.tier, &r.note, &r.outcome, &r.detail,
        ]))
        .collect()
}

/// The per-day log filename. The space is deliberate — it matches the
/// project's human-readable file naming (`testing/20260718 - Test Results…`).
/// The `.csv` extension is deliberate too: spreadsheet apps refuse to import
/// a `.log`, and the content has been CSV from day one.
pub fn filename_for(yyyymmdd: &str) -> String {
    format!("{} nog-update.csv", yyyymmdd)
}

/// v1.5.8: the per-day log of every nog run, whatever the command — one
/// line each. Javier's rule (2 Oct 2026): every run ends by saying where it
/// was logged, so every run has to be logged somewhere.
pub fn runs_filename_for(yyyymmdd: &str) -> String {
    format!("{} nog-runs.csv", yyyymmdd)
}

/// Header of the runs file.
pub const RUNS_HEADER: &str = "date,time,user,command,status,outcome";

/// Append one line to the day's runs file.
pub fn append_runs_row(dir: &str, yyyymmdd: &str, row: &[&str]) -> Result<PathBuf, String> {
    let line: Vec<String> = row.iter().map(|f| csv_field(f)).collect();
    append_block(dir, &runs_filename_for(yyyymmdd), RUNS_HEADER, &format!("{}\n", line.join(",")))
}

/// The per-day reboot-advice filename (v1.4.3, issue #22).
pub fn reboot_filename_for(yyyymmdd: &str) -> String {
    format!("{} nog-reboot.csv", yyyymmdd)
}

/// Header of the reboot-advice file: one line per line of advice nog printed.
/// `level` is the advice's own first word — `IMPORTANT` for a verified broken
/// state, `NOTE` for advice, `checked` when the probes ran and found nothing
/// to say, so a quiet result still leaves a trace.
pub const REBOOT_HEADER: &str = "date,time,user,level,advice";

/// Extract the date stamp from a log filename; `None` for anything that
/// isn't exactly `YYYYMMDD nog-update.csv` or `YYYYMMDD nog-reboot.csv` (so
/// foreign files in the log directory are never prune candidates).
fn log_date(name: &str) -> Option<&str> {
    let stamp = name
        .strip_suffix(" nog-update.csv")
        .or_else(|| name.strip_suffix(" nog-reboot.csv"))
        .or_else(|| name.strip_suffix(" nog-runs.csv"))?;
    if stamp.len() == 8 && stamp.bytes().all(|b| b.is_ascii_digit()) {
        Some(stamp)
    } else {
        None
    }
}

/// Pure retention decision: which of these filenames are run logs dated
/// strictly before the cutoff. Zero-padded YYYYMMDD compares correctly as a
/// plain string, so no date arithmetic is needed here.
pub fn prune_candidates<'a>(names: &'a [String], cutoff_yyyymmdd: &str) -> Vec<&'a str> {
    names.iter()
        .filter(|n| matches!(log_date(n), Some(d) if d < cutoff_yyyymmdd))
        .map(|n| n.as_str())
        .collect()
}

/// Expand a leading `~/` against $HOME so `[paths] run_logs` can use the
/// portable spelling. Anything else passes through untouched.
pub fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}/{}", home, rest);
        }
    }
    path.to_string()
}

/// `(today, cutoff)` as YYYYMMDD via the system `date` — nog already spawns
/// subprocesses and stays free of datetime crates. `None` if `date` is
/// unavailable or emits something unexpected; the caller skips logging.
pub fn today_and_cutoff() -> Option<(String, String)> {
    let today = date_stamp(&["+%Y%m%d"])?;
    let cutoff = date_stamp(&["-d", &format!("{} days ago", RETENTION_DAYS), "+%Y%m%d"])?;
    Some((today, cutoff))
}

fn date_stamp(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("date").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.len() == 8 && s.bytes().all(|b| b.is_ascii_digit()) {
        Some(s)
    } else {
        None
    }
}

/// Append a run to the day's log file, creating the directory and writing
/// the CSV header if the file is new. Returns the path written, or a
/// human-readable error for the caller's soft-fail warning.
///
/// The day nog is upgraded to v1.4.3 can already hold rows in the old
/// ten-column layout. Appending twelve-column rows under that header would
/// make every later row read wrongly, so when the existing header is not the
/// current one a blank line and the new header go in first. The file then
/// reads as two tables, each under its own header.
pub fn append_run(dir: &str, yyyymmdd: &str, record: &RunRecord) -> Result<PathBuf, String> {
    append_block(dir, &filename_for(yyyymmdd), CSV_HEADER, &render_run(record))
}

/// Append reboot advice to the day's `nog-reboot.csv` (issue #22). Written
/// after the run log is already on disk, so nothing here can cost the main
/// record — the property v1.4.0 placed the advice after the log to keep.
pub fn append_reboot(
    dir: &str,
    yyyymmdd: &str,
    date: &str,
    time: &str,
    user: &str,
    lines: &[(String, String)],
) -> Result<PathBuf, String> {
    let body: String = lines
        .iter()
        .map(|(level, text)| {
            [date, time, user, level.as_str(), text.as_str()]
                .iter()
                .map(|f| csv_field(f))
                .collect::<Vec<_>>()
                .join(",")
                + "\n"
        })
        .collect();
    append_block(dir, &reboot_filename_for(yyyymmdd), REBOOT_HEADER, &body)
}

fn append_block(dir: &str, filename: &str, header: &str, body: &str) -> Result<PathBuf, String> {
    let dir = expand_home(dir);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("could not create {}: {}", dir, e))?;

    let path = PathBuf::from(&dir).join(filename);
    let existing = fs::read_to_string(&path).ok();
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("could not open {}: {}", path.display(), e))?;

    let mut out = String::new();
    out.push_str(&header_prefix(existing.as_deref(), header));
    out.push_str(body);
    f.write_all(out.as_bytes())
        .map_err(|e| format!("could not write {}: {}", path.display(), e))?;
    Ok(path)
}

/// What must be written before new rows: the header for a new or empty file,
/// a blank line and the header when the file's *latest* header differs (the
/// upgrade day), nothing otherwise.
fn header_prefix(existing: Option<&str>, header: &str) -> String {
    let Some(text) = existing.filter(|t| !t.trim().is_empty()) else {
        return format!("{}\n", header);
    };
    // The last header in the file governs the rows being appended to it.
    let current = text
        .lines()
        .filter(|l| l.starts_with("date,time,user,"))
        .last();
    if current == Some(header) {
        String::new()
    } else {
        format!("\n{}\n", header)
    }
}

/// Delete run logs dated before the cutoff. Returns the pruned filenames.
/// A missing directory is simply "nothing to prune", not an error.
pub fn prune_old(dir: &str, cutoff_yyyymmdd: &str) -> Result<Vec<String>, String> {
    let dir = expand_home(dir);
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(Vec::new()),
    };
    let names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();

    let mut pruned = Vec::new();
    for name in prune_candidates(&names, cutoff_yyyymmdd) {
        let path = PathBuf::from(&dir).join(name);
        fs::remove_file(&path)
            .map_err(|e| format!("could not remove {}: {}", path.display(), e))?;
        pruned.push(name.to_string());
    }
    Ok(pruned)
}

#[cfg(test)]
mod tests {

    #[test]
    fn the_runs_log_is_named_by_day_and_pruned_like_the_others() {
        assert_eq!(runs_filename_for("20261002"), "20261002 nog-runs.csv");
        assert_eq!(log_date("20261002 nog-runs.csv"), Some("20261002"));
        assert_eq!(log_date("notes nog-runs.csv"), None);
    }

    use super::*;

    fn record(rows: Vec<RunRow>, marker: &str) -> RunRecord {
        RunRecord {
            date: "07/29/2026".into(),
            time: "10:15 AM".into(),
            user: "jetomev".into(),
            rows,
            marker: marker.into(),
        }
    }

    fn row(source: &str, bucket: &str, pkg: &str, tier: &str, note: &str, outcome: &str) -> RunRow {
        RunRow {
            source: source.into(),
            bucket: bucket.into(),
            package: pkg.into(),
            old_version: "1-1".into(),
            new_version: "2-1".into(),
            tier: tier.into(),
            note: note.into(),
            outcome: outcome.into(),
            detail: String::new(),
        }
    }

    #[test]
    fn csv_field_quotes_only_when_needed() {
        assert_eq!(csv_field("plain-1.2.3"), "plain-1.2.3");
        assert_eq!(csv_field("has,comma"), "\"has,comma\"");
        assert_eq!(csv_field("has \"quote\""), "\"has \"\"quote\"\"\"");
        assert_eq!(csv_field("line\nbreak"), "\"line\nbreak\"");
    }

    #[test]
    fn render_run_writes_one_outcome_per_package() {
        // Issue #19: a held row says held, whatever the run did.
        let rec = record(vec![
            row("pacman", "ready", "libnm", "2", "9 days past window", "installed"),
            row("pacman", "held", "gimp", "3", "1 day remaining", "held"),
        ], "");
        let csv = render_run(&rec);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(
            lines[0],
            "07/29/2026,10:15 AM,jetomev,pacman,ready,libnm,1-1,2-1,2,9 days past window,installed,"
        );
        assert!(lines[1].contains(",held,gimp,"));
        assert!(lines[1].ends_with(",held,"), "held row claims: {}", lines[1]);
        assert_eq!(lines[0].split(',').count(), CSV_HEADER.split(',').count());
    }

    #[test]
    fn the_two_snapd_rows_are_told_apart() {
        // Issue #20, from the 2026-09-15 log.
        let rec = record(vec![
            row("pacman", "ready", "snapd", "3", "hold just expired", "installed"),
            row("snap", "ready", "snapd", "3", "hold just expired", "installed"),
        ], "");
        let csv = render_run(&rec);
        let lines: Vec<&str> = csv.lines().collect();
        assert!(lines[0].contains(",pacman,ready,snapd,"));
        assert!(lines[1].contains(",snap,ready,snapd,"));
    }

    #[test]
    fn a_failure_reason_is_quoted_into_detail() {
        let mut r = row("aur", "ready", "discord", "3", "", "did not complete (status 1)");
        r.detail = "error: failed to build 'discord', see log, line 12".into();
        let csv = render_run(&record(vec![r], ""));
        assert!(csv.trim_end().ends_with(
            ",did not complete (status 1),\"error: failed to build 'discord', see log, line 12\""
        ));
    }

    #[test]
    fn render_run_empty_emits_marker_line() {
        let csv = render_run(&record(vec![], "up to date"));
        assert_eq!(csv, "07/29/2026,10:15 AM,jetomev,,,,,,,,up to date,\n");
        assert_eq!(csv.trim_end().split(',').count(), CSV_HEADER.split(',').count());
    }

    #[test]
    fn header_goes_in_once_and_again_only_when_the_layout_changes() {
        assert_eq!(header_prefix(None, CSV_HEADER), format!("{}\n", CSV_HEADER));
        assert_eq!(header_prefix(Some(""), CSV_HEADER), format!("{}\n", CSV_HEADER));
        let current = format!("{}\nrow\n", CSV_HEADER);
        assert_eq!(header_prefix(Some(&current), CSV_HEADER), "");
        // The upgrade day: rows under the pre-v1.4.3 header already exist.
        let old = "date,time,user,bucket,package,old_version,new_version,tier,note,outcome\nrow\n";
        assert_eq!(header_prefix(Some(old), CSV_HEADER), format!("\n{}\n", CSV_HEADER));
        // ...and after that the new header governs, so it is not repeated.
        let both = format!("{}\n{}\nrow\n", old, CSV_HEADER);
        assert_eq!(header_prefix(Some(&both), CSV_HEADER), "");
    }

    #[test]
    fn filename_is_spreadsheet_friendly_csv() {
        assert_eq!(filename_for("20260729"), "20260729 nog-update.csv");
        assert_eq!(reboot_filename_for("20260729"), "20260729 nog-reboot.csv");
    }

    #[test]
    fn prune_selects_only_expired_run_logs() {
        let names: Vec<String> = vec![
            "20260401 nog-update.csv".into(), // before cutoff — prune
            "20260430 nog-update.csv".into(), // day before cutoff — prune
            "20260501 nog-update.csv".into(), // exactly cutoff — keep
            "20260729 nog-update.csv".into(), // fresh — keep
            "notes.txt".into(),               // foreign file — never touch
            "2026 nog-update.csv".into(),     // malformed stamp — never touch
            "20260401 something-else.csv".into(), // wrong suffix — never touch
            "20260402 nog-reboot.csv".into(), // reboot log, expired — prune
            "20260801 nog-reboot.csv".into(), // reboot log, fresh — keep
        ];
        let pruned = prune_candidates(&names, "20260501");
        assert_eq!(
            pruned,
            vec!["20260401 nog-update.csv", "20260430 nog-update.csv", "20260402 nog-reboot.csv"]
        );
    }

    #[test]
    fn expand_home_only_touches_tilde_prefix() {
        std::env::set_var("HOME", "/home/testuser");
        assert_eq!(expand_home("~/.local/share/nog/logs"), "/home/testuser/.local/share/nog/logs");
        assert_eq!(expand_home("/var/tmp/logs"), "/var/tmp/logs");
    }
}
