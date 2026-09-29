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

/// Who actually links a set of sonames, read from the binaries themselves
/// (v1.4.2, issue #16). Keys are soname provides as pacman writes them
/// (`libbluray.so=3-64`), so the soname rule can look them up exactly as it
/// looks up `%DEPENDS%`.
#[derive(Debug, Default)]
pub struct Linkage {
    /// soname -> installed packages owning an ELF file whose `DT_NEEDED`
    /// names it, at the matching word size.
    pub linked_by: HashMap<String, Vec<String>>,
    /// soname -> installed packages owning the library file itself on the
    /// loader's default path (`usr/lib/` for 64-bit, `usr/lib32/` for 32-bit).
    /// A package can ship a library without declaring the provide; if it
    /// stays, the library stays, and nothing breaks.
    pub file_providers: HashMap<String, Vec<String>>,
}

/// Directories scanned for linkers. Everything that can hold a program or a
/// library the loader will open; `usr/share`, `usr/include` and `etc` hold
/// data, headers and configuration, and skipping them cuts the file count by
/// most of an order of magnitude.
const SCAN_PREFIXES: &[&str] = &["usr/lib/", "usr/lib32/", "usr/bin/", "usr/libexec/", "opt/"];

/// Extensions that are never a program or shared library. Of ~143,000 files
/// under the scan prefixes on the reference machine only ~10,000 are ELF; the
/// rest are headers, Python and Go sources, compressed firmware, images,
/// static archives, Windows DLLs (wine) and the like. Opening them costs a
/// disk read each — most of the 20s a cold scan took before this list.
///
/// Skipping a file can only cause a missed hold, never a false one, so an
/// extension belongs here only when it is plainly data.
const DATA_EXTENSIONS: &[&str] = &[
    "h", "hpp", "zst", "xz", "gz", "bz2", "go", "py", "pyc", "pyi", "cmake", "js", "png",
    "svg", "txt", "qml", "qmltypes", "pc", "dll", "exe", "json", "a", "mod", "html", "pak",
    "s", "tplg", "service", "pl", "pm", "rb", "lua", "xml", "conf", "md", "mo", "desktop",
    "ttf", "otf", "ico", "jpg", "css", "sh", "rules", "hwdb", "typelib", "gir", "jar",
    "class", "map", "def", "la", "tcl", "el", "elc", "ps", "rs", "c", "cpp", "yaml", "toml",
];

/// Is this path worth opening to look for `DT_NEEDED`?
fn scannable(path: &str) -> bool {
    if path.ends_with('/') || !SCAN_PREFIXES.iter().any(|p| path.starts_with(p)) {
        return false;
    }
    let base = path.rsplit('/').next().unwrap_or(path);
    match base.rsplit_once('.') {
        Some((_, ext)) => !DATA_EXTENSIONS.contains(&ext),
        None => true,
    }
}

/// Scan every installed package's files for linkers of `targets`.
///
/// This is the expensive half of issue #16 — it opens every program and
/// library on the system — so the caller runs it only when a pending update
/// actually drops a soname, which is about one update in 130.
///
/// Soft-fails like `load_installed`: an unreadable package contributes
/// nothing. That errs toward a missed hold, the same outcome nog had before
/// v1.4.2, never toward a hold that should not exist.
pub fn scan_linkage(targets: &[String]) -> Linkage {
    if targets.is_empty() {
        return Linkage::default();
    }
    let lists = load_file_lists();
    linkage_from(&lists, targets, |rel| {
        let path = std::path::Path::new("/").join(rel);
        // Symlinks point at a file the owning package also lists, so reading
        // through them would only count the same binary twice.
        match fs::symlink_metadata(&path) {
            Ok(m) if m.file_type().is_file() && m.len() >= 52 => crate::elf::needed(&path),
            _ => None,
        }
    })
}

