//! v1.6.0 — the whole run, as it appeared on screen, kept as a `.log` file
//! (Javier, 3 Oct 2026: "the logs, just the output of the nog run as is …
//! read the full run, I think, can be more valuable").
//!
//! The CSV logs keep one line per package; they can't show pacman's
//! questions, an AUR build or an error message. So a person's run is recorded
//! with util-linux `script`, which runs nog in a terminal of its own and
//! copies everything shown on screen into the file. You answer prompts as
//! usual; what you type is not recorded (`--log-out` keeps only the output,
//! and sudo never shows a password anyway).
//!
//! nog starts itself again inside `script`; the inner run sees
//! `NOG_RECORDING` (the file's path) and doesn't record twice. Not recorded:
//! JSON runs (a program asking), and runs without a terminal, or without
//! `script` installed: those run exactly as before.
//!
//! Files: `<logs>/<YYYYMMDD-HHMMSS> <command>.log`, deleted after 30 days.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Javier, 3 Oct 2026: "30 days is more than enough".
pub const KEEP_DAYS: u64 = 30;

/// Set inside a recorded run: the path of its `.log` file.
pub const ENV: &str = "NOG_RECORDING";

/// The `.log` file of this run, when it is being recorded.
pub fn current() -> Option<PathBuf> {
    std::env::var_os(ENV).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// Run this nog again inside `script` and exit with its status, or return
/// (and run unrecorded) when that isn't possible.
pub fn rerun_recorded(dir: &Path) {
    use std::io::IsTerminal;
    if current().is_some() || !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    let Some(stamp) = stamp() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    prune(dir, KEEP_DAYS);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let file = dir.join(file_name(&stamp, &args));
    // script runs the command with $SHELL -c; fish quotes differently, so sh
    // runs it, and nog gets the person's own SHELL back.
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let mut line = format!("SHELL={} exec {}", quote(&shell), quote(&exe.to_string_lossy()));
    for a in &args {
        line.push(' ');
        line.push_str(&quote(a));
    }
    let status = Command::new("script")
        .args(["-q", "-e", "-f", "--log-out"])
        .arg(&file)
        .args(["-c", &line])
        .env("SHELL", "/bin/sh")
        .env(ENV, &file)
        .status();
    match status {
        // the inner nog printed its own start, end and closing note
        Ok(s) => std::process::exit(s.code().unwrap_or(1)),
        Err(_) => {
            let _ = std::fs::remove_file(&file);    // no `script`: run unrecorded
        }
    }
}

/// `20261003-211604`, local time.
fn stamp() -> Option<String> {
    let out = Command::new("date").arg("+%Y%m%d-%H%M%S").output().ok()?;
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (s.len() == 15).then_some(s)
}

/// `20261003-211604 install nog-1.6.0rc3-1-x86_64.pkg.tar.zst.log`: the
/// command in words, a path by its last part, nothing a file name can't hold.
/// Pure.
pub fn file_name(stamp: &str, args: &[String]) -> String {
    let words: Vec<String> = args.iter()
        .map(|a| a.rsplit('/').next().unwrap_or(a).to_string())
        .filter(|a| !a.is_empty())
        .collect();
    let mut cmd: String = words.join(" ").chars()
        .map(|c| if c.is_alphanumeric() || " .,_+-".contains(c) { c } else { '-' })
        .collect();
    if cmd.chars().count() > 80 {
        cmd = cmd.chars().take(80).collect();
    }
    format!("{} {}.log", stamp, cmd.trim())
}

/// Shell single quotes: `it's` → `'it'\''s'`. Pure.
pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Delete this folder's `.log` files older than `days`. The CSV logs are
/// never touched here.
fn prune(dir: &Path, days: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let limit = std::time::Duration::from_secs(days * 86_400);
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().map_or(true, |x| x != "log") {
            continue;
        }
        let old = e.metadata().and_then(|m| m.modified()).ok()
            .and_then(|t| t.elapsed().ok())
            .map_or(false, |age| age > limit);
        if old {
            let _ = std::fs::remove_file(&p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_runs_file_is_named_by_its_time_and_command() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(file_name("20261003-211604", &a(&["install", "/home/j/x/nog-1.6.0rc3-1-x86_64.pkg.tar.zst"])),
            "20261003-211604 install nog-1.6.0rc3-1-x86_64.pkg.tar.zst.log");
        assert_eq!(file_name("20261003-211604", &a(&["update", "--keep", "ldb,smbclient"])),
            "20261003-211604 update --keep ldb,smbclient.log");
        assert_eq!(file_name("20261003-211604", &a(&["install", "a*b"])), "20261003-211604 install a-b.log");
        assert_eq!(file_name("20261003-211604", &a(&["clean"])), "20261003-211604 clean.log");
    }

    #[test]
    fn quoting_survives_a_quote() {
        assert_eq!(quote("it's"), "'it'\\''s'");
        assert_eq!(quote("/usr/bin/nog"), "'/usr/bin/nog'");
    }

    #[test]
    fn only_old_logs_go() {
        let dir = std::env::temp_dir().join(format!("nog-prune-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let old = dir.join("20260801-100000 update.log");
        let new = dir.join("20261003-100000 update.log");
        let csv = dir.join("20260801 nog-runs.csv");
        for p in [&old, &new, &csv] {
            std::fs::write(p, "x").unwrap();
        }
        let long_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(40 * 86_400);
        for p in [&old, &csv] {
            std::fs::File::options().write(true).open(p).unwrap().set_modified(long_ago).unwrap();
        }
        prune(&dir, 30);
        assert!(!old.exists(), "a log older than 30 days goes");
        assert!(new.exists(), "a recent one stays");
        assert!(csv.exists(), "the CSV logs are never touched");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
