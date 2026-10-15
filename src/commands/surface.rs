//! v2.0.0 (C4, #7): the command surface — every query command with `--json`,
//! every change command through the tool that owns the source. nogForge reads
//! the JSON; the tables here come from the same data and never change
//! behaviour. The words follow the house rules: a blank line around every
//! notice, the heading says what happened, small actions show only what
//! changes, nothing is installed or removed unasked.

use super::*;
use crate::{flatpak, locks, snap, sources};
use serde_json::{json, Value};
use std::path::Path;

const LOCAL_DB: &str = "/var/lib/pacman/local";

fn flatpak_on() -> bool { sources::load(sources::DEFAULT_PATH).flatpak && flatpak::is_available() }
fn snap_on() -> bool { sources::load(sources::DEFAULT_PATH).snap && snap::is_available() }

fn tier_of(tm: &TierManager, name: &str) -> u8 { tier_num(&tm.classify(name)) }

// ── search ──────────────────────────────────────────────────────────────────

/// `nog search <query> [--json]`: the repositories (pacman's order), the AUR
/// through the helper, then Flatpak and Snap when they are on and present. A
/// source that cannot be checked is said, never shown as "nothing".
pub fn search(query: &str, json: bool) {
    let cfg = load_config();
    let tm = load_tiers();
    let state = sources::load(sources::DEFAULT_PATH);
    let repo = String::from_utf8_lossy(&pacman::search_capture(query).stdout).to_string();
    let aur = resolve_helper(&cfg).filter(|_| state.aur).and_then(|h| {
        std::process::Command::new(h.binary()).args(["-Ssa", query]).output().ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
    });
    let mut v = crate::machine::search_json(&tm, query, &repo, aur.as_deref());
    let mut problems: Vec<String> = Vec::new();
    let mut fp_rows: Vec<Value> = Vec::new();
    if flatpak_on() {
        match flatpak::search(query) {
            Ok(apps) => {
                let installed = flatpak::installed_versions();
                let mut seen = std::collections::HashSet::new();
                for a in apps {
                    if !seen.insert(a.app_id.clone()) { continue; }
                    fp_rows.push(json!({
                        "name": a.app_id, "version": a.version, "description": a.name,
                        "source": "flatpak", "remote": a.remote, "installed": installed.contains_key(&a.app_id),
                        "tier": tier_of(&tm, &a.app_id), "install_as": format!("flatpak/{}", a.app_id),
                    }));
                }
            }
            Err(e) => problems.push(format!("Flatpak could not be searched: {}", e)),
        }
    }
    let mut snap_rows: Vec<Value> = Vec::new();
    if snap_on() {
        match snap::find(query) {
            Ok(found) => for f in found {
                snap_rows.push(json!({
                    "name": f.name, "version": f.version, "description": f.summary,
                    "source": "snap", "publisher": f.publisher, "classic": f.classic,
                    "installed": snap::is_installed(&f.name),
                    "tier": tier_of(&tm, &f.name), "install_as": format!("snap/{}", f.name),
                }));
            },
            Err(e) => problems.push(format!("Snap could not be searched: {}", e)),
        }
    }
    if let Some(results) = v["results"].as_array_mut() {
        results.extend(fp_rows.iter().cloned());
        results.extend(snap_rows.iter().cloned());
    }
    v["sources"] = json!({
        "flatpak": if flatpak_on() { "checked" } else { "off" },
        "snap": if snap_on() { "checked" } else { "off" },
        "problems": problems,
    });
    if json {
        crate::machine::emit(&v);
        return;
    }
    let results = v["results"].as_array().cloned().unwrap_or_default();
    if results.is_empty() && problems.is_empty() {
        notice(C_PEACH, &format!("Nothing answers to '{}'", query), &[
            "Not in the repositories, not in the AUR, not in Flatpak, not in Snap (the ones that are on).".to_string()]);
        return;
    }
    let groups: [(&str, &str, &str); 4] = [("repositories", "The repositories", C_GREEN), ("aur", "The AUR", C_PEACH),
        ("flatpak", "Flatpak", C_BLUE), ("snap", "Snap", C_MAUVE)];
    for (key, title, colour) in groups {
        let rows: Vec<&Value> = results.iter().filter(|r| {
            let s = r["source"].as_str().unwrap_or("");
            match key { "aur" | "flatpak" | "snap" => s == key, _ => !matches!(s, "aur" | "flatpak" | "snap") }
        }).collect();
        if rows.is_empty() { continue; }
        println!();
        println!("{}{}==> {} · {}{}", C_BOLD, colour, title, rows.len(), C_RESET);
        const SHOWN: usize = 30;
        let more = rows.len().saturating_sub(SHOWN);
        let w = rows.iter().take(SHOWN).map(|r| r["name"].as_str().unwrap_or("").chars().count()).max().unwrap_or(8).min(44);
        for r in rows.iter().take(SHOWN) {
            let name = r["name"].as_str().unwrap_or("");
            let tier = r["tier"].as_u64().unwrap_or(3) as u8;
            let mark = if r["installed"].as_bool().unwrap_or(false) { " [installed]" } else { "" };
            let where_ = match key {
                "flatpak" => format!("{}, as flatpak/{}", r["remote"].as_str().unwrap_or("flathub"), name),
                "snap" => format!("{}{}", r["publisher"].as_str().unwrap_or(""), if r["classic"].as_bool().unwrap_or(false) { " · classic" } else { "" }),
                _ => r["source"].as_str().unwrap_or("").to_string(),
            };
            println!("    {:<w$}  {:<14} {}Tier {}{}  {}{}", name, r["version"].as_str().unwrap_or(""),
                tier_color_num(tier), tier, C_RESET, where_, mark, w = w);
            let d = r["description"].as_str().unwrap_or("");
            if !d.is_empty() { println!("        {}{}{}", C_SUBTEXT, d, C_RESET); }
        }
        if more > 0 {
            println!("    {}… and {} more; a longer word narrows it, --json has them all.{}", C_SUBTEXT, more, C_RESET);
        }
    }
    for p in &problems {
        println!();
        println!("{}{}{}", C_PEACH, p, C_RESET);
    }
}

