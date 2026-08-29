//! Reader for the pacman **local** database — what is installed right now.
//!
//! `sync_db.rs` reads the repository metadata: what a package will become.
//! This reads `/var/lib/pacman/local/<name>-<ver>/desc`: what it currently is.
//! Both use the same `%KEY%` format, but the local database is a directory of
//! plain uncompressed files, so there is nothing to decompress and no new
//! dependency to take on.
//!
//! Added in v1.3.1 for issue #13. The soname coupling rule needs two things
//! nog had never had to ask about:
//!
//!   * `%PROVIDES%` — which installed package currently provides a soname, so
//!     the rule can identify the *actual* provider rather than guessing from
//!     the package name.
//!   * `%DEPENDS%` — which installed packages still require that soname, so
//!     the rule knows who would break.
//!
//! Only the two fields are kept. The local database holds a full description,
//! file list and install reason for every package on the system; carrying all
//! of that for 1400 packages to answer one question would be wasteful.

use std::collections::HashMap;
use std::fs;

const LOCAL_DB_DIR: &str = "/var/lib/pacman/local";

/// The two dependency-graph fields of one installed package.
#[derive(Debug, Clone, Default)]
pub struct InstalledDesc {
    /// `%PROVIDES%` — includes sonames such as `libbluray.so=3-64` alongside
    /// plain virtual-package names such as `sh`. Kept verbatim; the caller
    /// decides what a soname looks like.
    pub provides: Vec<String>,
    /// `%DEPENDS%` — same shape.
    pub depends: Vec<String>,
}

/// Read every installed package's `%PROVIDES%` and `%DEPENDS%`.
///
/// Soft-fails to an empty map: an unreadable local database means the soname
/// rule simply does not fire, which leaves nog behaving exactly as it did
/// before v1.3.1. That is the right failure direction here — the rule exists
/// to catch a transaction pacman would refuse anyway, so losing it costs a
/// clear error message from pacman, not a broken system.
///
/// Directories that cannot be read are skipped individually rather than
/// aborting the whole load, and the count of successes is returned alongside
/// so the caller can tell a partial read from a complete one.
pub fn load_installed() -> HashMap<String, InstalledDesc> {
    let entries = match fs::read_dir(LOCAL_DB_DIR) {
        Ok(e) => e,
        Err(_) => return HashMap::new(),
    };

    let mut out = HashMap::new();
    for entry in entries.flatten() {
        let desc = entry.path().join("desc");
        let contents = match fs::read_to_string(&desc) {
            Ok(c) => c,
            // Not every directory holds a readable desc (ALPM keeps `files`
            // and lock files alongside). Skipping one is not a failure.
            Err(_) => continue,
        };
        if let Some((name, d)) = parse_desc(&contents) {
            out.insert(name, d);
        }
    }
    out
}

/// Pull `%NAME%`, `%PROVIDES%` and `%DEPENDS%` out of a local desc file.
///
/// Unlike the sync database's single-value fields, these are **lists**: a
/// `%KEY%` line is followed by one value per line until a blank line or the
/// next key. Returns `None` without `%NAME%` — an entry nog cannot name is an
/// entry it cannot use.
fn parse_desc(contents: &str) -> Option<(String, InstalledDesc)> {
    let mut name: Option<String> = None;
    let mut provides: Vec<String> = Vec::new();
    let mut depends: Vec<String> = Vec::new();

    let mut key: Option<&str> = None;
    for line in contents.lines() {
        let t = line.trim();
        if t.is_empty() {
            key = None;
            continue;
        }
        if t.starts_with('%') && t.ends_with('%') {
            key = Some(t);
            continue;
        }
        match key {
            Some("%NAME%") if name.is_none() => name = Some(t.to_string()),
            Some("%PROVIDES%") => provides.push(t.to_string()),
            Some("%DEPENDS%") => depends.push(t.to_string()),
            _ => {}
        }
    }

    name.map(|n| (n, InstalledDesc { provides, depends }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESC: &str = "\
%NAME%
libbluray

%VERSION%
1.4.1-1

%DEPENDS%
glibc
libxml2

%PROVIDES%
libbluray.so=3-64
";

    #[test]
    fn reads_name_provides_and_depends() {
        let (name, d) = parse_desc(DESC).expect("desc should parse");
        assert_eq!(name, "libbluray");
        assert_eq!(d.provides, vec!["libbluray.so=3-64"]);
        assert_eq!(d.depends, vec!["glibc", "libxml2"]);
    }

    #[test]
    fn multi_value_fields_do_not_bleed_into_each_other() {
        // The bug this guards: treating every line after %DEPENDS% as a
        // dependency until end of file, swallowing %PROVIDES% with it.
        let (_, d) = parse_desc(DESC).unwrap();
        assert!(
            !d.depends.iter().any(|x| x.contains(".so=")),
            "a soname leaked from %PROVIDES% into %DEPENDS%"
        );
    }

    #[test]
    fn missing_fields_are_empty_not_fatal() {
        let (name, d) = parse_desc("%NAME%\nfoo\n").unwrap();
        assert_eq!(name, "foo");
        assert!(d.provides.is_empty());
        assert!(d.depends.is_empty());
    }

    #[test]
    fn no_name_is_no_entry() {
        assert!(parse_desc("%PROVIDES%\nlibfoo.so=1-64\n").is_none());
    }
}