/// The decision half of `scan_linkage`, with the file reading passed in so it
/// can be tested without a real system.
fn linkage_from<F>(lists: &HashMap<String, Vec<String>>, targets: &[String], read: F) -> Linkage
where
    F: Fn(&str) -> Option<(crate::elf::Class, Vec<String>)>,
{
    // loader filename -> (class, soname provide it came from)
    let mut wanted: HashMap<String, Vec<(crate::elf::Class, &str)>> = HashMap::new();
    for t in targets {
        if let Some((file, class)) = crate::elf::provide_to_needed(t) {
            wanted.entry(file).or_default().push((class, t.as_str()));
        }
    }

    let mut out = Linkage::default();
    for (pkg, files) in lists {
        // Basenames this package ships. A package that carries its own copy
        // of a library (thunderbird's bundled libs, reached through RUNPATH)
        // is not broken when the system copy goes, so it is not a linker of
        // the system one for this purpose.
        let own: std::collections::HashSet<&str> = files
            .iter()
            .filter_map(|f| f.rsplit('/').next())
            .collect();

        for f in files {
            if f.ends_with('/') || !SCAN_PREFIXES.iter().any(|p| f.starts_with(p)) {
                continue;
            }
            let base = f.rsplit('/').next().unwrap_or(f);
            if let Some(entries) = wanted.get(base) {
                for (class, provide) in entries {
                    let dir = match class {
                        crate::elf::Class::Elf64 => "usr/lib/",
                        crate::elf::Class::Elf32 => "usr/lib32/",
                    };
                    if f.strip_prefix(dir) == Some(base) {
                        push_unique(&mut out.file_providers, provide, pkg);
                    }
                }
            }
        }

        for f in files {
            if !scannable(f) {
                continue;
            }
            let Some((class, needs)) = read(f) else { continue };
            for n in needs {
                if own.contains(n.as_str()) {
                    continue;
                }
                if let Some(entries) = wanted.get(&n) {
                    for (c, provide) in entries {
                        if *c == class {
                            push_unique(&mut out.linked_by, provide, pkg);
                        }
                    }
                }
            }
        }
    }
    for v in out.linked_by.values_mut().chain(out.file_providers.values_mut()) {
        v.sort_unstable();
    }
    out
}

fn push_unique(map: &mut HashMap<String, Vec<String>>, key: &str, pkg: &str) {
    let v = map.entry(key.to_string()).or_default();
    if !v.iter().any(|p| p == pkg) {
        v.push(pkg.to_string());
    }
}

/// Every installed package's `%FILES%` list, paths relative to `/`.
fn load_file_lists() -> HashMap<String, Vec<String>> {
    let entries = match fs::read_dir(LOCAL_DB_DIR) {
        Ok(e) => e,
        Err(_) => return HashMap::new(),
    };
    let mut out = HashMap::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        let (Ok(desc), Ok(files)) = (
            fs::read_to_string(dir.join("desc")),
            fs::read_to_string(dir.join("files")),
        ) else {
            continue;
        };
        let Some((name, _)) = parse_desc(&desc) else { continue };
        out.insert(name, parse_files(&files));
    }
    out
}