// ── list ────────────────────────────────────────────────────────────────────

/// Installed Flatpak apps and snaps as rows of the `list` document (v2.0.0).
pub fn foreign_rows(tm: &TierManager) -> Vec<Value> {
    let mut rows = Vec::new();
    if flatpak_on() {
        for a in flatpak::installed_detail() {
            rows.push(json!({
                "name": a.app_id, "version": a.version, "description": a.name,
                "tier": tier_of(tm, &a.app_id), "source": "flatpak", "remote": a.origin, "runtime": a.runtime,
                "explicit": true, "size": a.size, "installed": 0, "groups": [], "required_by": [],
                "protected": null, "remove_as": a.app_id,
            }));
        }
    }
    if snap_on() {
        for s in snap::list_all().into_iter().filter(|s| !s.disabled()) {
            rows.push(json!({
                "name": s.name, "version": s.version, "description": if s.is_base() { "the system under the snaps (a base snap)" } else { "a snap" },
                "tier": tier_of(tm, &s.name), "source": "snap", "revision": s.revision, "tracking": s.tracking,
                "publisher": s.publisher, "base": s.is_base(),
                "explicit": !s.is_base(), "size": snap::revision_size(&s.name, &s.revision), "installed": 0,
                "groups": [], "required_by": [], "protected": if s.is_base() { json!("the system under the snaps") } else { json!(null) },
                "remove_as": s.name,
            }));
        }
    }
    rows
}

pub fn list(json: bool) {
    let tm = load_tiers();
    let repos: HashMap<String, String> = sync_db::load_packages().iter()
        .filter_map(|(n, d)| d.repo.clone().map(|r| (n.clone(), r)))
        .collect();
    let mut v = crate::machine::list_json(&tm, Path::new(LOCAL_DB), &repos);
    let extra = foreign_rows(&tm);
    if let Some(p) = v["packages"].as_array_mut() { p.extend(extra); }
    if json {
        crate::machine::emit(&v);
        return;
    }
    for p in v["packages"].as_array().into_iter().flatten().filter(|p| p["explicit"] == true) {
        println!("{:<28} {:<20} Tier {}  {}", p["name"].as_str().unwrap_or(""), p["version"].as_str().unwrap_or(""),
            p["tier"], p["source"].as_str().unwrap_or(""));
    }
}

// ── info ────────────────────────────────────────────────────────────────────

/// Pure. `Key : value` lines (pacman -Qi/-Si, yay -Si) to pairs; a wrapped
/// value's continuation lines are joined.
pub fn parse_fields(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for l in text.lines() {
        if l.starts_with(' ') || l.starts_with('\t') {
            if let Some(last) = out.last_mut() { last.1 = format!("{} {}", last.1, l.trim()).trim().to_string(); }
            continue;
        }
        if let Some((k, v)) = l.split_once(" : ").or_else(|| l.split_once(':')) {
            let k = k.trim();
            if k.is_empty() { continue; }
            out.push((k.to_string(), v.trim().to_string()));
        }
    }
    out
}

/// Pure. The first letter up, for a reason that opens a line.
pub fn capitalised(text: &str) -> String {
    let mut c = text.chars();
    match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() }
}

/// Pure. A long space-separated list (glibc's "Required By" is 900 names)
/// cut to its first `keep` words, with the count of the rest.
pub fn shorten_list(value: &str, keep: usize) -> String {
    let words: Vec<&str> = value.split_whitespace().collect();
    if words.len() <= keep { return value.to_string(); }
    format!("{} … and {} more", words[..keep].join("  "), words.len() - keep)
}

fn field<'a>(fields: &'a [(String, String)], key: &str) -> Option<&'a str> {
    fields.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str())
}

/// Pure. The hold a package's updates get, in words, from its tier and window.
pub fn hold_words(tier: u8, holds: &crate::config::HoldsConfig, locked: bool) -> String {
    if locked { return "locked by you: never updated until `nog unlock`".to_string(); }
    let days = match tier { 1 => holds.tier1_days, 2 => holds.tier2_days, _ => holds.tier3_days };
    format!("Tier {}: a new version waits {} days before nog offers it", tier, days)
}

