//! v2.0.0 (C4, `nog history`): what nog has done, read from its own CSV logs
//! in the run-logs folder — `<date> nog-runs.csv` (every command: date, time,
//! user, command, status, outcome), `<date> nog-update.csv` (every package an
//! update decided on: source, bucket, package, old, new, tier, note, outcome,
//! detail) and `<date> nog-reboot.csv` (the reboot advice given). Pure
//! parsers; the command module prints or emits JSON.

use std::collections::HashMap;

/// One row of any of the three CSVs, as column name → value.
pub type Row = HashMap<String, String>;

/// Pure. A CSV with a header, quoted fields allowed (nog writes them with
/// `runlog::csv_field`). Rows with fewer fields than the header are padded.
pub fn parse_csv(text: &str) -> Vec<Row> {
    let mut lines = text.lines();
    let Some(header) = lines.next() else { return Vec::new() };
    let cols = split(header);
    lines.filter(|l| !l.trim().is_empty()).map(|l| {
        let f = split(l);
        cols.iter().enumerate().map(|(i, c)| (c.clone(), f.get(i).cloned().unwrap_or_default())).collect()
    }).collect()
}

fn split(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => { cur.push('"'); chars.next(); }
            '"' => quoted = !quoted,
            ',' if !quoted => { out.push(std::mem::take(&mut cur)); }
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// The three kinds of log file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind { Runs, Updates, Reboots }

/// Pure. Which kind a file name is, and its day (`YYYYMMDD`).
pub fn classify(file_name: &str) -> Option<(Kind, String)> {
    let (day, rest) = file_name.split_once(' ')?;
    if day.len() != 8 || !day.chars().all(|c| c.is_ascii_digit()) { return None; }
    let kind = match rest {
        "nog-runs.csv" => Kind::Runs,
        "nog-update.csv" => Kind::Updates,
        "nog-reboot.csv" => Kind::Reboots,
        _ => return None,
    };
    Some((kind, day.to_string()))
}

/// Everything in the folder from `since` (`YYYYMMDD`, inclusive) on, newest
/// day first; within a day, file order (oldest first — the files are appended).
pub fn load(dir: &str, since: &str) -> (Vec<Row>, Vec<Row>, Vec<Row>) {
    let mut files: Vec<(Kind, String, std::path::PathBuf)> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.flatten().filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let (kind, day) = classify(&name)?;
            (day.as_str() >= since).then(|| (kind, day, e.path()))
        }).collect(),
        Err(_) => Vec::new(),
    };
    files.sort_by(|a, b| b.1.cmp(&a.1));
    let (mut runs, mut updates, mut reboots) = (Vec::new(), Vec::new(), Vec::new());
    for (kind, _, path) in files {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let rows = newest_first(parse_csv(&text));
        match kind { Kind::Runs => runs.extend(rows), Kind::Updates => updates.extend(rows), Kind::Reboots => reboots.extend(rows) }
    }
    (runs, updates, reboots)
}

/// Pure. One file's rows, newest run first (F-26): the rows one run wrote together (same date, time
/// and user) keep the order they were written in. Reversing row by row printed a six-line reboot
/// advice upside down, and `nog status` took its last line for the headline.
pub fn newest_first(rows: Vec<Row>) -> Vec<Row> {
    let key = |r: &Row| (r.get("date").cloned(), r.get("time").cloned(), r.get("user").cloned());
    let mut runs: Vec<Vec<Row>> = Vec::new();
    for r in rows {
        match runs.last_mut() {
            Some(run) if key(&run[0]) == key(&r) => run.push(r),
            _ => runs.push(vec![r]),
        }
    }
    runs.into_iter().rev().flatten().collect()
}

/// Pure. `YYYYMMDD` for `days` days before `today` (`YYYYMMDD`), by the
/// calendar — a plain day count, no clock.
pub fn day_minus(today: &str, days: u32) -> String {
    let (y, m, d) = (today[0..4].parse::<i64>().unwrap_or(2000), today[4..6].parse::<u32>().unwrap_or(1), today[6..8].parse::<u32>().unwrap_or(1));
    let mut n = days_from_civil(y, m, d) - days as i64;
    if n < 0 { n = 0; }
    let (y, m, d) = civil_from_days(n);
    format!("{:04}{:02}{:02}", y, m, d)
}

// Howard Hinnant's date algorithms (public domain).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nogs_own_csvs_are_read_back_quotes_included() {
        let text = "date,time,user,command,status,outcome\n10/14/2026,04:43 PM,jetomev,\"install a, b\",0,done\n10/14/2026,04:47 PM,jetomev,install zzz-nope,1,stopped\n";
        let rows = parse_csv(text);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["command"], "install a, b");
        assert_eq!(rows[1]["outcome"], "stopped");
        assert_eq!(parse_csv("").len(), 0);
        assert_eq!(parse_csv("a,b\n1\n")[0]["b"], "", "a short row is padded");
    }

    #[test]
    fn the_file_names_tell_the_kind_and_the_day() {
        assert_eq!(classify("20261014 nog-runs.csv"), Some((Kind::Runs, "20261014".into())));
        assert_eq!(classify("20261013 nog-reboot.csv"), Some((Kind::Reboots, "20261013".into())));
        assert_eq!(classify("20261014-171609 keep vlc.log"), None);
        assert_eq!(classify("holds.tsv"), None);
    }

    #[test]
    fn a_runs_lines_stay_in_order_newest_run_first() {
        let text = "date,time,user,level,advice\n\
            10/21/2026,09:00 AM,jetomev,NOTE,NOTE: an older run.\n\
            10/22/2026,05:00 PM,jetomev,IMPORTANT,IMPORTANT: It is highly recommended to reboot the system!\n\
            10/22/2026,05:00 PM,jetomev,IMPORTANT,grub and lib32-mesa were updated this run.\n\
            10/22/2026,05:00 PM,jetomev,NOTE,mesa was updated this run.\n";
        let rows = newest_first(parse_csv(text));
        let advice: Vec<&str> = rows.iter().map(|r| r["advice"].as_str()).collect();
        assert_eq!(advice, vec![
            "IMPORTANT: It is highly recommended to reboot the system!",
            "grub and lib32-mesa were updated this run.",
            "mesa was updated this run.",
            "NOTE: an older run.",
        ]);
    }

    #[test]
    fn days_are_counted_by_the_calendar() {
        assert_eq!(day_minus("20261014", 30), "20260914");
        assert_eq!(day_minus("20260301", 1), "20260228");
        assert_eq!(day_minus("20260101", 1), "20251231");
        assert_eq!(day_minus("20261014", 0), "20261014");
    }
}
