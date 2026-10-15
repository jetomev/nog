//! v2.0.0 (C5): maintenance — what each source leaves behind and how much it
//! takes. Report first, act on confirmation (the `nog update` shape); every
//! category independently selectable; nothing outside package management.
//! The pacman download cache is `cache.rs` (v1.5.5); this module adds the
//! orphans, the AUR helper's build trees, Flatpak's unused runtimes and Snap's
//! kept old revisions. Everything here is pure or a plain read; the commands
//! module does the asking and the removing.

use std::path::{Path, PathBuf};

/// A category of the maintenance report. `id` is what `--only` and nogForge
/// name; `tool` is who removes it and how it asks.
#[derive(Debug, Clone, PartialEq)]
pub struct Category {
    pub id: &'static str,
    pub what: String,
    pub items: usize,
    pub bytes: u64,
    pub names: Vec<String>,
    pub how: String,
}

/// An orphan: installed as a dependency, needed by nothing now.
#[derive(Debug, Clone, PartialEq)]
pub struct Orphan {
    pub name: String,
    pub size: u64,
}

/// `pacman -Qtdq`, each with its installed size from the local database.
pub fn orphans(sizes: &dyn Fn(&str) -> u64) -> Vec<Orphan> {
    let output = match std::process::Command::new("pacman").args(["-Qtdq"]).env("LC_ALL", "C").output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    // exit 1 with empty output means "no orphans", which is fine
    String::from_utf8_lossy(&output.stdout).lines().map(str::trim).filter(|l| !l.is_empty())
        .map(|n| Orphan { name: n.to_string(), size: sizes(n) }).collect()
}

/// A build tree the AUR helper left behind (yay: `~/.cache/yay/<pkg>`; paru:
/// `~/.cache/paru/clone/<pkg>`).
#[derive(Debug, Clone, PartialEq)]
pub struct BuildTree {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
}

/// Where a helper keeps its clones. Pure.
pub fn helper_cache_dir(helper: &str, home: &Path) -> PathBuf {
    match helper {
        "paru" => home.join(".cache").join("paru").join("clone"),
        _ => home.join(".cache").join("yay"),
    }
}

/// The directories in a helper cache (its files — `completion.cache` and
/// friends — are not build trees and stay).
pub fn build_trees(dir: &Path) -> Vec<BuildTree> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<BuildTree> = rd.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).map(|e| {
        let path = e.path();
        BuildTree { name: e.file_name().to_string_lossy().to_string(), bytes: dir_size(&path), path }
    }).collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Bytes under a directory (what `du -sb` counts), without following links.
pub fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(rd) = std::fs::read_dir(path) else { return 0 };
    for e in rd.flatten() {
        let Ok(md) = e.metadata() else { continue };
        if md.is_dir() { total += dir_size(&e.path()); } else { total += md.len(); }
    }
    total
}

/// Pure. The `--only` words, checked against the report's ids; unknown ones
/// are returned so the run can stop and name them.
pub fn pick_only(only: &[String], ids: &[&str]) -> Result<Vec<String>, Vec<String>> {
    let mut good = Vec::new();
    let mut bad = Vec::new();
    for o in only {
        for w in o.split(',').map(str::trim).filter(|w| !w.is_empty()) {
            if ids.contains(&w) { if !good.iter().any(|g: &String| g == w) { good.push(w.to_string()); } }
            else { bad.push(w.to_string()); }
        }
    }
    if bad.is_empty() { Ok(good) } else { Err(bad) }
}

/// Pure. The answer to "Clean up which?": `all`/`a`/`y`, numbers (1-based,
/// space or comma separated), or nothing/`n` for none. `None` = asked again.
pub fn parse_choice(answer: &str, count: usize) -> Option<Vec<usize>> {
    let a = answer.trim().to_lowercase();
    if a.is_empty() || a == "n" || a == "no" || a == "none" { return Some(Vec::new()); }
    if a == "all" || a == "a" || a == "y" || a == "yes" { return Some((0..count).collect()); }
    let mut picked = Vec::new();
    for w in a.split(|c: char| c == ',' || c.is_whitespace()).filter(|w| !w.is_empty()) {
        let n: usize = w.parse().ok()?;
        if n == 0 || n > count { return None; }
        if !picked.contains(&(n - 1)) { picked.push(n - 1); }
    }
    Some(picked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_helper_cache_is_where_each_helper_keeps_it() {
        let home = Path::new("/home/x");
        assert_eq!(helper_cache_dir("yay", home), PathBuf::from("/home/x/.cache/yay"));
        assert_eq!(helper_cache_dir("paru", home), PathBuf::from("/home/x/.cache/paru/clone"));
    }

    #[test]
    fn build_trees_are_the_directories_only_and_sized() {
        let dir = std::env::temp_dir().join(format!("nog-maint-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("cowsay").join("src")).unwrap();
        std::fs::write(dir.join("cowsay").join("PKGBUILD"), "x".repeat(100)).unwrap();
        std::fs::write(dir.join("cowsay").join("src").join("a"), "y".repeat(50)).unwrap();
        std::fs::write(dir.join("completion.cache"), "not a tree").unwrap();
        let trees = build_trees(&dir);
        assert_eq!(trees.len(), 1);
        assert_eq!(trees[0].name, "cowsay");
        assert_eq!(trees[0].bytes, 150);
        assert!(build_trees(&dir.join("nope")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_words_are_checked_against_the_ids() {
        let ids = ["cache", "orphans", "aur", "flatpak", "snap"];
        assert_eq!(pick_only(&["cache,snap".into(), "aur".into()], &ids).unwrap(), vec!["cache", "snap", "aur"]);
        assert_eq!(pick_only(&["cache,typo".into()], &ids).unwrap_err(), vec!["typo"]);
        assert!(pick_only(&[], &ids).unwrap().is_empty());
    }

    #[test]
    fn the_choice_is_all_numbers_or_none_and_nonsense_is_asked_again() {
        assert_eq!(parse_choice("", 3), Some(vec![]));
        assert_eq!(parse_choice("n", 3), Some(vec![]));
        assert_eq!(parse_choice("all", 3), Some(vec![0, 1, 2]));
        assert_eq!(parse_choice("1 3", 3), Some(vec![0, 2]));
        assert_eq!(parse_choice("2,2", 3), Some(vec![1]));
        assert_eq!(parse_choice("4", 3), None);
        assert_eq!(parse_choice("x", 3), None);
    }
}
