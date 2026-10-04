//! v1.6.0 (#7): nog for programs — `--json` output, and the password window.
//!
//! nogForge (the Forge Suite's package app) shows what nog decides; it never
//! decides itself. So nog says it as data:
//!
//! * `nog list --json` — every installed package: version, description, tier,
//!   where it came from, whether you chose it, what needs it, and whether it is
//!   protected (Tier 1, the `base` set, or needed by another package);
//! * `nog search --json <q>` — the repositories (and the AUR through the helper);
//! * `nog update --json [--keep a,b]` — the plan, computed by the same code as a
//!   real update, stopped before any question or transaction.
//!
//! In JSON mode everything nog normally prints goes to stderr; stdout carries
//! exactly one JSON document. And with `NOG_ASKPASS=1` in the environment
//! (nogForge sets it), every `sudo` nog runs gets `-A`: the system's own
//! password window asks, never a terminal nogForge is drawing on.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicI32, Ordering};

use serde_json::{json, Value};

use crate::tiers::{Tier, TierManager};

static SAVED_STDOUT: AtomicI32 = AtomicI32::new(-1);
static PLAN: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// `nog update --json [--keep a,b]`: compute the plan, write it, stop there.
pub fn set_plan(keep: Vec<String>) {
    let _ = PLAN.set(keep);
}

pub fn plan_mode() -> bool {
    PLAN.get().is_some()
}

/// What you asked to keep back (`--keep`), also in a real `nog update`.
pub fn keep() -> Vec<String> {
    KEEP.get().cloned().unwrap_or_else(|| PLAN.get().cloned().unwrap_or_default())
}

static KEEP: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
static PROMOTE: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// v1.6.0: what you promoted (`--promote`): ready now, installed with the rest.
pub fn set_promote(p: Vec<String>) {
    let _ = PROMOTE.set(p);
}

pub fn promote() -> Vec<String> {
    PROMOTE.get().cloned().unwrap_or_default()
}

static ONLY: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// v1.6.1: `nog update a b c` — only these, on purpose (Javier, 4 Oct 2026).
pub fn set_only(p: Vec<String>) {
    let _ = ONLY.set(p);
}

pub fn only() -> Vec<String> {
    ONLY.get().cloned().unwrap_or_default()
}

pub fn set_keep(keep: Vec<String>) {
    let _ = KEEP.set(keep);
}

/// Start JSON mode: stdout's real file is kept for the one JSON document, and
/// everything else written to stdout from now on goes to stderr.
pub fn begin() {
    if SAVED_STDOUT.load(Ordering::SeqCst) >= 0 {
        return;
    }
    let _ = std::io::stdout().flush();
    // SAFETY: plain descriptor duplication on the process's own stdio.
    unsafe {
        let saved = libc::dup(1);
        if saved >= 0 && libc::dup2(2, 1) >= 0 {
            SAVED_STDOUT.store(saved, Ordering::SeqCst);
        }
    }
}

/// Write the JSON document to the real stdout (or stdout, outside JSON mode).
pub fn emit(v: &Value) {
    let text = format!("{}\n", serde_json::to_string(v).unwrap_or_else(|_| "{}".into()));
    let _ = std::io::stdout().flush();
    let fd = SAVED_STDOUT.load(Ordering::SeqCst);
    if fd >= 0 {
        // SAFETY: writing a byte buffer to a descriptor we duplicated ourselves.
        unsafe {
            let mut off = 0usize;
            let b = text.as_bytes();
            while off < b.len() {
                let n = libc::write(fd, b[off..].as_ptr() as *const libc::c_void, b.len() - off);
                if n <= 0 {
                    break;
                }
                off += n as usize;
            }
        }
    } else {
        print!("{}", text);
    }
}

/// `NOG_ASKPASS=1`: ask for the password through the system's window.
pub fn askpass() -> bool {
    std::env::var("NOG_ASKPASS").map(|v| v == "1").unwrap_or(false)
}

