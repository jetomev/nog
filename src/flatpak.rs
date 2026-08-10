// flatpak.rs — Flatpak backend (v1.1.0, the multi-source arc C1)
//
// nog never talks to OSTree itself. It delegates to the `flatpak` CLI the
// same way aur.rs delegates to yay/paru: (a) detect the binary, (b) ask it
// what app updates are pending, (c) resolve commit dates so the tier hold
// windows apply, (d) hand off the actual update.
//
// Design rulings (docs/v2-design.md):
//   - On-demand backend: flatpak absent is NOT an error — the source simply
//     reports unavailable and nog moves on. The install-flatpak offer lives
//     in the C3 install chain, not here.
//   - Tiers apply: flatpak refs age through the same hold windows, keyed on
//     the remote commit's timestamp (the moment the update was published —
//     the builddate analog). Default tier for app IDs is Tier 3.
//   - Fail closed: if the remote can't be queried, we report the error and
//     the caller treats flatpak as "couldn't check" — never "all quiet".
//
// CLI surface used (stable since flatpak 1.x):
//   flatpak list --app --columns=application,version,origin      installed apps
//   flatpak remote-ls --updates --app --columns=application,version,origin
//                                                                pending updates
//   flatpak remote-info <origin> <app-id>                        "Date:" of the
//                                                                pending commit
//   flatpak update -y [--noninteractive] <app-ids…>              apply

use std::collections::HashMap;
use std::process::{Command, ExitStatus};

use crate::pacman::PendingUpdate;

/// PATH check, same convention as aur::is_on_path.
pub fn is_available() -> bool {
    Command::new("flatpak")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// One pending flatpak app update. `origin` is the remote (e.g. "flathub") —
/// needed for the remote-info date lookup and shown as the source detail.
#[derive(Debug, Clone, PartialEq)]
pub struct FlatpakUpdate {
    pub app_id: String,
    pub origin: String,
    pub new_version: String, // may be empty — many flatpaks don't set a version
}

/// Ask flatpak which installed apps have updates pending.
///
/// `flatpak remote-ls --updates` exits 0 with empty stdout when there is
/// nothing to do, so — unlike the AUR helpers — empty output is unambiguous
/// on its own. A non-zero exit is a genuine failure (remote unreachable):
/// bubble it up so the caller fails CLOSED, never "assume all quiet".
pub fn pending_updates() -> Result<Vec<FlatpakUpdate>, String> {
    let output = Command::new("flatpak")
        .args(["remote-ls", "--updates", "--app", "--columns=application,version,origin"])
        .output()
        .map_err(|e| format!("failed to launch flatpak: {}", e))?;

    if !output.status.success() {
        let msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if msg.is_empty() {
            format!("flatpak remote-ls exited with status {}", output.status)
        } else {
            msg
        });
    }

    Ok(parse_columns_output(&String::from_utf8_lossy(&output.stdout)))
}

