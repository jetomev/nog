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
use std::process::Command;

use crate::pacman::{PendingUpdate, Source};

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
            format!("flatpak remote-ls exited with status {}", crate::machine::status_text(&output.status))
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
pub fn update(app_ids: &[String]) -> crate::handoff::Handoff {
    let mut args: Vec<&str> = vec!["update", "-y"];
    args.extend(app_ids.iter().map(|s| s.as_str()));
    crate::handoff::run(Command::new("flatpak").args(&args), "flatpak")
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
        source: Source::Flatpak,
    }
}

// ── v1.9.0 (C3): the install chain ───────────────────────────────────────────

/// One app a remote offers, from `flatpak search`.
#[derive(Debug, Clone, PartialEq)]
pub struct FlatpakApp {
    pub app_id: String,
    pub version: String,
    /// The human name ("GIMP"), as the remote's metadata gives it.
    pub name: String,
    pub remote: String,
}

/// Ask every configured remote what it has for `query`. `flatpak search` is a
/// fuzzy search over names and descriptions, so the caller picks the exact
/// match with `find`. "No matches found" is a plain line on stdout with exit
/// 0; a non-zero exit is a real failure (no remotes, no network), bubbled up
/// so the chain says "could not check" instead of "not found".
pub fn search(query: &str) -> Result<Vec<FlatpakApp>, String> {
    let output = Command::new("flatpak")
        .args(["search", "--columns=application,version,name,remotes", "--", query])
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("failed to launch flatpak: {}", e))?;
    if !output.status.success() {
        let msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if msg.is_empty() {
            format!("flatpak search exited with status {}", crate::machine::status_text(&output.status))
        } else {
            msg
        });
    }
    Ok(parse_search(&String::from_utf8_lossy(&output.stdout)))
}

/// Parse `--columns=application,version,name,remotes` (tab-separated). Pure.
pub fn parse_search(stdout: &str) -> Vec<FlatpakApp> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with("No matches found") {
            continue;
        }
        let mut parts = line.split('\t');
        let app_id = match parts.next() {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => continue,
        };
        let version = parts.next().unwrap_or("").trim().to_string();
        let name = parts.next().unwrap_or("").trim().to_string();
        // Several remotes are listed comma-separated; the first configured wins.
        let remote = parts.next().unwrap_or("flathub").split(',').next().unwrap_or("flathub").trim().to_string();
        out.push(FlatpakApp { app_id, version, name, remote });
    }
    out
}

/// What the search found for the name typed.
#[derive(Debug, PartialEq)]
pub enum Found {
    One(FlatpakApp),
    /// More than one app answers to the name; the person names the app ID.
    Several(Vec<FlatpakApp>),
    None,
}

/// Pure. The exact match for a typed name, in this order: the full app ID
/// (`org.gimp.GIMP`), the ID's last segment (`gimp` → `org.gimp.GIMP`), the
/// human name (`spotify` → `com.spotify.Client`, named "Spotify"). Case does
/// not matter. A fuzzy hit (`gimp` also lists `org.gimp.GIMP.Manual`) is never
/// taken: an install goes only to a package that answers to the name.
pub fn find(typed: &str, apps: &[FlatpakApp]) -> Found {
    let t = typed.to_lowercase();
    let pick = |hits: Vec<&FlatpakApp>| -> Option<Found> {
        let mut ids: Vec<&FlatpakApp> = Vec::new();
        for h in hits {
            if !ids.iter().any(|i| i.app_id == h.app_id) { ids.push(h); }
        }
        match ids.len() {
            0 => None,
            1 => Some(Found::One(ids[0].clone())),
            _ => Some(Found::Several(ids.into_iter().cloned().collect())),
        }
    };
    let by_id = apps.iter().filter(|a| a.app_id.to_lowercase() == t).collect();
    if let Some(f) = pick(by_id) { return f; }
    let by_last = apps.iter()
        .filter(|a| a.app_id.rsplit('.').next().map(|s| s.to_lowercase()) == Some(t.clone())).collect();
    if let Some(f) = pick(by_last) { return f; }
    let by_name = apps.iter().filter(|a| a.name.to_lowercase() == t).collect();
    if let Some(f) = pick(by_name) { return f; }
    Found::None
}