/// The `%FILES%` block of a local `files` entry: one path per line until a
/// blank line or the next key (`%BACKUP%` follows it).
fn parse_files(contents: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_files = false;
    for line in contents.lines() {
        let t = line.trim();
        if t.starts_with('%') && t.ends_with('%') {
            in_files = t == "%FILES%";
            continue;
        }
        if t.is_empty() {
            in_files = false;
            continue;
        }
        if in_files {
            out.push(t.to_string());
        }
    }
    out
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

    // ---- issue #16: linkage read from the binaries --------------------------

    use crate::elf::Class;

    fn lists(entries: &[(&str, &[&str])]) -> HashMap<String, Vec<String>> {
        entries
            .iter()
            .map(|(n, fs)| (n.to_string(), fs.iter().map(|s| s.to_string()).collect()))
            .collect()
    }

    /// Fake binaries: path -> (class, DT_NEEDED).
    fn fake(bins: &'static [(&'static str, Class, &'static [&'static str])])
        -> impl Fn(&str) -> Option<(Class, Vec<String>)>
    {
        move |p: &str| {
            bins.iter()
                .find(|(path, _, _)| *path == p)
                .map(|(_, c, n)| (*c, n.iter().map(|s| s.to_string()).collect()))
        }
    }

    #[test]
    fn finds_the_undeclared_linker_from_the_ffmpeg_obs_incident() {
        // ffmpeg-obs declared plain `libbluray`; only its binary knew the soname.
        let l = lists(&[
            ("libbluray", &["usr/", "usr/lib/", "usr/lib/libbluray.so.3", "usr/lib/libbluray.so"]),
            ("ffmpeg-obs", &["usr/lib/", "usr/lib/libavformat.so.63"]),
            ("vlc", &["usr/bin/vlc"]),
        ]);
        let read = fake(&[
            ("usr/lib/libavformat.so.63", Class::Elf64, &["libbluray.so.3", "libc.so.6"]),
            ("usr/bin/vlc", Class::Elf64, &["libc.so.6"]),
        ]);
        let got = linkage_from(&l, &["libbluray.so=3-64".to_string()], read);
        assert_eq!(got.linked_by["libbluray.so=3-64"], vec!["ffmpeg-obs"]);
        assert_eq!(got.file_providers["libbluray.so=3-64"], vec!["libbluray"]);
    }

    #[test]
    fn a_package_that_bundles_its_own_copy_is_not_a_linker() {
        // thunderbird-style: ships the library next to the binary.
        let l = lists(&[("bundler", &["opt/app/app", "opt/app/libbluray.so.3"])]);
        let read = fake(&[("opt/app/app", Class::Elf64, &["libbluray.so.3"])]);
        let got = linkage_from(&l, &["libbluray.so=3-64".to_string()], read);
        assert!(got.linked_by.is_empty());
        // And its private copy is off the loader path, so it provides nothing.
        assert!(got.file_providers.is_empty());
    }

    #[test]
    fn word_size_must_match() {
        // A 32-bit binary needing libEGL.so.1 does not link the 64-bit one.
        let l = lists(&[("wine", &["usr/lib32/wine/foo.so"])]);
        let read = fake(&[("usr/lib32/wine/foo.so", Class::Elf32, &["libEGL.so.1"])]);
        let got = linkage_from(&l, &["libEGL.so=1-64".to_string()], &read);
        assert!(got.linked_by.is_empty());
        let got = linkage_from(&l, &["libEGL.so=1-32".to_string()], &read);
        assert_eq!(got.linked_by["libEGL.so=1-32"], vec!["wine"]);
    }

    #[test]
    fn data_directories_are_never_opened() {
        let l = lists(&[("docs", &["usr/share/doc/thing", "etc/thing.conf"])]);
        let read = |p: &str| -> Option<(Class, Vec<String>)> {
            panic!("opened {p}, which is outside the scan prefixes")
        };
        linkage_from(&l, &["libbluray.so=3-64".to_string()], read);
    }

    #[test]
    fn versioned_libraries_are_scanned_and_plain_data_is_not() {
        // `libfoo.so.3.1.0` ends in `.0`: a version, not a data extension.
        assert!(scannable("usr/lib/libavformat.so.63.1.101"));
        assert!(scannable("usr/lib/libbluray.so.3"));
        assert!(scannable("usr/bin/vlc"));
        assert!(scannable("usr/lib/qt6/plugins/imageformats/libqjpeg.so"));
        assert!(!scannable("usr/lib/python3.14/os.py"));
        assert!(!scannable("usr/lib/firmware/foo.bin.zst"));
        assert!(!scannable("usr/lib/wine/x86_64-windows/kernel32.dll"));
        assert!(!scannable("usr/share/doc/thing"));
        assert!(!scannable("usr/lib/"));
    }

    #[test]
    fn parse_files_stops_at_the_next_key() {
        let f = "%FILES%\nusr/\nusr/lib/libfoo.so.1\n\n%BACKUP%\netc/foo.conf\tabc\n";
        assert_eq!(parse_files(f), vec!["usr/", "usr/lib/libfoo.so.1"]);
    }

    /// Reads this machine. `#[ignore]`d because its result depends on the box:
    /// a diagnostic for comparing against `readelf`, and for timing the scan.
    ///
    /// `cargo test --release live_scan -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_scan_of_this_machine() {
        let targets: Vec<String> = ["libbluray.so=4-64", "libc.so=6-64", "libGL.so=1-32"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let t = std::time::Instant::now();
        let l = scan_linkage(&targets);
        println!("scan took {:?}", t.elapsed());
        for k in &targets {
            let by = l.linked_by.get(k).map(|v| v.len()).unwrap_or(0);
            println!("{k}: linked by {by} packages, file on path from {:?}", l.file_providers.get(k));
        }
        println!("libbluray.so=4-64 linkers: {:?}", l.linked_by.get("libbluray.so=4-64"));
    }
}