/// `nog info <name> [--json]`: one package, from whichever source has it —
/// installed first (pacman's local database, an installed Flatpak, an installed
/// snap), then the repositories, the AUR, Flatpak, Snap.
pub fn info(name: &str, json: bool) {
    let cfg = load_config();
    let tm = load_tiers();
    let (tier, why) = tm.explain(name);
    let tier_n = tier_num(&tier);
    let locked = locks::load(locks::DEFAULT_PATH).map(|l| l.contains(&name.to_string())).unwrap_or(false);
    let mut doc = json!({"nog": env!("CARGO_PKG_VERSION"), "kind": "info", "name": name,
        "tier": tier_n, "tier_reason": why, "hold": hold_words(tier_n, &cfg.holds, locked), "locked": locked});
    let run = |cmd: &str, args: &[&str]| -> Option<String> {
        let o = std::process::Command::new(cmd).args(args).env("LC_ALL", "C").stdin(std::process::Stdio::null()).output().ok()?;
        o.status.success().then(|| String::from_utf8_lossy(&o.stdout).to_string())
    };
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut source = String::new();
    let mut installed = false;
    let fp_apps = if flatpak_on() { flatpak::installed_detail() } else { Vec::new() };
    let fp_installed_id = flatpak::installed_match(name, &fp_apps.iter().map(|a| (a.app_id.clone(), a.name.clone())).collect::<Vec<_>>());
    if let Some(out) = run("pacman", &["-Qi", name]) {
        fields = parse_fields(&out); installed = true;
        source = sync_db::load_packages().get(name).and_then(|d| d.repo.clone()).unwrap_or_else(|| "aur".into());
    } else if let Some(id) = fp_installed_id {
        installed = true; source = "flatpak".into();
        fields = flatpak::info(&id).unwrap_or_default();
        if let Some(a) = fp_apps.iter().find(|a| a.app_id == id) {
            for (k, v) in [("Name", &a.name), ("Version", &a.version), ("Origin", &a.origin), ("Runtime", &a.runtime)] {
                if !fields.iter().any(|(fk, _)| fk == k) { fields.push((k.into(), v.clone())); }
            }
        }
        doc["app_id"] = json!(id);
    } else if snap_on() && snap::is_installed(name) {
        installed = true; source = "snap".into();
        if let Ok(Some(s)) = snap::info(name) {
            fields = vec![("Name".into(), s.name.clone()), ("Version".into(), s.version.clone()),
                ("Publisher".into(), s.publisher.clone()), ("Confinement".into(), s.confinement.clone())];
        }
        if let Some(i) = snap::list_all().into_iter().find(|i| i.name == name && !i.disabled()) {
            fields.push(("Revision".into(), i.revision.clone()));
            fields.push(("Tracking".into(), i.tracking.clone()));
            if !fields.iter().any(|(k, _)| k == "Version") { fields.push(("Version".into(), i.version.clone())); }
        }
    } else if let Some(out) = run("pacman", &["-Si", name]) {
        fields = parse_fields(&out);
        source = field(&fields, "Repository").unwrap_or("").to_string();
    } else if let Some(out) = resolve_helper(&cfg).filter(|_| sources::load(sources::DEFAULT_PATH).aur)
            .and_then(|h| run(h.binary(), &["-Si", "--aur", name]).or_else(|| run(h.binary(), &["-Si", name]))) {
        fields = parse_fields(&out); source = "aur".into();
    } else if flatpak_on() {
        if let Ok(found) = flatpak::search(name) {
            if let flatpak::Found::One(a) = flatpak::find(name, &found) {
                source = "flatpak".into();
                fields = vec![("Name".into(), a.name.clone()), ("Version".into(), a.version.clone()), ("Remote".into(), a.remote.clone())];
                doc["app_id"] = json!(a.app_id);
            }
        }
    }
    if source.is_empty() && snap_on() {
        if let Ok(Some(s)) = snap::info(name) {
            source = "snap".into();
            fields = vec![("Name".into(), s.name.clone()), ("Version".into(), s.version.clone()),
                ("Publisher".into(), s.publisher.clone()), ("Confinement".into(), s.confinement.clone())];
        }
    }
    if source.is_empty() {
        if json {
            doc["found"] = json!(false);
            crate::machine::emit(&doc);
            return;
        }
        enotice(C_PEACH, &format!("Nothing is known about '{}'", name), &[
            "Not installed, not in the repositories, not in the AUR, not in Flatpak, not in Snap (the ones that are on).".to_string(),
            "`nog search <word>` looks for a part of a name.".to_string()]);
        end(1);
    }
    let built_days_ago = sync_db::load_packages().get(name).filter(|_| !matches!(source.as_str(), "flatpak" | "snap" | "aur")).map(|d| {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|x| x.as_secs()).unwrap_or(0);
        now.saturating_sub(d.builddate) / 86_400
    });
    doc["found"] = json!(true); doc["source"] = json!(source); doc["installed"] = json!(installed);
    doc["version"] = json!(field(&fields, "Version").unwrap_or(""));
    doc["description"] = json!(field(&fields, "Description").or_else(|| field(&fields, "_summary")).unwrap_or(""));
    doc["fields"] = json!(fields.iter().filter(|(k, _)| k != "_summary").map(|(k, v)| json!({"key": k, "value": v})).collect::<Vec<_>>());
    doc["built_days_ago"] = json!(built_days_ago);
    if json {
        crate::machine::emit(&doc);
        return;
    }
    let src_word = match source.as_str() {
        "aur" => "the AUR".to_string(), "flatpak" => "Flatpak".to_string(), "snap" => "Snap".to_string(),
        r => format!("the {} repository", r),
    };
    let heading = format!("{} · {}{}", name, doc["version"].as_str().unwrap_or(""), if installed { " · installed" } else { " · not installed" });
    let mut lines = vec![
        format!("From {}.", src_word),
        format!("{}Tier {} — {}.", tier_color_num(tier_n), tier_n, capitalised(&why)) + C_RESET,
        hold_words(tier_n, &cfg.holds, locked) + ".",
    ];
    if let Some(d) = built_days_ago { lines.push(format!("The repository's build is {} day{} old.", d, if d == 1 { "" } else { "s" })); }
    let d = doc["description"].as_str().unwrap_or("");
    if !d.is_empty() { lines.push(String::new()); lines.push(d.to_string()); }
    notice(C_GREEN, &heading, &lines);
    let shown = ["Installed Size", "Download Size", "Install Date", "Install Reason", "Required By", "Optional For", "Depends On",
        "Licenses", "URL", "Packager", "Build Date", "Remote", "Origin", "Runtime", "Publisher", "Confinement", "Ref", "Commit", "Revision", "Tracking"];
    let w = fields.iter().filter(|(k, _)| shown.contains(&k.as_str())).map(|(k, _)| k.len()).max().unwrap_or(0);
    for (k, v) in &fields {
        if shown.contains(&k.as_str()) && !v.is_empty() && v != "None" {
            println!("    {}{:<w$}{}  {}", C_SUBTEXT, k, C_RESET, shorten_list(v, 12), w = w);
        }
    }
    println!();
}

