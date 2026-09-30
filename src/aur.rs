// aur.rs — AUR helper detection and delegation
//
// nog never builds AUR packages itself. It delegates to yay or paru, which
// already handle the hard parts: fetching PKGBUILDs, running makepkg as the
// invoking user, and sudo-ing to pacman for the install step. nog's job is to
// (a) pick a helper, (b) ask it what AUR updates are pending, and (c) hand off
// the transaction when the time comes.
//
// Helper choice comes from `[aur] helper` in nog.conf:
//   "auto" — prefer yay, fall back to paru, skip AUR support if neither is installed
//   "yay"  — require yay; error if not installed
//   "paru" — require paru; error if not installed
//   "none" — disable AUR paths entirely (official repos only)
//
// Both yay and paru share the pacman CLI surface we care about:
//   <helper> -Qua               list pending AUR updates (pkg oldver -> newver)
//   <helper> -Sai <pkgs>        AUR package info, for build dates
//   <helper> -S <pkgs>          install/rebuild, tries sync repos first then AUR
// So once we pick a binary name, everything downstream is identical.
//
// Note what is NOT in that list: nog no longer asks a helper for a full `-Syu`
// (issue #10). Official packages are pacman's own step, and the AUR step is
// handed an explicit list of names. That keeps nog on the part of the surface
// both helpers implement identically, and off flags like `-Sua` whose
// sysupgrade-filtering behaviour is helper-specific and hard to verify.

use std::collections::HashMap;
use std::process::{Command, ExitStatus};

use crate::pacman::{PendingUpdate, Source};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Helper {
    Yay,
    Paru,
}

impl Helper {
    pub fn binary(&self) -> &'static str {
        match self {
            Helper::Yay  => "yay",
            Helper::Paru => "paru",
        }
    }
}

impl std::fmt::Display for Helper {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.binary())
    }
}

/// Resolve the user's `[aur] helper` preference against what's actually on PATH.
///
/// Returns:
///   - `Ok(Some(helper))` — an AUR helper is available and should be used
///   - `Ok(None)`         — AUR support is disabled ("none") or "auto" found nothing
///   - `Err(msg)`         — user requested a specific helper that isn't installed
pub fn detect_helper(preference: &str) -> Result<Option<Helper>, String> {
    match preference.trim().to_lowercase().as_str() {
        "none" => Ok(None),
        "auto" => {
            // yay first — it's the more common default on Arch
            if is_on_path("yay") { return Ok(Some(Helper::Yay)); }
            if is_on_path("paru") { return Ok(Some(Helper::Paru)); }
            Ok(None)
        }
        "yay" => {
            if is_on_path("yay") { Ok(Some(Helper::Yay)) }
            else { Err("nog.conf requests `helper = \"yay\"` but yay is not on PATH".to_string()) }
        }
        "paru" => {
            if is_on_path("paru") { Ok(Some(Helper::Paru)) }
            else { Err("nog.conf requests `helper = \"paru\"` but paru is not on PATH".to_string()) }
        }
        other => Err(format!(
            "invalid `[aur] helper` value '{}'. Expected one of: auto, yay, paru, none",
            other
        )),
    }
}