/// `sudo`, with `-A` when the password window is wanted.
pub fn sudo() -> Command {
    let mut c = Command::new("sudo");
    if askpass() {
        c.arg("-A");
    }
    c
}

/// Extra arguments for the AUR helper so its own sudo uses the window too.
pub fn helper_sudo_args() -> Vec<String> {
    if askpass() { vec!["--sudoflags".into(), "-A".into()] } else { Vec::new() }
}

pub fn tier_number(t: &Tier) -> u8 {
    match t {
        Tier::One => 1,
        Tier::Two => 2,
        Tier::Three => 3,
    }
}

// ── nog list --json ─────────────────────────────────────────────────────────

/// One installed package, read from pacman's local database.
#[derive(Debug, Default, Clone)]
pub struct Installed {
    pub name: String,
    pub version: String,
    pub desc: String,
    pub explicit: bool,
    pub size: u64,
    pub installed: u64,
    pub depends: Vec<String>,
    pub provides: Vec<String>,
    pub groups: Vec<String>,
}

fn strip_dep(d: &str) -> &str {
    d.split(|c| c == '<' || c == '>' || c == '=' || c == ':').next().unwrap_or(d).trim()
}

/// Parse one `desc` file (pacman's local database format).
pub fn parse_desc(text: &str) -> Installed {
    let mut p = Installed { explicit: true, ..Default::default() };
    let mut key = "";
    for line in text.lines() {
        if line.starts_with('%') && line.ends_with('%') {
            key = line;
            continue;
        }
        if line.is_empty() {
            continue;
        }
        match key {
            "%NAME%" => p.name = line.to_string(),
            "%VERSION%" => p.version = line.to_string(),
            "%DESC%" => p.desc = line.to_string(),
            "%REASON%" => p.explicit = line.trim() != "1",
            "%SIZE%" => p.size = line.trim().parse().unwrap_or(0),
            "%INSTALLDATE%" => p.installed = line.trim().parse().unwrap_or(0),
            "%DEPENDS%" => p.depends.push(strip_dep(line).to_string()),
            "%PROVIDES%" => p.provides.push(strip_dep(line).to_string()),
            "%GROUPS%" => p.groups.push(line.to_string()),
            _ => {}
        }
    }
    p
}