/// Installed apps as (app ID, human name), for `nog remove` to recognise a
/// flatpak by either.
pub fn installed_apps() -> Vec<(String, String)> {
    let output = match Command::new("flatpak")
        .args(["list", "--app", "--columns=application,name"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    String::from_utf8_lossy(&output.stdout).lines().filter_map(|l| {
        let mut parts = l.split('\t');
        let id = parts.next()?.trim();
        if id.is_empty() { return None; }
        Some((id.to_string(), parts.next().unwrap_or("").trim().to_string()))
    }).collect()
}

/// Pure. The installed app a typed name refers to: the ID, its last segment,
/// or the human name, case ignored.
pub fn installed_match(typed: &str, apps: &[(String, String)]) -> Option<String> {
    let t = typed.to_lowercase();
    apps.iter().find(|(id, _)| id.to_lowercase() == t)
        .or_else(|| apps.iter().find(|(id, _)| id.rsplit('.').next().map(|s| s.to_lowercase()) == Some(t.clone())))
        .or_else(|| apps.iter().find(|(_, name)| name.to_lowercase() == t))
        .map(|(id, _)| id.clone())
}

/// v2.0.0 (C4): an installed Flatpak app, for `nog list` and `nog info`.
#[derive(Debug, Clone, PartialEq)]
pub struct InstalledApp {
    pub app_id: String,
    pub name: String,
    pub version: String,
    pub size: u64,
    pub origin: String,
    pub runtime: String,
}

/// Installed apps with their size, remote and runtime.
pub fn installed_detail() -> Vec<InstalledApp> {
    let output = match Command::new("flatpak")
        .args(["list", "--app", "--columns=application,name,version,size,origin,runtime"])
        .env("LC_ALL", "C")
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    parse_installed_detail(&String::from_utf8_lossy(&output.stdout))
}

/// Pure. `--columns=application,name,version,size,origin,runtime`, tab-separated.
pub fn parse_installed_detail(stdout: &str) -> Vec<InstalledApp> {
    stdout.lines().filter_map(|l| {
        let f: Vec<&str> = l.split('\t').map(|x| x.trim()).collect();
        if f.is_empty() || f[0].is_empty() { return None; }
        let get = |i: usize| f.get(i).copied().unwrap_or("").to_string();
        Some(InstalledApp {
            app_id: get(0), name: get(1), version: get(2),
            size: parse_size(&get(3)), origin: get(4), runtime: get(5),
        })
    }).collect()
}

/// Pure. flatpak's sizes as it prints them ("12.4 MB", "1.1 GB", "649.7 kB")
/// to bytes. flatpak uses powers of 1000.
pub fn parse_size(text: &str) -> u64 {
    let t = text.trim();
    let (num, unit) = match t.find(|c: char| c.is_ascii_alphabetic()) {
        Some(i) => (&t[..i], t[i..].trim()),
        None => (t, "B"),
    };
    let n: f64 = num.trim().parse().unwrap_or(0.0);
    let mult: f64 = match unit.to_lowercase().as_str() {
        "kb" => 1e3, "mb" => 1e6, "gb" => 1e9, "tb" => 1e12, _ => 1.0,
    };
    (n * mult) as u64
}

/// An installed runtime or extension: its ref (`org.gnome.Platform/x86_64/51`)
/// and size.
#[derive(Debug, Clone, PartialEq)]
pub struct Runtime {
    pub reference: String,
    pub size: u64,
}

pub fn installed_runtimes() -> Vec<Runtime> {
    let output = match Command::new("flatpak")
        .args(["list", "--runtime", "--columns=ref,size"])
        .env("LC_ALL", "C")
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    parse_runtimes(&String::from_utf8_lossy(&output.stdout))
}

/// Pure. `--columns=ref,size`, tab-separated.
pub fn parse_runtimes(stdout: &str) -> Vec<Runtime> {
    stdout.lines().filter_map(|l| {
        let mut f = l.split('\t');
        let reference = f.next()?.trim().to_string();
        if reference.is_empty() { return None; }
        Some(Runtime { reference, size: parse_size(f.next().unwrap_or("")) })
    }).collect()
}

/// v2.0.0 (C5): nog's estimate of the runtimes no installed app uses. Pure.
/// A runtime (`org.gnome.Platform/x86_64/51`) is used when an app names it;
/// an extension of a runtime (`org.gnome.Platform.Locale/x86_64/51`,
/// `org.freedesktop.Platform.GL.default/x86_64/26.08-extra`) is used when a
/// used runtime shares its family and branch (the `-extra` suffix dropped).
/// Anything else is unused. flatpak itself decides the exact list on
/// `flatpak uninstall --unused` — this is the report before it asks.
pub fn unused_runtimes(apps: &[InstalledApp], runtimes: &[Runtime], host_driver: Option<&str>) -> Vec<Runtime> {
    let used: Vec<(String, String)> = apps.iter().filter_map(|a| split_ref(&a.runtime)).collect();
    runtimes.iter().filter(|r| {
        let Some((id, branch)) = split_ref(&r.reference) else { return false };
        // flatpak keeps the GL extension matching the host's NVIDIA driver and
        // the GTK themes whatever the apps say; so does this estimate.
        if id.contains(".GL.nvidia-") {
            return host_driver.map_or(false, |d| !id.ends_with(&format!(".GL.nvidia-{}", d)));
        }
        if id.contains(".Gtk3theme.") { return false; }
        let branch = branch.trim_end_matches("-extra").to_string();
        !used.iter().any(|(uid, ubranch)| {
            (*uid == id || id.starts_with(&format!("{}.", uid))) && *ubranch == branch
        })
    }).cloned().collect()
}

/// The host's NVIDIA driver as flatpak names it (`615-71-09` for
/// nvidia-utils 615.71.09-1), or None without the driver.
pub fn host_nvidia_driver() -> Option<String> {
    let out = Command::new("pacman").args(["-Q", "nvidia-utils"]).output().ok()?;
    if !out.status.success() { return None; }
    let text = String::from_utf8_lossy(&out.stdout);
    let version = text.split_whitespace().nth(1)?;
    let pkgver = version.split_once('-').map(|(v, _)| v).unwrap_or(version);
    Some(pkgver.replace('.', "-"))
}

/// Pure. `org.gnome.Platform/x86_64/51` → (`org.gnome.Platform`, `51`).
fn split_ref(reference: &str) -> Option<(String, String)> {
    let mut parts = reference.split('/');
    let id = parts.next()?.to_string();
    let _arch = parts.next()?;
    let branch = parts.next()?.to_string();
    Some((id, branch))
}

/// `flatpak uninstall --unused`: flatpak shows its own list and asks; with
/// `yes` it is answered for the person.
pub fn uninstall_unused(yes: bool) -> crate::handoff::Handoff {
    let mut cmd = Command::new("flatpak");
    cmd.args(["uninstall", "--unused"]);
    if yes { cmd.arg("-y"); }
    crate::handoff::run_on_screen(&mut cmd, "flatpak uninstall --unused")
}

/// `flatpak install --reinstall` of an installed app (C4, `nog reinstall`).
pub fn reinstall(app_ids: &[String], yes: bool) -> crate::handoff::Handoff {
    let mut cmd = Command::new("flatpak");
    cmd.args(["install", "--reinstall"]);
    if yes { cmd.arg("-y"); }
    cmd.args(app_ids);
    crate::handoff::run_on_screen(&mut cmd, "flatpak install --reinstall")
}

/// `flatpak info <id>` as key: value pairs (installed apps only). The
/// description line (first line of `flatpak info`) comes as `_summary`.
pub fn info(app_id: &str) -> Option<Vec<(String, String)>> {
    let output = Command::new("flatpak").args(["info", "--", app_id]).env("LC_ALL", "C").output().ok()?;
    if !output.status.success() { return None; }
    Some(parse_info(&String::from_utf8_lossy(&output.stdout)))
}

/// Pure. `flatpak info` prints a summary line, a blank line, then `Key: value`.
pub fn parse_info(stdout: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut lines = stdout.lines().skip_while(|l| l.trim().is_empty());
    if let Some(first) = lines.next() {
        let t = first.trim();
        if !t.is_empty() && !t.contains(':') { out.push(("_summary".to_string(), t.to_string())); }
        else if let Some((k, v)) = t.split_once(':') { out.push((k.trim().to_string(), v.trim().to_string())); }
    }
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            let k = k.trim();
            if !k.is_empty() && !k.contains(' ') || k.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '-') {
                out.push((k.to_string(), v.trim().to_string()));
            }
        }
    }
    out
}

