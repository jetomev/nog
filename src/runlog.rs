//! v1.0.8 — CSV run-logging.
//!
//! Every `nog update` run appends a CSV record of the report it presented
//! (mirroring the Ready / Held / Unknown table columns) plus the run's
//! outcome, to a per-day log file — `YYYYMMDD nog-update.log` — under the
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
/// line carries the full run context (date/time/user/outcome) so a single
/// file with multiple runs — or a `cat` across files — stays self-describing.
pub const CSV_HEADER: &str =
    "date,time,user,bucket,package,old_version,new_version,tier,note,outcome";

/// One package row of the run record — the update-table columns plus the
/// bucket the package landed in ("ready" / "held" / "unknown").
pub struct RunRow {
    pub bucket: String,
    pub package: String,
    pub old_version: String,
    pub new_version: String,
    /// The tier digit as text; empty for the no-pending-updates marker row.
    pub tier: String,
    pub note: String,
}

/// A full `nog update` run: the banner context, what the report showed, and
/// how the run ended ("installed", "cancelled", "up to date", "all held",
/// "handoff failed (status N)").
pub struct RunRecord {
    pub date: String,
    pub time: String,
    pub user: String,
    pub rows: Vec<RunRow>,
    pub outcome: String,
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
    let ctx = |bucket: &str, pkg: &str, old: &str, new: &str, tier: &str, note: &str| {
        format!(
            "{},{},{},{},{},{},{},{},{},{}\n",
            csv_field(&record.date),
            csv_field(&record.time),
            csv_field(&record.user),
            csv_field(bucket),
            csv_field(pkg),
            csv_field(old),
            csv_field(new),
            csv_field(tier),
            csv_field(note),
            csv_field(&record.outcome),
        )
    };

    if record.rows.is_empty() {
        return ctx("", "", "", "", "", "");
    }
    record.rows.iter()
        .map(|r| ctx(&r.bucket, &r.package, &r.old_version, &r.new_version, &r.tier, &r.note))
        .collect()
}

/// The per-day log filename. The space is deliberate — it matches the
/// project's human-readable file naming (`testing/20260718 - Test Results…`).
pub fn filename_for(yyyymmdd: &str) -> String {
    format!("{} nog-update.log", yyyymmdd)
}

/// Extract the date stamp from a run-log filename; `None` for anything that
/// isn't exactly `YYYYMMDD nog-update.log` (so foreign files in the log
/// directory are never prune candidates).
fn log_date(name: &str) -> Option<&str> {
    let stamp = name.strip_suffix(" nog-update.log")?;
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
pub fn append_run(dir: &str, yyyymmdd: &str, record: &RunRecord) -> Result<PathBuf, String> {
    let dir = expand_home(dir);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("could not create {}: {}", dir, e))?;

    let path = PathBuf::from(&dir).join(filename_for(yyyymmdd));
    let is_new = !path.exists();
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("could not open {}: {}", path.display(), e))?;

    let mut body = String::new();
    if is_new {
        body.push_str(CSV_HEADER);
        body.push('\n');
    }
    body.push_str(&render_run(record));
    f.write_all(body.as_bytes())
        .map_err(|e| format!("could not write {}: {}", path.display(), e))?;
    Ok(path)
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
    use super::*;

    fn record(rows: Vec<RunRow>, outcome: &str) -> RunRecord {
        RunRecord {
            date: "07/29/2026".into(),
            time: "10:15 AM".into(),
            user: "jetomev".into(),
            rows,
            outcome: outcome.into(),
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
    fn render_run_mirrors_table_columns() {
        let rec = record(vec![
            RunRow {
                bucket: "ready".into(),
                package: "libnm".into(),
                old_version: "1.56.1-1".into(),
                new_version: "1.56.1-2".into(),
                tier: "2".into(),
                note: "9 days past window".into(),
            },
            RunRow {
                bucket: "held".into(),
                package: "lib32-nvidia-utils".into(),
                old_version: "580.65-1".into(),
                new_version: "580.76-1".into(),
                tier: "1".into(),
                note: "coupled to nvidia-utils · 12 days".into(),
            },
        ], "installed");
        let csv = render_run(&rec);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines[0],
            "07/29/2026,10:15 AM,jetomev,ready,libnm,1.56.1-1,1.56.1-2,2,9 days past window,installed"
        );
        // Every line carries the full run context and the outcome last.
        assert!(lines[1].starts_with("07/29/2026,10:15 AM,jetomev,held,lib32-nvidia-utils,"));
        assert!(lines[1].ends_with(",installed"));
        // Field count survives the note's interpunct (no stray commas).
        assert_eq!(lines[0].split(',').count(), 10);
    }

    #[test]
    fn render_run_empty_emits_marker_line() {
        let csv = render_run(&record(vec![], "up to date"));
        assert_eq!(csv, "07/29/2026,10:15 AM,jetomev,,,,,,,up to date\n");
    }

    #[test]
    fn render_run_quotes_comma_notes() {
        let rec = record(vec![RunRow {
            bucket: "held".into(),
            package: "systemd".into(),
            old_version: "257.7-1".into(),
            new_version: "258.1-1".into(),
            tier: "1".into(),
            note: "manual sign-off required, run `nog unlock` to release".into(),
        }], "cancelled");
        let csv = render_run(&rec);
        assert!(csv.contains("\"manual sign-off required, run `nog unlock` to release\""));
        // The quoted comma must not change the parsed field count. Cheap
        // check: splitting on `","` boundaries is overkill here; instead
        // confirm exactly one quoted region exists.
        assert_eq!(csv.matches('"').count(), 2);
    }

    #[test]
    fn filename_matches_roadmap_convention() {
        assert_eq!(filename_for("20260729"), "20260729 nog-update.log");
    }

    #[test]
    fn prune_selects_only_expired_run_logs() {
        let names: Vec<String> = vec![
            "20260401 nog-update.log".into(), // before cutoff — prune
            "20260430 nog-update.log".into(), // day before cutoff — prune
            "20260501 nog-update.log".into(), // exactly cutoff — keep
            "20260729 nog-update.log".into(), // fresh — keep
            "notes.txt".into(),               // foreign file — never touch
            "2026 nog-update.log".into(),     // malformed stamp — never touch
            "20260401 something-else.log".into(), // wrong suffix — never touch
        ];
        let pruned = prune_candidates(&names, "20260501");
        assert_eq!(pruned, vec!["20260401 nog-update.log", "20260430 nog-update.log"]);
    }

    #[test]
    fn expand_home_only_touches_tilde_prefix() {
        std::env::set_var("HOME", "/home/testuser");
        assert_eq!(expand_home("~/.local/share/nog/logs"), "/home/testuser/.local/share/nog/logs");
        assert_eq!(expand_home("/var/tmp/logs"), "/var/tmp/logs");
    }
}