pub fn load_installed(root: &Path) -> Vec<Installed> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(root) {
        for e in entries.flatten() {
            if let Ok(t) = fs::read_to_string(e.path().join("desc")) {
                let p = parse_desc(&t);
                if !p.name.is_empty() {
                    out.push(p);
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Who needs whom: for every installed package, the installed packages that
/// depend on it (directly, by name or by something it provides).
pub fn required_by(pkgs: &[Installed]) -> HashMap<String, Vec<String>> {
    let mut providers: HashMap<&str, Vec<&str>> = HashMap::new();
    for p in pkgs {
        providers.entry(p.name.as_str()).or_default().push(p.name.as_str());
        for pr in &p.provides {
            providers.entry(pr.as_str()).or_default().push(p.name.as_str());
        }
    }
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for p in pkgs {
        for d in &p.depends {
            if let Some(names) = providers.get(d.as_str()) {
                for n in names {
                    if *n != p.name {
                        let v = out.entry(n.to_string()).or_default();
                        if !v.contains(&p.name) {
                            v.push(p.name.clone());
                        }
                    }
                }
            }
        }
    }
    for v in out.values_mut() {
        v.sort();
    }
    out
}

/// Why a package may not be removed from nogForge, in plain words — or None.
/// Javier, 3 Oct 2026: system packages can't be touched unless nothing needs them.
pub fn protection(p: &Installed, tier: &Tier, base: &HashSet<String>, needed_by: &[String]) -> Option<String> {
    if p.name == "base" || base.contains(&p.name) {
        return Some("part of the base system".into());
    }
    if *tier == Tier::One {
        return Some("the system needs this to start".into());
    }
    if !needed_by.is_empty() {
        let shown: Vec<&str> = needed_by.iter().take(2).map(|s| s.as_str()).collect();
        let more = needed_by.len().saturating_sub(2);
        return Some(if more > 0 {
            format!("needed by {} and {} more", shown.join(", "), more)
        } else {
            format!("needed by {}", shown.join(" and "))
        });
    }
    None
}

pub fn list_json(tm: &TierManager, local_root: &Path, repos: &HashMap<String, String>) -> Value {
    let pkgs = load_installed(local_root);
    let req = required_by(&pkgs);
    let base: HashSet<String> = pkgs
        .iter()
        .find(|p| p.name == "base")
        .map(|b| b.depends.iter().cloned().collect())
        .unwrap_or_default();
    let rows: Vec<Value> = pkgs
        .iter()
        .map(|p| {
            let tier = tm.classify(&p.name);
            let needed = req.get(&p.name).cloned().unwrap_or_default();
            let source = repos.get(&p.name).cloned().unwrap_or_else(|| "aur".into());
            json!({
                "name": p.name, "version": p.version, "description": p.desc,
                "tier": tier_number(&tier), "source": source, "explicit": p.explicit,
                "size": p.size, "installed": p.installed, "groups": p.groups,
                "required_by": needed,
                "protected": protection(p, &tier, &base, &needed),
            })
        })
        .collect();
    json!({"nog": env!("CARGO_PKG_VERSION"), "kind": "list", "packages": rows})
}

// ── nog search --json ───────────────────────────────────────────────────────

/// Parse `pacman -Ss` / `<helper> -Ssa` output: `repo/name version [flags]`
/// followed by an indented description line.
pub fn parse_search(text: &str) -> Vec<(String, String, String, bool, String)> {
    let mut out = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if !l.starts_with(' ') && !l.starts_with('\t') && l.contains('/') {
            let mut parts = l.split_whitespace();
            let first = parts.next().unwrap_or("");
            let (repo, name) = first.split_once('/').unwrap_or(("", first));
            let version = parts.next().unwrap_or("").to_string();
            let installed = l.contains("[installed") || l.contains("(Installed");
            let desc = lines.get(i + 1).filter(|n| n.starts_with(' ') || n.starts_with('\t'))
                .map(|n| n.trim().to_string()).unwrap_or_default();
            out.push((repo.to_string(), name.to_string(), version, installed, desc));
        }
        i += 1;
    }
    out
}

pub fn search_json(tm: &TierManager, query: &str, repo_out: &str, aur_out: Option<&str>) -> Value {
    let mut rows: Vec<Value> = Vec::new();
    let mut seen = HashSet::new();
    for (text, aur) in [(repo_out, false)].into_iter().chain(aur_out.map(|t| (t, true))) {
        for (repo, name, version, installed, desc) in parse_search(text) {
            if aur && repo != "aur" {
                continue; // the helper repeats repo results; those came from pacman already
            }
            if !seen.insert(name.clone()) {
                continue;
            }
            let tier = tm.classify(&name);
            rows.push(json!({
                "name": name, "version": version, "description": desc,
                "source": repo, "installed": installed, "tier": tier_number(&tier),
            }));
        }
    }
    json!({"nog": env!("CARGO_PKG_VERSION"), "kind": "search", "query": query, "results": rows})
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESC: &str = "%NAME%\nsteam\n\n%VERSION%\n1.0.0.87-3\n\n%DESC%\nValve's digital software delivery system\n\n%SIZE%\n20475439\n\n%REASON%\n1\n\n%DEPENDS%\nbash\ncoreutils>=9\nlib32-glibc\n\n%PROVIDES%\nsteam-runtime=1\n";

    #[test]
    fn desc_is_read() {
        let p = parse_desc(DESC);
        assert_eq!(p.name, "steam");
        assert_eq!(p.version, "1.0.0.87-3");
        assert!(!p.explicit, "REASON 1 means installed as a dependency");
        assert_eq!(p.depends, vec!["bash", "coreutils", "lib32-glibc"]);
        assert_eq!(p.provides, vec!["steam-runtime"]);
        assert_eq!(p.size, 20475439);
        assert!(parse_desc("%NAME%\nx\n").explicit, "no REASON line means you chose it");
    }

    fn pkg(name: &str, deps: &[&str], provides: &[&str]) -> Installed {
        Installed {
            name: name.into(),
            depends: deps.iter().map(|s| s.to_string()).collect(),
            provides: provides.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn who_needs_whom_by_name_and_by_provides() {
        let pkgs = vec![pkg("glibc", &[], &[]), pkg("firefox", &["glibc", "sh"], &[]), pkg("bash", &["glibc"], &["sh"]),
                        pkg("steam", &["glibc"], &[])];
        let r = required_by(&pkgs);
        assert_eq!(r["glibc"], vec!["bash", "firefox", "steam"]);
        assert_eq!(r["bash"], vec!["firefox"], "firefox needs sh, which bash provides");
        assert!(r.get("steam").is_none());
    }

    #[test]
    fn protection_in_plain_words() {
        let base: HashSet<String> = ["bash".to_string()].into_iter().collect();
        let p = pkg("bash", &[], &[]);
        assert_eq!(protection(&p, &Tier::Three, &base, &[]).as_deref(), Some("part of the base system"));
        let k = pkg("linux-zen", &[], &[]);
        assert_eq!(protection(&k, &Tier::One, &HashSet::new(), &[]).as_deref(), Some("the system needs this to start"));
        let l = pkg("libfoo", &[], &[]);
        let by = vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()];
        assert_eq!(protection(&l, &Tier::Three, &HashSet::new(), &by).as_deref(), Some("needed by a, b and 2 more"));
        assert_eq!(protection(&l, &Tier::Three, &HashSet::new(), &by[..2]).as_deref(), Some("needed by a and b"));
        let s = pkg("steam", &[], &[]);
        assert_eq!(protection(&s, &Tier::Three, &HashSet::new(), &[]), None, "nothing needs it: removable");
    }

    #[test]
    fn search_output_is_read() {
        let repo = "extra/krita 6.0.4-2\n    Edit and paint images\nextra/gimp 3.2.6-2 [installed]\n    GNU Image Manipulation Program\n";
        let aur = "extra/krita 6.0.4-2\n    Edit and paint images\naur/photoqt 4.8-1 (+12 0.40)\n    Image viewer\n";
        let r = parse_search(repo);
        assert_eq!(r.len(), 2);
        assert_eq!(r[1], ("extra".into(), "gimp".into(), "3.2.6-2".into(), true, "GNU Image Manipulation Program".into()));
        let f = std::env::temp_dir().join(format!("nog-machine-test-{}.toml", std::process::id()));
        std::fs::write(&f, "[tier1]\nmanual_signoff = false\npackages = []\n[tier2]\nmanual_signoff = false\npackages = []\n[tier3]\nmanual_signoff = false\npackages = []\n").unwrap();
        let tm = TierManager::load(f.to_str().unwrap()).expect("minimal pins load");
        let _ = std::fs::remove_file(&f);
        let v = search_json(&tm, "image", repo, Some(aur));
        let names: Vec<&str> = v["results"].as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["krita", "gimp", "photoqt"], "the helper's repeat of krita is dropped");
        assert_eq!(v["results"][2]["source"], "aur");
    }

    #[test]
    fn sudo_asks_through_the_window_only_when_wanted() {
        std::env::remove_var("NOG_ASKPASS");
        assert!(sudo().get_args().next().is_none());
        assert!(helper_sudo_args().is_empty());
        std::env::set_var("NOG_ASKPASS", "1");
        assert_eq!(sudo().get_args().collect::<Vec<_>>(), vec!["-A"]);
        assert_eq!(helper_sudo_args(), vec!["--sudoflags", "-A"]);
        std::env::remove_var("NOG_ASKPASS");
    }
}