/// Install apps from one remote. flatpak shows its own transaction (the app,
/// the runtimes it needs, the sizes) and asks before anything changes; with
/// `yes` it is answered for the person (`nog install --yes`). Its output
/// stays on the screen, like pacman's.
pub fn install(remote: &str, app_ids: &[String], yes: bool) -> crate::handoff::Handoff {
    let mut cmd = Command::new("flatpak");
    cmd.arg("install");
    if yes { cmd.arg("-y"); }
    cmd.arg(remote).args(app_ids);
    crate::handoff::run_on_screen(&mut cmd, "flatpak")
}

/// Remove an app. flatpak lists what goes and asks; `yes` answers it.
pub fn uninstall(app_ids: &[String], yes: bool) -> crate::handoff::Handoff {
    let mut cmd = Command::new("flatpak");
    cmd.arg("uninstall");
    if yes { cmd.arg("-y"); }
    cmd.args(app_ids);
    crate::handoff::run_on_screen(&mut cmd, "flatpak")
}

/// Does flatpak have anywhere to look? Arch's package ships Flathub, but a
/// hand-set-up flatpak may have no remote at all, and `search` then finds
/// nothing without saying why.
pub fn has_remotes() -> bool {
    Command::new("flatpak").args(["remotes", "--columns=name"]).output()
        .map(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).trim().is_empty())
        .unwrap_or(false)
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

    // ── v1.9.0 (C3): the install chain ─────────────────────────────────────

    fn app(id: &str, name: &str) -> FlatpakApp {
        FlatpakApp { app_id: id.into(), version: "1".into(), name: name.into(), remote: "flathub".into() }
    }

    #[test]
    fn search_output_is_parsed_and_no_matches_is_empty() {
        let out = "org.gimp.GIMP\t3.2.6\tGNU Image Manipulation Program\tflathub\n\
                   com.spotify.Client\t\tSpotify\tflathub,fedora\n";
        let got = parse_search(out);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].app_id, "org.gimp.GIMP");
        assert_eq!(got[0].version, "3.2.6");
        assert_eq!(got[1].name, "Spotify");
        assert_eq!(got[1].remote, "flathub");
        assert!(parse_search("No matches found\n").is_empty());
    }

    #[test]
    fn a_typed_name_finds_the_app_that_answers_to_it_and_nothing_fuzzier() {
        // `flatpak search gimp` lists the manual and plugins too; only GIMP answers to "gimp".
        let apps = vec![
            app("org.gimp.GIMP.Manual", "GIMP User Manual"),
            app("org.gimp.GIMP", "GNU Image Manipulation Program"),
            app("org.gimp.GIMP.Plugin.Lensfun", "GimpLensfun"),
        ];
        assert_eq!(find("gimp", &apps), Found::One(apps[1].clone()));
        assert_eq!(find("org.gimp.GIMP", &apps), Found::One(apps[1].clone()));
        assert_eq!(find("GIMP User Manual", &apps), Found::One(apps[0].clone()));
        assert_eq!(find("lensfun", &apps), Found::One(apps[2].clone()), "the ID's last segment answers");
        assert_eq!(find("plugin", &apps), Found::None, "a middle segment does not");
        assert_eq!(find("image", &apps), Found::None, "a word of the description does not");
    }

    #[test]
    fn the_human_name_counts_when_the_id_does_not() {
        let apps = vec![app("com.spotify.Client", "Spotify"), app("com.termius.Termius", "Termius")];
        assert_eq!(find("spotify", &apps), Found::One(apps[0].clone()));
        assert_eq!(find("termius", &apps), Found::One(apps[1].clone()));
    }

    #[test]
    fn two_apps_answering_to_one_name_are_both_shown_not_guessed() {
        let apps = vec![app("org.one.Editor", "Editor"), app("org.two.Editor", "Editor")];
        match find("editor", &apps) {
            Found::Several(v) => assert_eq!(v.len(), 2),
            other => panic!("expected Several, got {:?}", other),
        }
        // The same app listed by two remotes is one app.
        let twice = vec![app("org.gimp.GIMP", "GIMP"), app("org.gimp.GIMP", "GIMP")];
        assert_eq!(find("gimp", &twice), Found::One(twice[0].clone()));
    }

    #[test]
    fn an_installed_flatpak_is_recognised_by_id_segment_or_name() {
        let apps = vec![("com.github.tchx84.Flatseal".to_string(), "Flatseal".to_string())];
        assert_eq!(installed_match("flatseal", &apps).as_deref(), Some("com.github.tchx84.Flatseal"));
        assert_eq!(installed_match("com.github.tchx84.Flatseal", &apps).as_deref(), Some("com.github.tchx84.Flatseal"));
        assert_eq!(installed_match("gimp", &apps), None);
    }

    #[test]
    fn installed_detail_and_runtimes_are_read_with_sizes() {
        let apps = parse_installed_detail("com.termius.Termius\tTermius\t9.38.2\t12.4 MB\tflathub\torg.freedesktop.Platform/x86_64/25.08\n");
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].size, 12_400_000);
        assert_eq!(apps[0].runtime, "org.freedesktop.Platform/x86_64/25.08");
        assert_eq!(parse_size("1.1 GB"), 1_100_000_000);
        assert_eq!(parse_size("649.7 kB"), 649_700);
        assert_eq!(parse_size(""), 0);
        let r = parse_runtimes("org.gnome.Platform/x86_64/51\t1.1 GB\n\n");
        assert_eq!(r, vec![Runtime { reference: "org.gnome.Platform/x86_64/51".into(), size: 1_100_000_000 }]);
    }

    #[test]
    fn unused_runtimes_are_those_no_app_names_including_their_extensions() {
        // This desktop on 2026-10-14: Flatseal on GNOME 50, Termius on freedesktop 25.08.
        let apps = parse_installed_detail(
            "com.github.tchx84.Flatseal\tFlatseal\t2.4.1\t1.4 MB\tflathub\torg.gnome.Platform/x86_64/50\n\
             com.termius.Termius\tTermius\t9.38.2\t12.4 MB\tflathub\torg.freedesktop.Platform/x86_64/25.08\n");
        let rt = parse_runtimes(
            "org.freedesktop.Platform/x86_64/25.08\t659.3 MB\n\
             org.freedesktop.Platform/x86_64/26.08\t721.9 MB\n\
             org.freedesktop.Platform.GL.default/x86_64/25.08-extra\t457.1 MB\n\
             org.freedesktop.Platform.GL.default/x86_64/26.08\t469.9 MB\n\
             org.gnome.Platform/x86_64/50\t1.1 GB\n\
             org.gnome.Platform/x86_64/51\t1.1 GB\n\
             org.gnome.Platform.Locale/x86_64/51\t18.7 kB\n\
             org.freedesktop.Platform.GL.nvidia-610-57-04/x86_64/1.4\t896.4 MB\n\
             org.freedesktop.Platform.GL.nvidia-615-71-09/x86_64/1.4\t957.3 MB\n\
             org.gtk.Gtk3theme.Breeze/x86_64/3.22\t649.7 kB\n");
        let got = unused_runtimes(&apps, &rt, Some("615-71-09"));
        let unused: Vec<&str> = got.iter().map(|r| r.reference.as_str()).collect();
        assert_eq!(unused, vec![
            "org.freedesktop.Platform/x86_64/26.08",
            "org.freedesktop.Platform.GL.default/x86_64/26.08",
            "org.gnome.Platform/x86_64/51",
            "org.gnome.Platform.Locale/x86_64/51",
            "org.freedesktop.Platform.GL.nvidia-610-57-04/x86_64/1.4",
        ], "the old driver's GL extension goes, the current one and the theme stay");
        assert!(unused_runtimes(&apps, &parse_runtimes("org.freedesktop.Platform.GL.nvidia-610-57-04/x86_64/1.4\t896 MB\n"), None).is_empty(),
            "without a known host driver no NVIDIA extension is called unused");
    }

    #[test]
    fn flatpak_info_is_read_as_pairs() {
        let got = parse_info("\nFlatseal - Manage Flatpak permissions\n\n          ID: com.github.tchx84.Flatseal\n         Ref: app/com.github.tchx84.Flatseal/x86_64/stable\n     Version: 2.4.1\n");
        assert_eq!(got[0], ("_summary".to_string(), "Flatseal - Manage Flatpak permissions".to_string()));
        assert!(got.contains(&("Version".to_string(), "2.4.1".to_string())));
    }
}