/// Installed app versions, for the Old column of the update table.
/// app-id → version (possibly empty).
pub fn installed_versions() -> HashMap<String, String> {
    let output = match Command::new("flatpak")
        .args(["list", "--app", "--columns=application,version"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return HashMap::new(), // soft-fail: Old column shows "?"
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut out = HashMap::new();
    for line in stdout.lines() {
        let mut parts = line.split('\t');
        if let Some(id) = parts.next() {
            let id = id.trim();
            if id.is_empty() { continue; }
            out.insert(id.to_string(), parts.next().unwrap_or("").trim().to_string());
        }
    }
    out
}

/// Parse `--columns=application,version,origin` output (tab-separated).
/// Kept pure for tests.
pub fn parse_columns_output(stdout: &str) -> Vec<FlatpakUpdate> {
    let mut updates = Vec::new();
    for line in stdout.lines() {
        let line = line.trim_end();
        if line.is_empty() { continue; }
        let mut parts = line.split('\t');
        let app_id = match parts.next() {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => continue,
        };
        let new_version = parts.next().unwrap_or("").trim().to_string();
        let origin = parts.next().unwrap_or("flathub").trim().to_string();
        updates.push(FlatpakUpdate { app_id, origin, new_version });
    }
    updates
}

/// Resolve the publish date of each pending update's remote commit, as a Unix
/// timestamp — the input for the tier hold windows. One `remote-info` call
/// per app (pending flatpak lists are typically tiny). Apps whose date can't
/// be resolved are omitted → the caller buckets them Unknown, matching the
/// AUR fallback behavior exactly.
pub fn commit_dates_for(updates: &[FlatpakUpdate]) -> HashMap<String, u64> {
    let mut out = HashMap::new();
    for u in updates {
        let output = match Command::new("flatpak")
            .args(["remote-info", &u.origin, &u.app_id])
            .output()
        {
            Ok(o) if o.status.success() => o,
            _ => continue,
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(ts) = parse_remote_info_date(&stdout) {
            out.insert(u.app_id.clone(), ts);
        }
    }
    out
}

/// Extract the `Date:` line from `flatpak remote-info` output and convert it
/// to Unix time via `date -d` (the aur.rs precedent — no new Rust dep).
pub fn parse_remote_info_date(stdout: &str) -> Option<u64> {
    for line in stdout.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Date:") {
            let s = rest.trim();
            let out = Command::new("date").arg("-d").arg(s).arg("+%s").output().ok()?;
            if !out.status.success() { return None; }
            return String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().ok();
        }
    }
    None
}

/// Apply updates for exactly the given app IDs (the Ready bucket). Held apps
/// are simply not listed — flatpak has no --ignore, so "update only these"
/// IS the exclusion mechanism. Runs as the invoking user; flatpak escalates
/// via polkit itself for system installations.
///
/// Issue #8 (dogfood 2026-08-10): NOT `--noninteractive`. The user must see
/// the work happening — flatpak's own transaction table and progress bars,
/// the same way the pacman/AUR handoff shows its own. `-y` still answers the
/// confirmation (nog already gated the run at its own Proceed? prompt), but
/// flatpak keeps its voice.
pub fn update(app_ids: &[String]) -> ExitStatus {
    let mut args: Vec<&str> = vec!["update", "-y"];
    args.extend(app_ids.iter().map(|s| s.as_str()));
    Command::new("flatpak")
        .args(&args)
        .status()
        .unwrap_or_else(|e| panic!("nog: failed to launch flatpak: {}", e))
}

/// Which flatpak refs may be handed to `flatpak update` this run.
///
/// The hold mechanism for flatpak is *naming*: flatpak has no `--ignore`, so
/// nog passes exactly the refs it cleared and nothing else. Cleared means
/// Ready, or an Unknown the user approved (i.e. not in the ignore list).
/// Held refs are structurally unreachable here — this function is the single
/// place that decides, so the rule is testable rather than merely true.
pub fn apply_list(
    flatpak_names: &[String],
    ready: &[String],
    unknown: &[String],
    ignore: &[String],
) -> Vec<String> {
    let is_flatpak = |n: &String| flatpak_names.iter().any(|f| f == n);
    let mut out: Vec<String> = ready.iter().filter(|n| is_flatpak(n)).cloned().collect();
    out.extend(
        unknown.iter()
            .filter(|n| is_flatpak(n) && !ignore.iter().any(|i| i == *n))
            .cloned(),
    );
    out
}

/// Convert a FlatpakUpdate into the shared PendingUpdate shape used by the
/// update pipeline. Empty versions render as "?" so the table stays honest
/// about flatpak's loose version metadata.
pub fn to_pending(u: &FlatpakUpdate, installed: &HashMap<String, String>) -> PendingUpdate {
    let old = installed.get(&u.app_id).map(|s| s.as_str()).unwrap_or("");
    PendingUpdate {
        name: u.app_id.clone(),
        old_version: if old.is_empty() { "?".to_string() } else { old.to_string() },
        new_version: if u.new_version.is_empty() { "?".to_string() } else { u.new_version.clone() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tab_separated_updates() {
        let out = "org.gimp.GIMP\t3.0.4\tflathub\ncom.spotify.Client\t\tflathub\n";
        let got = parse_columns_output(out);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].app_id, "org.gimp.GIMP");
        assert_eq!(got[0].new_version, "3.0.4");
        assert_eq!(got[0].origin, "flathub");
        assert_eq!(got[1].new_version, ""); // versionless flatpak — common
    }

    #[test]
    fn empty_output_means_no_updates() {
        assert!(parse_columns_output("").is_empty());
        assert!(parse_columns_output("\n\n").is_empty());
    }

    #[test]
    fn remote_info_date_line_is_found() {
        let out = "\n        ID: org.gimp.GIMP\n    Branch: stable\n      Date: 2026-08-01 12:30:00 +0000\n";
        let ts = parse_remote_info_date(out);
        assert!(ts.is_some());
        assert!(ts.unwrap() > 1_700_000_000);
    }

    #[test]
    fn remote_info_without_date_is_none() {
        assert_eq!(parse_remote_info_date("ID: x\nBranch: y\n"), None);
    }

    #[test]
    fn apply_list_names_only_cleared_flatpaks() {
        let flatpaks = vec!["org.gimp.GIMP".to_string(), "com.spotify.Client".to_string(),
                            "org.kde.kate".to_string()];
        let ready = vec!["org.gimp.GIMP".to_string(), "vlc".to_string()]; // vlc = pacman
        let unknown = vec!["com.spotify.Client".to_string(), "org.kde.kate".to_string()];
        // kate was skipped by the user at the Unknown prompt:
        let ignore = vec!["org.kde.kate".to_string(), "systemd".to_string()];

        let got = apply_list(&flatpaks, &ready, &unknown, &ignore);
        assert_eq!(got, vec!["org.gimp.GIMP".to_string(), "com.spotify.Client".to_string()]);
        // Non-flatpak Ready packages never leak into the flatpak transaction:
        assert!(!got.contains(&"vlc".to_string()));
        // Skipped/held refs are never named — flatpak's only exclusion is silence:
        assert!(!got.contains(&"org.kde.kate".to_string()));
    }

    #[test]
    fn held_flatpaks_can_never_be_applied() {
        // A held ref appears in neither `ready` nor `unknown` — the caller's
        // buckets are disjoint. Nothing to name, nothing to update.
        let flatpaks = vec!["org.gimp.GIMP".to_string()];
        let got = apply_list(&flatpaks, &[], &[], &[]);
        assert!(got.is_empty());
    }

    #[test]
    fn to_pending_marks_missing_versions_honestly() {
        let u = FlatpakUpdate {
            app_id: "com.spotify.Client".into(),
            origin: "flathub".into(),
            new_version: "".into(),
        };
        let p = to_pending(&u, &HashMap::new());
        assert_eq!(p.old_version, "?");
        assert_eq!(p.new_version, "?");
    }
}