/// PATH lookup by attempting `<bin> --version`. Cheaper than parsing $PATH
/// ourselves and works identically across shells.
fn is_on_path(bin: &str) -> bool {
    Command::new(bin)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Query the helper for pending AUR-only updates. Output format matches
/// `checkupdates` exactly — `pkg oldver -> newver` — so we reuse `PendingUpdate`.
///
/// Exit 0 with stdout lines  = updates available
/// Exit 1 with empty stdout  = no AUR updates available (helpers use 1, not 2,
///                             here — differs from checkupdates but harmless
///                             since empty stdout is unambiguous)
/// Any other exit            = genuine failure, bubble the stderr up
pub fn pending_updates(helper: Helper) -> Result<Vec<PendingUpdate>, String> {
    let output = Command::new(helper.binary())
        .arg("-Qua")
        .output()
        .map_err(|e| format!("failed to launch {}: {}", helper.binary(), e))?;
    interpret_qua(
        helper.binary(),
        output.status.success(),
        output.status.code(),
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
    )
}

/// Read a helper's `-Qua` answer. Pure, so every case is a unit test.
///
/// Both helpers exit 1 with empty stdout when there is nothing to update, so
/// the exit code alone cannot tell "nothing" from "failed". v1.5.4 (#25): the
/// error output can. Checked on the development desktop, 30 Sep 2026:
///   * `yay -Qua` with nothing pending: exit 1, stdout empty, stderr empty.
///   * `paru -Qua` failing (`--ignore` probe): exit 1, stdout empty, stderr
///     `error: failed to run: pacman --query …`.
/// Until now both read as "0 AUR updates", and the error was thrown away.
/// Now empty stdout is "nothing to update" only when stderr says nothing
/// either — yay's `->` notices (e.g. `-> Missing AUR Packages`) and
/// `warning:` lines are information, not failure.
fn interpret_qua(
    binary: &str,
    success: bool,
    code: Option<i32>,
    stdout: &str,
    stderr: &str,
) -> Result<Vec<PendingUpdate>, String> {
    let complaint: Vec<&str> = stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| !l.starts_with("->") && !l.to_ascii_lowercase().starts_with("warning:"))
        .collect();

    if stdout.trim().is_empty() {
        if complaint.is_empty() {
            return Ok(Vec::new());
        }
        return Err(complaint.join(" / "));
    }

    if !success {
        return Err(if complaint.is_empty() {
            format!("{} -Qua exited with status {}", binary, code.unwrap_or(-1))
        } else {
            complaint.join(" / ")
        });
    }

    let mut updates = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let mut parts = line.split_whitespace();
        let name = match parts.next() {
            Some(s) => s.to_string(),
            None => continue,
        };
        let old_version = parts.next().unwrap_or("").to_string();
        let _arrow = parts.next();
        let new_version = parts.next().unwrap_or("").to_string();
        updates.push(PendingUpdate { name, old_version, new_version, source: Source::Aur });
    }

    Ok(updates)
}

/// Install one or more packages via the helper. The helper checks sync repos
/// first, then falls back to AUR. nog doesn't care which path serves the
/// package — the helper handles it. Runs as the invoking user; the helper
/// sudo-s to pacman internally for the pacman portion.
pub fn install(helper: Helper, packages: &[String]) -> ExitStatus {
    let pkgs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
    let mut args = vec!["-S"];
    args.extend_from_slice(&pkgs);
    Command::new(helper.binary())
        .args(&args)
        .status()
        .unwrap_or_else(|e| panic!("nog: failed to launch {}: {}", helper.binary(), e))
}

/// Upgrade exactly the AUR packages nog cleared this run (v1.3.0, issue #10).
///
/// Replaces the old `-Syu` handoff, which asked the helper to drive the entire
/// system upgrade — official repos included — and so had the helper rebuilding
/// a plan pacman had already been given. Official packages are now pacman's
/// step; this one is only ever handed AUR names.
///
/// Naming the packages explicitly, rather than relying on a helper flag to
/// filter a sysupgrade down to the AUR, is the same mechanism flatpak and snap
/// already use: **listing exactly what was cleared IS the hold**. Nothing
/// unnamed can move, so a failed or empty AUR query cannot release a held
/// package by omission — the hole the v1.0.9 foreign fence was built to cover
/// is closed structurally here. It also keeps nog on bedrock pacman syntax
/// that yay and paru implement identically, instead of a flag whose
/// sysupgrade-filtering semantics differ between them.
///
/// `excluded` is still passed as `--ignore`: an AUR build can pull a repo
/// package in as a dependency, and a held package must stay held even then.
pub fn upgrade_cleared(helper: Helper, packages: &[String], excluded: &[String]) -> crate::handoff::Handoff {
    let mut args: Vec<String> = vec!["-S".to_string()];
    args.extend(packages.iter().cloned());
    if !excluded.is_empty() {
        args.push("--ignore".to_string());
        args.push(excluded.join(","));
    }
    let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    crate::handoff::run(Command::new(helper.binary()).args(&str_args), helper.binary())
}