// ── reinstall ───────────────────────────────────────────────────────────────

/// `nog reinstall <name...> [--yes]`: each installed package through its own
/// tool — pacman, the helper (a rebuild), `flatpak install --reinstall`. A snap
/// cannot be reinstalled in place (snapd has no such step); a name that is not
/// installed stops before anything.
pub fn reinstall(packages: &[String], yes: bool) {
    let cfg = load_config();
    let helper = resolve_helper(&cfg);
    guard_not_sudo_with_helper(helper);
    let (pkgs, others) = crate::removal::split_installed(packages);
    let fp_installed = if flatpak_on() { flatpak::installed_apps() } else { Vec::new() };
    let mut flatpaks = Vec::new();
    let mut snaps = Vec::new();
    let mut nowhere = Vec::new();
    for o in &others {
        if let Some(id) = flatpak::installed_match(o, &fp_installed) { flatpaks.push(id); }
        else if snap_on() && snap::is_installed(o) { snaps.push(o.clone()); }
        else { nowhere.push(o.clone()); }
    }
    let label = packages.join(" ");
    if !nowhere.is_empty() {
        enotice(C_PEACH, &format!("Not reinstalling: {}", label), &[
            format!("{} {} not installed on this computer: not a package, not a Flatpak, not a snap.", nowhere.join(", "), if nowhere.len() == 1 { "is" } else { "are" }),
            "Reinstalling puts back the files of something that is already here; `nog install` is for something new.".to_string()]);
        end(1);
    }
    if !snaps.is_empty() {
        enotice(C_PEACH, &format!("Not reinstalling: {}", label), &[
            format!("{} {} a snap, and snapd has no reinstall step: a snap is removed and installed again, which drops its data after a while (snapd's rule).",
                snaps.join(", "), if snaps.len() == 1 { "is" } else { "are" }),
            format!("If that is what you want: nog remove {} && nog install snap/{}", snaps.join(" "), snaps.join(" snap/")),
            "Nothing was changed.".to_string()]);
        end(1);
    }
    if !yes && !at_the_keyboard() {
        enotice(C_PEACH, &format!("Not reinstalling: {}", label), &nobody_to_answer_lines("reinstall"));
        end(1);
    }
    let foreign = pacman::foreign_package_names();
    let (aur_pkgs, repo_pkgs): (Vec<String>, Vec<String>) = pkgs.iter().cloned().partition(|p| foreign.contains(p));
    let mut steps: Vec<(&str, String)> = Vec::new();
    if !repo_pkgs.is_empty() { steps.push(("pacman", format!("Reinstalling {}", repo_pkgs.join(" ")))); }
    if !aur_pkgs.is_empty() { steps.push(("aur", format!("Rebuilding {}", aur_pkgs.join(" ")))); }
    if !flatpaks.is_empty() { steps.push(("flatpak", format!("Reinstalling {} through Flatpak", flatpaks.join(" ")))); }
    crate::events::steps(&steps);
    if !repo_pkgs.is_empty() {
        notice(C_GREEN, &format!("Reinstalling {}", repo_pkgs.join(" ")), &[
            "The same version, its files put back.".to_string(), handoff_line(false, "", yes).trim_start_matches("nog: ").to_string()]);
        crate::events::start("pacman", "Reinstalling");
        let st = pacman::install(&repo_pkgs, yes);
        crate::events::finished("pacman", "Reinstalling", st.success(), "");
        if !st.success() { stopped_reinstalling(&label, st.code(), "pacman"); }
    }
    if !aur_pkgs.is_empty() {
        let Some(h) = helper else {
            enotice(C_PEACH, &format!("Not reinstalling: {}", aur_pkgs.join(" ")), &[
                "These came from the AUR and no AUR helper (yay or paru) is installed to rebuild them.".to_string()]);
            end(1);
        };
        if yes {
            enotice(C_PEACH, &format!("Not reinstalling: {}", aur_pkgs.join(" ")), &[
                "--yes does not answer the AUR: a rebuild shows its recipe for review, which needs a person.".to_string(),
                format!("Run this in a terminal: nog reinstall {}", aur_pkgs.join(" "))]);
            end(1);
        }
        notice(C_PEACH, &format!("Rebuilding {}", aur_pkgs.join(" ")), &[
            format!("Handing off to {}: it shows each build recipe to review, builds, then pacman asks once more before installing.", h)]);
        crate::events::start("aur", "Rebuilding");
        let st = aur::install(h, &aur_pkgs);
        crate::events::finished("aur", "Rebuilding", st.success(), "");
        if !st.success() { stopped_reinstalling(&label, st.code(), h.binary()); }
    }
    if !flatpaks.is_empty() {
        notice(C_BLUE, &format!("Reinstalling {}", flatpaks.join(" ")), &[flatpak_handoff_line(yes)]);
        crate::events::start("flatpak", "Reinstalling through Flatpak");
        let h = flatpak::reinstall(&flatpaks, yes);
        crate::events::finished("flatpak", "Reinstalling through Flatpak", h.status.success(), "");
        if !h.status.success() { stopped_reinstalling(&label, h.status.code(), "flatpak"); }
    }
}

