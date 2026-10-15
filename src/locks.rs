//! v2.0.0 (C4, `nog lock` / `nog unlock`): packages held back from every
//! update until you say otherwise. System-wide, in `/etc/nog/locks.toml`,
//! written the way tier pins are (`sudo tee`), so every user of the computer
//! and nogForge see the same locks. A locked package shows in `nog update`'s
//! Held list as "locked by you" and is passed to pacman as `--ignore`, as a
//! held package is. Missing file = no locks; an unreadable one fails closed:
//! nothing is treated as locked, and the update says so.

use serde::Deserialize;

pub const DEFAULT_PATH: &str = "/etc/nog/locks.toml";

#[derive(Deserialize, Default)]
struct FileFormat {
    #[serde(default)]
    locked: Locked,
}

#[derive(Deserialize, Default)]
struct Locked {
    #[serde(default)]
    packages: Vec<String>,
}

/// Pure. The locked names, in file order, each once.
pub fn parse(text: &str) -> Result<Vec<String>, String> {
    let f: FileFormat = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut out: Vec<String> = Vec::new();
    for p in f.locked.packages {
        if !out.contains(&p) { out.push(p); }
    }
    Ok(out)
}

/// Pure. The file for a set of locks, with its explanation.
pub fn render(locked: &[String]) -> String {
    let mut out = String::from(
        "# nog locks (v2.0.0). A locked package is never updated until it is\n\
         # unlocked: `nog lock <pkg>` adds one, `nog unlock <pkg>` removes it,\n\
         # `nog lock` alone lists them. Written by nog; edit by hand if you like.\n\
         [locked]\npackages = [\n");
    for p in locked {
        out.push_str(&format!("    \"{}\",\n", p));
    }
    out.push_str("]\n");
    out
}

/// What the file says, or why it could not be read (`Err`) — the caller
/// decides what a problem means. A missing file is no locks.
pub fn load(path: &str) -> Result<Vec<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text).map_err(|e| format!("{} is not readable as TOML: {}", path, e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("{} could not be read: {}", path, e)),
    }
}

/// Write the locks (root: `sudo tee`, like the tier pins).
pub fn save(path: &str, locked: &[String]) -> Result<(), String> {
    crate::tiers::write_as_root(path, &render(locked))
}

/// Pure. The lock list after adding `names`; what was already there is said.
pub fn with_added(current: &[String], names: &[String]) -> (Vec<String>, Vec<String>) {
    let mut out = current.to_vec();
    let mut already = Vec::new();
    for n in names {
        if out.contains(n) { already.push(n.clone()); } else { out.push(n.clone()); }
    }
    (out, already)
}

/// Pure. The lock list after removing `names`; what was not locked is said.
pub fn with_removed(current: &[String], names: &[String]) -> (Vec<String>, Vec<String>) {
    let mut not_locked = Vec::new();
    for n in names {
        if !current.contains(n) { not_locked.push(n.clone()); }
    }
    (current.iter().filter(|c| !names.contains(c)).cloned().collect(), not_locked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> { items.iter().map(|s| s.to_string()).collect() }

    #[test]
    fn the_file_round_trips_and_a_missing_or_empty_one_is_no_locks() {
        let text = render(&v(&["linux", "mesa"]));
        assert_eq!(parse(&text).unwrap(), v(&["linux", "mesa"]));
        assert_eq!(parse("").unwrap(), Vec::<String>::new());
        assert_eq!(parse("[locked]\npackages = [\"a\", \"a\"]\n").unwrap(), v(&["a"]), "each name once");
        assert_eq!(load("/nonexistent/nog-locks-test.toml").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn garbage_is_an_error_not_an_empty_lock_list() {
        assert!(parse("[locked\npackages = ").is_err());
    }

    #[test]
    fn adding_and_removing_say_what_did_not_change() {
        let (now, already) = with_added(&v(&["linux"]), &v(&["linux", "mesa"]));
        assert_eq!(now, v(&["linux", "mesa"]));
        assert_eq!(already, v(&["linux"]));
        let (now, not_locked) = with_removed(&v(&["linux", "mesa"]), &v(&["mesa", "firefox"]));
        assert_eq!(now, v(&["linux"]));
        assert_eq!(not_locked, v(&["firefox"]));
    }
}