/// Which AUR packages this run cleared: Ready ones, plus Unknowns the user
/// answered yes to. Mirrors `flatpak::apply_list` and `snap::apply_list` — one
/// idiom across every source nog drives.
pub fn apply_list(
    aur_names: &[String],
    ready: &[String],
    unknown: &[String],
    ignore: &[String],
) -> Vec<String> {
    let is_aur = |n: &String| aur_names.iter().any(|a| a == n);
    let mut out: Vec<String> = ready.iter().filter(|n| is_aur(n)).cloned().collect();
    out.extend(
        unknown.iter()
            .filter(|n| is_aur(n) && !ignore.iter().any(|i| i == *n))
            .cloned(),
    );
    out
}

/// Resolve AUR build dates for the given packages by delegating to the helper's
/// `-Sai` info command. The helper already caches AUR metadata for the user;
/// we reuse its cache instead of calling the AUR RPC ourselves, which keeps
/// nog's threat model unchanged — it remains purely a subprocess orchestrator.
///
/// Parses the helper's human-readable "Last Modified" line per package and
/// converts it to a Unix timestamp via `date -d "<str>" +%s`. Packages with
/// unparseable dates, missing entries, or helper failures are simply omitted —
/// callers treat them as Unknown, matching the current fallback behavior.
///
/// Single batched call for efficiency; AUR upgrade lists are typically < 10.
pub fn build_dates_for(helper: Helper, packages: &[String]) -> HashMap<String, u64> {
    let mut out = HashMap::new();
    if packages.is_empty() {
        return out;
    }

    // `-Sai` forces AUR lookup; packages also found in a sync DB will error
    // for that entry, which is fine — the caller already has the sync-DB date.
    let mut args: Vec<String> = vec!["-Sai".to_string()];
    args.extend(packages.iter().cloned());
    let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let output = match Command::new(helper.binary()).args(&str_args).output() {
        Ok(o) => o,
        Err(_) => return out, // helper unavailable mid-run: soft-fail to Unknown
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse key/value blocks. The output is a stream of `Key ... : value` lines
    // separated by blank lines between packages. We only care about `Name` (to
    // track which package a subsequent field belongs to) and `Last Modified`.
    // split_once(':') grabs only the first colon, so values containing colons
    // (URLs, timestamps) are preserved intact.
    let mut current_name: Option<String> = None;
    for line in stdout.lines() {
        if line.trim().is_empty() {
            current_name = None;
            continue;
        }
        let (key, val) = match line.split_once(':') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => continue,
        };
        match key {
            "Name" => current_name = Some(val.to_string()),
            "Last Modified" => {
                if let (Some(name), Some(ts)) = (current_name.as_ref(), parse_date_to_unix(val)) {
                    out.insert(name.clone(), ts);
                }
            }
            _ => {}
        }
    }

    out
}