fn stopped_reinstalling(label: &str, code: Option<i32>, tool: &str) -> ! {
    enotice(C_PEACH, &format!("Not reinstalled: {}", label), &[
        format!("{} stopped (status {}): you answered no, or it hit a problem shown above.", tool, code.unwrap_or(-1)),
        "Nothing more was changed.".to_string()]);
    end(code.unwrap_or(1));
}

// ── tier ────────────────────────────────────────────────────────────────────

/// `nog tier <name> [--set N] [--json]`: the tier and where it comes from;
/// `--set` pins (what `nog pin` does), and says what changed.
pub fn tier(package: &str, set: Option<u8>, json: bool) {
    let cfg = load_config();
    let tm = load_tiers();
    let (t, why) = tm.explain(package);
    let n = tier_num(&t);
    if let Some(new) = set {
        if !(1..=3).contains(&new) {
            enotice(C_PEACH, &format!("Not changing {}", package), &[format!("{} is not a tier: 1, 2 or 3.", new)]);
            end(1);
        }
        if new == n && why.starts_with("pinned") || (new == 3 && why.ends_with("the default")) {
            notice(C_GREEN, &format!("{} is already Tier {}", package, new), &[format!("{}. Nothing was changed.", capitalised(&why))]);
            return;
        }
        notice(C_GREEN, &format!("Moving {} to Tier {}", package, new), &[
            format!("Now Tier {}: {}.", n, why),
            format!("Tier {} means its updates wait {} days (tier-pins.toml; root is needed, so sudo asks once).", new,
                match new { 1 => cfg.holds.tier1_days, 2 => cfg.holds.tier2_days, _ => cfg.holds.tier3_days })]);
        if let Err(e) = crate::tiers::pin_package(&cfg.paths.tier_pins, package, new) {
            enotice(C_PEACH, &format!("Not changed: {}", package), &[e]);
            end(1);
        }
        return;
    }
    let days = match n { 1 => cfg.holds.tier1_days, 2 => cfg.holds.tier2_days, _ => cfg.holds.tier3_days };
    if json {
        crate::machine::emit(&json!({"nog": env!("CARGO_PKG_VERSION"), "kind": "tier", "name": package,
            "tier": n, "reason": why, "hold_days": days, "manual_signoff": tm.is_manual_signoff(package)}));
        return;
    }
    notice(tier_color_num(n), &format!("{} is Tier {}", package, n), &[
        format!("{}.", capitalised(&why)),
        format!("Its updates wait {} days before nog offers them{}.", days,
            if tm.is_manual_signoff(package) { ", and need your sign-off (`nog unlock --promote`)" } else { "" }),
        format!("To change it: nog tier {} --set 1|2|3", package)]);
}

// ── lock / unlock ───────────────────────────────────────────────────────────

fn load_locks_or_stop() -> Vec<String> {
    match locks::load(locks::DEFAULT_PATH) {
        Ok(l) => l,
        Err(e) => {
            enotice(C_PEACH, "The lock file could not be read", &[e, "Nothing was changed. Fix or remove the file, then try again.".to_string()]);
            end(1);
        }
    }
}

/// `nog lock [<name>...] [--json]`: with names, lock them (never updated until
/// unlocked); alone, list the locks.
pub fn lock(packages: &[String], json: bool) {
    let current = load_locks_or_stop();
    if packages.is_empty() {
        if json {
            crate::machine::emit(&json!({"nog": env!("CARGO_PKG_VERSION"), "kind": "locks", "locked": current}));
            return;
        }
        if current.is_empty() {
            notice(C_GREEN, "No packages are locked", &["A locked package is never updated until you unlock it: nog lock <package>".to_string()]);
        } else {
            let mut lines: Vec<String> = current.iter().map(|p| format!("  {}", p)).collect();
            lines.push("Each one stays at its installed version until `nog unlock <package>`.".to_string());
            notice(C_GREEN, &format!("Locked: {}", current.len()), &lines);
        }
        return;
    }
    let installed = pacman::installed_versions(packages);
    let unknown: Vec<&String> = packages.iter().filter(|p| !installed.contains_key(*p)).collect();
    if !unknown.is_empty() {
        enotice(C_PEACH, &format!("Not locking: {}", packages.join(" ")), &[
            format!("{} {} not an installed package. A lock keeps an installed version where it is; `nog list` shows what is installed.",
                unknown.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "), if unknown.len() == 1 { "is" } else { "are" }),
            "Nothing was changed.".to_string()]);
        end(1);
    }
    let (next, already) = locks::with_added(&current, packages);
    if next.len() == current.len() {
        notice(C_GREEN, &format!("Already locked: {}", packages.join(" ")), &["Nothing was changed.".to_string()]);
        return;
    }
    let mut lines: Vec<String> = packages.iter().filter(|p| !already.contains(p))
        .map(|p| format!("{} stays at {} until `nog unlock {}`.", p, installed.get(p).cloned().unwrap_or_default(), p)).collect();
    if !already.is_empty() { lines.push(format!("Already locked: {}.", already.join(", "))); }
    lines.push("`nog update` lists a locked package as held, \"locked by you\", and pacman is told to ignore it.".to_string());
    lines.push(format!("Written to {} (root is needed, so sudo asks once).", locks::DEFAULT_PATH));
    notice(C_GREEN, &format!("Locking {}", packages.iter().filter(|p| !already.contains(p)).cloned().collect::<Vec<_>>().join(" ")), &lines);
    if let Err(e) = locks::save(locks::DEFAULT_PATH, &next) {
        enotice(C_PEACH, "Not locked", &[e, "Nothing was changed.".to_string()]);
        end(1);
    }
}

/// `nog unlock <name>... [--promote]`: remove the lock; `--promote` also
/// upgrades the package now, past its hold window (what `unlock --promote` has
/// meant since v1.0.4).
pub fn unlock(packages: &[String], promote: bool) {
    let current = load_locks_or_stop();
    let (next, not_locked) = locks::with_removed(&current, packages);
    let removed: Vec<String> = packages.iter().filter(|p| !not_locked.contains(p)).cloned().collect();
    if !removed.is_empty() {
        let mut lines = vec![format!("{} will be offered again by `nog update`, by {} tier's rules.", removed.join(", "),
            if removed.len() == 1 { "its" } else { "each one's" })];
        if !not_locked.is_empty() { lines.push(format!("Not locked, nothing to undo: {}.", not_locked.join(", "))); }
        lines.push(format!("Written to {} (root is needed, so sudo asks once).", locks::DEFAULT_PATH));
        notice(C_GREEN, &format!("Unlocking {}", removed.join(" ")), &lines);
        if let Err(e) = locks::save(locks::DEFAULT_PATH, &next) {
            enotice(C_PEACH, "Not unlocked", &[e, "Nothing was changed.".to_string()]);
            end(1);
        }
    }
    if !promote {
        if removed.is_empty() {
            let tm = load_tiers();
            let mut lines = vec![format!("{} {} not locked, so there is nothing to undo.", packages.join(", "), if packages.len() == 1 { "is" } else { "are" })];
            for p in packages {
                let (t, why) = tm.explain(p);
                lines.push(format!("{} is {} ({}).", p, t, why));
            }
            lines.push(format!("To upgrade a held package now, past its window: nog unlock {} --promote", packages.join(" ")));
            notice(C_GREEN, &format!("Nothing to unlock: {}", packages.join(" ")), &lines);
        }
        return;
    }
    let cfg = load_config();
    let helper = resolve_helper(&cfg);
    guard_not_sudo_with_helper(helper);
    let tm = load_tiers();
    for p in packages {
        notice(C_PEACH, &format!("Promoting {}", p), &[format!("{} ({}): upgraded now, past its hold window.", p, tm.classify(p))]);
    }
    let status = match helper {
        Some(h) => aur::install(h, packages),
        None => pacman::install(packages, false),
    };
    if !status.success() {
        enotice(C_PEACH, &format!("Not promoted: {}", packages.join(" ")), &[
            format!("The upgrade stopped (status {}): you answered no, or it hit a problem shown above.", status.code().unwrap_or(-1))]);
        end(status.code().unwrap_or(1));
    }
}

// ── status ──────────────────────────────────────────────────────────────────

/// `nog status [--json]`: the dashboard — what is installed from where, the
/// sources, the update plan's buckets (held ones with their release dates),
/// the locks, the last run and the last reboot advice.
pub fn status(json: bool) {
    let cfg = load_config();
    if !json {
        println!("nog: Checking every source for updates (this takes a moment) ...");
    }
    crate::machine::set_capture();
    let _ = crate::machine::quietly(|| super::run_update(false));
    let plan = crate::machine::captured();
    let installed = crate::machine::load_installed(Path::new(LOCAL_DB));
    let explicit = installed.iter().filter(|p| p.explicit).count();
    let foreign = pacman::foreign_package_names().len();
    let fp = if flatpak_on() { Some(flatpak::installed_detail().len()) } else { None };
    let sn = if snap_on() { Some(snap::list_all().iter().filter(|s| !s.disabled() && !s.is_base()).count()) } else { None };
    let locked = locks::load(locks::DEFAULT_PATH).unwrap_or_default();
    let dir = runlog::expand_home(&cfg.paths.run_logs);
    let today = runlog::today_and_cutoff().map(|(t, _)| t).unwrap_or_else(|| "20000101".into());
    let (runs, _updates, reboots) = crate::history::load(&dir, &crate::history::day_minus(&today, 90));
    let last_run = runs.first().cloned();
    let last_update_run = runs.iter().find(|r| r.get("command").map(|c| c.starts_with("update")).unwrap_or(false)).cloned();
    let last_reboot = reboots.first().cloned();
    let switches = sources::load(sources::DEFAULT_PATH);
    let empty = Vec::new();
    let (ready, held, unknown) = match &plan {
        Some(p) => (p["ready"].as_array().unwrap_or(&empty).clone(), p["held"].as_array().unwrap_or(&empty).clone(), p["unknown"].as_array().unwrap_or(&empty).clone()),
        None => (Vec::new(), Vec::new(), Vec::new()),
    };
    let doc = json!({
        "nog": env!("CARGO_PKG_VERSION"), "kind": "status",
        "installed": {"packages": installed.len(), "chosen": explicit, "foreign": foreign, "flatpaks": fp, "snaps": sn},
        "sources": {"aur": switches.aur, "chaotic_aur": switches.chaotic_aur, "flatpak": switches.flatpak, "snap": switches.snap,
                    "flatpak_present": flatpak::is_available(), "snap_present": snap::is_available()},
        "plan": plan.clone().unwrap_or(json!(null)),
        "counts": {"ready": ready.len(), "held": held.len(), "unknown": unknown.len()},
        "locked": locked,
        "last_run": last_run, "last_update": last_update_run, "last_reboot_advice": last_reboot,
    });
    if json {
        crate::machine::emit(&doc);
        return;
    }
    let onoff = |on: bool, present: bool| match (on, present) { (true, true) => "on", (true, false) => "on, not installed", (false, _) => "off" };
    notice(C_GREEN, "This computer", &[
        format!("{} packages installed, {} of them chosen by you, {} from outside the repositories (the AUR or a file).", installed.len(), explicit, foreign),
        format!("Flatpak apps: {} · snaps: {}.", fp.map(|n| n.to_string()).unwrap_or_else(|| "off".into()), sn.map(|n| n.to_string()).unwrap_or_else(|| "off".into())),
        format!("Sources: AUR {} · chaotic-aur {} · Flatpak {} · Snap {}.", if switches.aur { "on" } else { "off" }, if switches.chaotic_aur { "on" } else { "off" },
            onoff(switches.flatpak, flatpak::is_available()), onoff(switches.snap, snap::is_available())),
    ]);
    match &plan {
        None => notice(C_PEACH, "Updates: could not check", &["`nog update` says why.".to_string()]),
        Some(p) => {
            let mut lines = vec![format!("{} ready now · {} held by their tier · {} nog would ask you about.", ready.len(), held.len(), unknown.len())];
            if !ready.is_empty() {
                lines.push(format!("Ready: {}.", ready.iter().take(8).map(|r| r["name"].as_str().unwrap_or("")).collect::<Vec<_>>().join(", ")
                    + if ready.len() > 8 { ", …" } else { "" }));
            }
            let mut soon: Vec<(u64, &str)> = held.iter().filter_map(|h| Some((h["days_remaining"].as_u64()?, h["name"].as_str()?))).filter(|(d, _)| *d > 0).collect();
            soon.sort();
            if let Some((d, _)) = soon.first() {
                let names: Vec<&str> = soon.iter().filter(|(dd, _)| dd == d).map(|(_, n)| *n).take(6).collect();
                lines.push(format!("Next to be released: {} in {} day{}.", names.join(", "), d, if *d == 1 { "" } else { "s" }));
            }
            let src = &p["sources"];
            let bad: Vec<String> = ["aur", "flatpak", "snap"].iter().filter(|k| src[*k] == "could not check").map(|k| k.to_string()).collect();
            if !bad.is_empty() { lines.push(format!("Could not check: {}.", bad.join(", "))); }
            lines.push("`nog update` shows the full plan and installs what is ready.".to_string());
            notice(if ready.is_empty() { C_GREEN } else { C_YELLOW }, if ready.is_empty() { "Updates: nothing waiting for you" } else { "Updates: some are ready" }, &lines);
        }
    }
    if !locked.is_empty() {
        notice(C_PEACH, &format!("Locked by you: {}", locked.len()), &[format!("{} — never updated until `nog unlock`.", locked.join(", "))]);
    }
    let g = |r: &Option<crate::history::Row>, k: &str| r.as_ref().and_then(|r| r.get(k).cloned()).unwrap_or_else(|| "-".into());
    let when = |r: &Option<crate::history::Row>| if r.is_some() { format!("{} {}", g(r, "date"), g(r, "time")) } else { "never (in the last 90 days)".to_string() };
    let mut lines = vec![format!("Last run: {} — {} ({}).", when(&last_run), g(&last_run, "command"), g(&last_run, "outcome"))];
    lines.push(format!("Last update run: {}.", when(&last_update_run)));
    if last_reboot.is_some() {
        lines.push(format!("Last reboot advice: {} — {}.", when(&last_reboot), g(&last_reboot, "advice").trim_end_matches('.')));
    }
    lines.push("`nog history` shows everything nog has done.".to_string());
    notice(C_SUBTEXT, "Recently", &lines);
}

// ── history ─────────────────────────────────────────────────────────────────

/// `nog history [--days N] [--json]`: every run, every package decision and
/// every reboot advice from nog's own logs, newest first.
pub fn history(days: u32, json: bool) {
    let cfg = load_config();
    let dir = runlog::expand_home(&cfg.paths.run_logs);
    let today = runlog::today_and_cutoff().map(|(t, _)| t).unwrap_or_else(|| "20000101".into());
    let since = crate::history::day_minus(&today, days);
    let (runs, updates, reboots) = crate::history::load(&dir, &since);
    if json {
        crate::machine::emit(&json!({"nog": env!("CARGO_PKG_VERSION"), "kind": "history", "days": days, "folder": dir,
            "runs": runs, "updates": updates, "reboots": reboots}));
        return;
    }
    if runs.is_empty() && updates.is_empty() && reboots.is_empty() {
        notice(C_GREEN, &format!("Nothing in the last {} days", days), &[format!("nog's logs live in {}; they are kept {} days.", dir, runlog::RETENTION_DAYS)]);
        return;
    }
    println!();
    println!("{}{}==> Runs · {} in the last {} days{}", C_BOLD, C_GREEN, runs.len(), days, C_RESET);
    for r in runs.iter().take(60) {
        let g = |k: &str| r.get(k).cloned().unwrap_or_default();
        let ok = g("outcome") == "done" || g("outcome") == "up to date";
        println!("    {} {}  {}{:<8}{}  {}", g("date"), g("time"), if ok { C_GREEN } else { C_PEACH }, g("outcome"), C_RESET, g("command"));
    }
    if runs.len() > 60 { println!("    {}… and {} more (--json has them all){}", C_SUBTEXT, runs.len() - 60, C_RESET); }
    let decided: Vec<&crate::history::Row> = updates.iter().filter(|u| !u.get("package").map(|p| p.is_empty()).unwrap_or(true)).collect();
    if !decided.is_empty() {
        println!();
        println!("{}{}==> Update decisions · {} package rows{}", C_BOLD, C_BLUE, decided.len(), C_RESET);
        for u in decided.iter().take(40) {
            let g = |k: &str| u.get(k).cloned().unwrap_or_default();
            println!("    {} {}  {:<7} {:<26} {} → {}  {}{}{}", g("date"), g("time"), g("bucket"), g("package"), g("old_version"), g("new_version"),
                C_SUBTEXT, g("outcome"), C_RESET);
        }
        if decided.len() > 40 { println!("    {}… and {} more (--json has them all){}", C_SUBTEXT, decided.len() - 40, C_RESET); }
    }
    if !reboots.is_empty() {
        println!();
        println!("{}{}==> Reboot advice · {}{}", C_BOLD, C_PEACH, reboots.len(), C_RESET);
        for r in reboots.iter().take(10) {
            let g = |k: &str| r.get(k).cloned().unwrap_or_default();
            println!("    {} {}  {}", g("date"), g("time"), g("advice"));
        }
    }
    println!();
    println!("{}Logs: {} (kept {} days).{}", C_SUBTEXT, dir, runlog::RETENTION_DAYS, C_RESET);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pacman_info_fields_are_read_wrapped_values_joined() {
        let text = "Name            : cowsay\nVersion         : 3.8.4-1\nDescription     : Configurable talking cow\nDepends On      : perl\n                  bash\nInstalled Size  : 44.86 KiB\n";
        let f = parse_fields(text);
        assert_eq!(field(&f, "Version"), Some("3.8.4-1"));
        assert_eq!(field(&f, "Depends On"), Some("perl bash"));
        assert_eq!(field(&f, "Nope"), None);
    }

    #[test]
    fn long_lists_are_cut_with_a_count() {
        assert_eq!(shorten_list("a b c", 12), "a b c");
        assert_eq!(shorten_list("a b c d", 2), "a  b … and 2 more");
    }

    #[test]
    fn the_hold_is_said_from_the_tier_or_the_lock() {
        let h = crate::config::HoldsConfig { tier1_days: 30, tier2_days: 15, tier3_days: 7, tier1_safety_days: 7, tier2_safety_days: 3, tier3_safety_days: 1 };
        assert_eq!(hold_words(1, &h, false), "Tier 1: a new version waits 30 days before nog offers it");
        assert_eq!(hold_words(3, &h, false), "Tier 3: a new version waits 7 days before nog offers it");
        assert!(hold_words(3, &h, true).starts_with("locked by you"));
    }
}