/// Convert a human-readable date string (as printed by yay/paru's `-Si`) into
/// a Unix timestamp by shelling out to `date -d`. Matches how `_debug-dates`
/// already handles epoch display — no new Rust dep needed.
fn parse_date_to_unix(s: &str) -> Option<u64> {
    let out = Command::new("date")
        .arg("-d").arg(s)
        .arg("+%s")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- v1.5.4: a failing helper is not "nothing to update" (#25) ----------

    #[test]
    fn an_empty_quiet_answer_is_nothing_to_update() {
        // yay, 30 Sep 2026: exit 1, empty stdout, empty stderr.
        assert!(interpret_qua("yay", false, Some(1), "", "").unwrap().is_empty());
    }

    #[test]
    fn an_empty_answer_with_an_error_is_a_failure() {
        // paru, 30 Sep 2026 — this read as "0 AUR updates" before v1.5.4.
        let e = interpret_qua("paru", false, Some(1), "",
            "error: failed to run: pacman --query --ignore=fresh-editor-bin -q -- a b\n").unwrap_err();
        assert!(e.starts_with("error: failed to run: pacman --query"), "{e}");
    }

    #[test]
    fn yay_notices_are_not_failures() {
        let got = interpret_qua("yay", false, Some(1), "", " -> Missing AUR Packages:  cpuminer\nwarning: something mild\n");
        assert!(got.unwrap().is_empty());
    }

    #[test]
    fn a_real_answer_is_parsed_with_or_without_yays_age_suffix() {
        let got = interpret_qua("yay", true, Some(0), "fresh-editor-bin 0.5.1-1 -> 0.5.2-1 [2d9h]\n", "").unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].name.as_str(), got[0].old_version.as_str(), got[0].new_version.as_str()),
                   ("fresh-editor-bin", "0.5.1-1", "0.5.2-1"));
        let got = interpret_qua("paru", true, Some(0), "fresh-editor-bin 0.5.1-1 -> 0.5.2-1\n", "").unwrap();
        assert_eq!(got[0].new_version, "0.5.2-1");
    }

    fn owned(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn apply_list_names_only_cleared_aur_packages() {
        let aur = owned(&["fresh-editor-bin", "snapd", "sparrow-wallet"]);
        let ready = owned(&["fresh-editor-bin", "vlc"]); // vlc is an official package
        let unknown = owned(&["snapd", "sparrow-wallet"]);
        // sparrow-wallet was skipped by the user at the Unknown prompt:
        let ignore = owned(&["sparrow-wallet", "glibc"]);

        let got = apply_list(&aur, &ready, &unknown, &ignore);
        assert_eq!(got, owned(&["fresh-editor-bin", "snapd"]));
        // Official Ready packages belong to pacman's step and must never leak here:
        assert!(!got.contains(&"vlc".to_string()));
        // A skipped Unknown is simply never named:
        assert!(!got.contains(&"sparrow-wallet".to_string()));
    }

    #[test]
    fn held_aur_packages_can_never_be_applied() {
        // A held package appears in neither bucket — the caller's buckets are
        // disjoint — so there is nothing to name and nothing can be built.
        let aur = owned(&["fresh-editor-bin"]);
        assert!(apply_list(&aur, &[], &[], &[]).is_empty());
    }

    #[test]
    fn a_failed_aur_query_cannot_release_a_hold() {
        // Regression for the 2026-08-01 bypass, now closed structurally rather
        // than by the foreign fence. If the AUR query returns nothing, no AUR
        // package is classified, so none reaches Ready — and the helper is
        // handed an empty list instead of being invited to resolve its own
        // idea of what needs upgrading.
        let aur: Vec<String> = Vec::new();
        let ready = owned(&["fresh-editor-bin", "snapd"]);
        let unknown = owned(&["sparrow-wallet"]);
        assert!(apply_list(&aur, &ready, &unknown, &[]).is_empty());
    }

    #[test]
    fn ready_comes_before_approved_unknowns() {
        // Order is stable and meaningful: what cleared on its own, then what the
        // user waved through. The run log and the helper both see the same shape.
        let aur = owned(&["a-pkg", "z-pkg"]);
        let got = apply_list(&aur, &owned(&["z-pkg"]), &owned(&["a-pkg"]), &[]);
        assert_eq!(got, owned(&["z-pkg", "a-pkg"]));
    }

    /// Issue #12: run nog's own helper-facing code against every helper
    /// installed here and require the same answers. `#[ignore]`d because it
    /// needs yay and paru both on PATH and reads the live AUR — a diagnostic
    /// for the paru validation, not a regression test.
    ///
    /// `cargo test --release live_helpers_agree -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_helpers_agree() {
        let y = pending_updates(Helper::Yay).expect("yay -Qua failed");
        let p = pending_updates(Helper::Paru).expect("paru -Qua failed");
        let key = |v: &[PendingUpdate]| {
            let mut k: Vec<(String, String, String)> = v
                .iter()
                .map(|u| (u.name.clone(), u.old_version.clone(), u.new_version.clone()))
                .collect();
            k.sort();
            k
        };
        println!("yay  -Qua: {:?}", key(&y));
        println!("paru -Qua: {:?}", key(&p));
        assert_eq!(key(&y), key(&p), "the helpers report different pending AUR updates");

        let mut names: Vec<String> = y.iter().map(|u| u.name.clone()).collect();
        names.push("paru".into());
        names.push("yay".into());
        let dy = build_dates_for(Helper::Yay, &names);
        let dp = build_dates_for(Helper::Paru, &names);
        println!("yay  dates: {:?}", dy);
        println!("paru dates: {:?}", dp);
        assert_eq!(dy.len(), names.len(), "yay left a package undated");
        assert_eq!(dp.len(), names.len(), "paru left a package undated — it would classify as Unknown");
        assert_eq!(dy, dp, "the helpers date the same packages differently");
    }
}
