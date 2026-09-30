//! v1.5.5 (issue #15) — `nog clean`: tier-aware package-cache retention.
//!
//! pacman keeps every package it ever downloaded in its cache
//! (`/var/cache/pacman/pkg` by default) and never removes one by itself. On
//! the development machine it had reached 28 GB by August 2026 and was back
//! to 19 GB by the end of September. `paccache` keeps the last N of
//! everything; nog already knows how much each package matters, so it keeps
//! more of what hurts to lose:
//!
//!   Tier 1 (kernel, glibc, systemd, bootloader)  3 versions — the rollback path
//!   Tier 2 (desktop and key apps)                2 versions
//!   Tier 3 (everything else)                     1 version
//!
//! Rules, per package:
//!   * The installed version is always kept. A held package is waiting on it.
//!   * Versions newer than the installed one are kept (downloaded for an
//!     update that has not finished; pacman will use them).
//!   * Of the older versions, the newest (keep - 1) are kept.
//!   * A package that is no longer installed has all its files removed.
//!   * A signature file (`.sig`) goes with its package.
//!
//! Version order is pacman's own (`alpm_pkg_vercmp`), ported below and tested
//! against the `vercmp` tool. The planning is pure; listing and deleting are
//! thin wrappers, and deleting goes through `sudo rm`, only ever on files
//! directly inside a configured cache directory.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// ---- pacman's version comparison -------------------------------------------

/// `alpm_pkg_vercmp`: compare two full versions, `[epoch:]pkgver[-pkgrel]`.
pub fn vercmp(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    let (e1, v1, r1) = parse_evr(a);
    let (e2, v2, r2) = parse_evr(b);
    let o = rpmvercmp(e1, e2);
    if o != Ordering::Equal {
        return o;
    }
    let o = rpmvercmp(v1, v2);
    if o != Ordering::Equal {
        return o;
    }
    match (r1, r2) {
        (Some(x), Some(y)) => rpmvercmp(x, y),
        _ => Ordering::Equal,
    }
}

/// Split `[epoch:]version[-release]`. A missing or empty epoch is "0".
fn parse_evr(s: &str) -> (&str, &str, Option<&str>) {
    let digits_end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let (epoch, rest) = if s[digits_end..].starts_with(':') {
        let e = &s[..digits_end];
        (if e.is_empty() { "0" } else { e }, &s[digits_end + 1..])
    } else {
        ("0", s)
    };
    match rest.rfind('-') {
        Some(i) => (epoch, &rest[..i], Some(&rest[i + 1..])),
        None => (epoch, rest, None),
    }
}

/// libalpm's `rpmvercmp`, segment by segment.
fn rpmvercmp(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    let (x, y) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0usize, 0usize);
    while i < x.len() && j < y.len() {
        let (si, sj) = (i, j);
        while i < x.len() && !x[i].is_ascii_alphanumeric() { i += 1; }
        while j < y.len() && !y[j].is_ascii_alphanumeric() { j += 1; }
        if i >= x.len() || j >= y.len() {
            break;
        }
        // Different separator lengths decide it.
        if i - si != j - sj {
            return if i - si < j - sj { Ordering::Less } else { Ordering::Greater };
        }
        let (mut ei, mut ej) = (i, j);
        let isnum = x[i].is_ascii_digit();
        if isnum {
            while ei < x.len() && x[ei].is_ascii_digit() { ei += 1; }
            while ej < y.len() && y[ej].is_ascii_digit() { ej += 1; }
        } else {
            while ei < x.len() && x[ei].is_ascii_alphabetic() { ei += 1; }
            while ej < y.len() && y[ej].is_ascii_alphabetic() { ej += 1; }
        }
        // Segment types differ: a number beats letters.
        if ej == j {
            return if isnum { Ordering::Greater } else { Ordering::Less };
        }
        let (mut s1, mut s2) = (&x[i..ei], &y[j..ej]);
        if isnum {
            while s1.first() == Some(&b'0') { s1 = &s1[1..]; }
            while s2.first() == Some(&b'0') { s2 = &s2[1..]; }
            if s1.len() != s2.len() {
                return s1.len().cmp(&s2.len());
            }
        }
        let o = s1.cmp(s2);
        if o != Ordering::Equal {
            return o;
        }
        i = ei;
        j = ej;
    }
    let (ra, rb) = (&x[i.min(x.len())..], &y[j.min(y.len())..]);
    if ra.is_empty() && rb.is_empty() {
        return Ordering::Equal;
    }
    // The final showdown: a leftover letter segment never beats nothing.
    let a_alpha = ra.first().is_some_and(|c| c.is_ascii_alphabetic());
    let b_alpha = rb.first().is_some_and(|c| c.is_ascii_alphabetic());
    if (ra.is_empty() && !b_alpha) || a_alpha {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

// ---- reading the cache --------------------------------------------------------

/// A package file's identity, read from its name:
/// `<name>-<pkgver>-<pkgrel>-<arch>.pkg.tar.<ext>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkgFile {
    pub file: String,
    pub name: String,
    /// `[epoch:]pkgver-pkgrel`
    pub version: String,
    pub size: u64,
    /// The matching `.sig`, when there is one: (file name, size).
    pub sig: Option<(String, u64)>,
}

/// Parse a cache file name. `None` for anything that is not a package file
/// (signatures, partial downloads, leftover download folders).
pub fn parse_file_name(file: &str) -> Option<(String, String)> {
    let stem_end = file.rfind(".pkg.tar")?;
    let rest = &file[stem_end + ".pkg.tar".len()..];
    if rest.ends_with(".sig") || rest.ends_with(".part") {
        return None;
    }
    let stem = &file[..stem_end];
    let mut parts = stem.rsplitn(4, '-');
    let _arch = parts.next()?;
    let pkgrel = parts.next()?;
    let pkgver = parts.next()?;
    let name = parts.next()?;
    if name.is_empty() || pkgver.is_empty() || pkgrel.is_empty() {
        return None;
    }
    Some((name.to_string(), format!("{}-{}", pkgver, pkgrel)))
}

/// Group a directory listing, `(file name, size)`, into package files with
/// their signatures attached. Signatures without a package are ignored here:
/// they are neither kept nor removed by this pass.
pub fn read_listing(listing: &[(String, u64)]) -> Vec<PkgFile> {
    let sizes: HashMap<&str, u64> = listing.iter().map(|(n, s)| (n.as_str(), *s)).collect();
    let mut out = Vec::new();
    for (file, size) in listing {
        if let Some((name, version)) = parse_file_name(file) {
            let sig_name = format!("{}.sig", file);
            let sig = sizes.get(sig_name.as_str()).map(|s| (sig_name.clone(), *s));
            out.push(PkgFile { file: file.clone(), name, version, size: *size, sig });
        }
    }
    out
}

// ---- deciding --------------------------------------------------------------------

/// Why a file is going.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// An older version beyond the tier's keep count. Carries the tier digit.
    Older(u8),
    /// The package is no longer installed.
    NotInstalled,
}

#[derive(Debug, Default)]
pub struct Plan {
    pub remove: Vec<(PkgFile, Reason)>,
    pub keep: Vec<PkgFile>,
}

impl Plan {
    /// Bytes freed by removing a file and its signature.
    pub fn bytes(f: &PkgFile) -> u64 {
        f.size + f.sig.as_ref().map_or(0, |(_, s)| *s)
    }
}

/// Decide, per package. `installed` maps name → installed version;
/// `tier_of` gives a name's tier digit and `keep_for` how many versions that
/// tier keeps (at least 1: the installed one).
pub fn plan<T, K>(files: Vec<PkgFile>, installed: &HashMap<String, String>, tier_of: T, keep_for: K) -> Plan
where
    T: Fn(&str) -> u8,
    K: Fn(u8) -> usize,
{
    let mut by_name: HashMap<String, Vec<PkgFile>> = HashMap::new();
    for f in files {
        by_name.entry(f.name.clone()).or_default().push(f);
    }
    let mut names: Vec<String> = by_name.keys().cloned().collect();
    names.sort();

    let mut p = Plan::default();
    for name in names {
        let mut group = by_name.remove(&name).unwrap_or_default();
        let Some(iv) = installed.get(&name) else {
            for f in group {
                p.remove.push((f, Reason::NotInstalled));
            }
            continue;
        };
        let tier = tier_of(&name);
        let keep_older = keep_for(tier).max(1) - 1;
        // Newest first.
        group.sort_by(|a, b| vercmp(&b.version, &a.version).then_with(|| a.file.cmp(&b.file)));
        let mut older_versions_kept: Vec<String> = Vec::new();
        for f in group {
            match vercmp(&f.version, iv) {
                Ordering::Greater | Ordering::Equal => p.keep.push(f),
                Ordering::Less => {
                    // Two files of one version (e.g. two architectures) share a slot.
                    if older_versions_kept.contains(&f.version) {
                        p.keep.push(f);
                    } else if older_versions_kept.len() < keep_older {
                        older_versions_kept.push(f.version.clone());
                        p.keep.push(f);
                    } else {
                        p.remove.push((f, Reason::Older(tier)));
                    }
                }
            }
        }
    }
    p
}

/// A leftover `download-<6 letters>` folder from an interrupted download, old
/// enough that no running pacman can still own it.
pub fn is_stale_download_dir(name: &str, age_secs: u64) -> bool {
    const DAY: u64 = 86_400;
    name.len() == "download-".len() + 6
        && name.starts_with("download-")
        && name["download-".len()..].chars().all(|c| c.is_ascii_alphanumeric())
        && age_secs >= DAY
}

// ---- IO ----------------------------------------------------------------------------

/// The cache directories pacman uses: every `CacheDir` in pacman.conf, or
/// pacman's default when there is none.
pub fn cache_dirs(pacman_conf: &str) -> Vec<PathBuf> {
    let text = fs::read_to_string(pacman_conf).unwrap_or_default();
    let dirs: Vec<PathBuf> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == "CacheDir").then(|| v.split_whitespace().map(PathBuf::from).collect::<Vec<_>>())
        })
        .flatten()
        .collect();
    if dirs.is_empty() { vec![PathBuf::from("/var/cache/pacman/pkg/")] } else { dirs }
}

/// What one cache directory holds.
pub struct Listing {
    pub files: Vec<(String, u64)>,
    /// Stale leftover download folders.
    pub stale_dirs: Vec<String>,
}

pub fn list_dir(dir: &Path) -> Listing {
    let mut files = Vec::new();
    let mut stale_dirs = Vec::new();
    let now = std::time::SystemTime::now();
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let Ok(meta) = e.metadata() else { continue };
            let name = e.file_name().to_string_lossy().to_string();
            if meta.is_file() {
                files.push((name, meta.len()));
            } else if meta.is_dir() {
                let age = meta.modified().ok()
                    .and_then(|m| now.duration_since(m).ok())
                    .map_or(0, |d| d.as_secs());
                if is_stale_download_dir(&name, age) {
                    stale_dirs.push(name);
                }
            }
        }
    }
    Listing { files, stale_dirs }
}

/// `true` while pacman holds its database lock — never clean then.
pub fn pacman_is_running() -> bool {
    Path::new("/var/lib/pacman/db.lck").exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use Ordering::*;

    #[test]
    fn vercmp_matches_pacman_on_the_hard_cases() {
        // Every row checked against /usr/bin/vercmp on 30 Sep 2026.
        let cases = [
            ("1.0", "1.0a", Greater),
            ("1.0a", "1.0", Less),
            ("1:1.0", "2.0", Greater),
            ("1.0-1", "1.0-2", Less),
            ("2.44+r24+g16be1518495f-1", "2.44+r50+g1848099f063e-1", Less),
            ("1.0.r23.gabc", "1.0", Greater),
            ("7.2.7.zen1-1", "7.2.10.zen1-1", Less),
            ("1.0_rc1", "1.0", Greater),
            ("1.0", "1.0.0", Less),
            ("2.2.1-2", "2.2.1.r23.gdee3b387-1", Less),
            ("1:26.1.1-2", "1:26.2.3-1", Less),
            ("1.0", "1.0", Equal),
            ("1.0-1", "1.0", Equal),
            ("0:1.0", "1.0", Equal),
            ("1.001", "1.1", Equal),
        ];
        for (a, b, want) in cases {
            assert_eq!(vercmp(a, b), want, "vercmp({a}, {b})");
            assert_eq!(vercmp(b, a), want.reverse(), "vercmp({b}, {a})");
        }
    }

    /// Runs only where pacman's `vercmp` exists: every version pair from a
    /// real package cache must order exactly as pacman orders it.
    #[test]
    #[ignore = "live: compares against /usr/bin/vercmp over this machine's cache"]
    fn vercmp_agrees_with_pacman_over_the_real_cache() {
        let files = read_listing(&list_dir(Path::new("/var/cache/pacman/pkg")).files);
        let mut by_name: HashMap<&str, Vec<&str>> = HashMap::new();
        for f in &files { by_name.entry(&f.name).or_default().push(&f.version); }
        let mut checked = 0;
        for vs in by_name.values() {
            for w in vs.windows(2) {
                let out = std::process::Command::new("vercmp").args([w[0], w[1]]).output().unwrap();
                let n: i32 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap();
                assert_eq!(vercmp(w[0], w[1]), n.cmp(&0), "vercmp {} {}", w[0], w[1]);
                checked += 1;
            }
        }
        assert!(checked > 100, "only {checked} pairs — not a real cache");
    }

    #[test]
    fn file_names_are_read_from_the_right() {
        assert_eq!(parse_file_name("linux-zen-headers-7.2.7.zen1-1-x86_64.pkg.tar.zst"),
                   Some(("linux-zen-headers".into(), "7.2.7.zen1-1".into())));
        assert_eq!(parse_file_name("mesa-1:26.1.1-2-x86_64.pkg.tar.zst"),
                   Some(("mesa".into(), "1:26.1.1-2".into())));
        assert_eq!(parse_file_name("ttf-font-2.0-3-any.pkg.tar.xz"), Some(("ttf-font".into(), "2.0-3".into())));
        assert_eq!(parse_file_name("mesa-1:26.1.1-2-x86_64.pkg.tar.zst.sig"), None);
        assert_eq!(parse_file_name("mesa-1:26.1.1-2-x86_64.pkg.tar.zst.part"), None);
        assert_eq!(parse_file_name("download-0Fg8wG"), None);
        assert_eq!(parse_file_name("broken.pkg.tar.zst"), None);
    }

    fn listing(names: &[&str]) -> Vec<(String, u64)> {
        names.iter().map(|n| (n.to_string(), 100)).collect()
    }

    fn installed(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(n, v)| (n.to_string(), v.to_string())).collect()
    }

    fn keep_default(t: u8) -> usize { match t { 1 => 3, 2 => 2, _ => 1 } }

    fn removed(p: &Plan) -> Vec<&str> {
        let mut v: Vec<&str> = p.remove.iter().map(|(f, _)| f.file.as_str()).collect();
        v.sort();
        v
    }

    #[test]
    fn tier_1_keeps_the_installed_version_and_two_older() {
        let files = read_listing(&listing(&[
            "linux-zen-7.0.5.zen1-1-x86_64.pkg.tar.zst",
            "linux-zen-7.1.9.zen1-2-x86_64.pkg.tar.zst",
            "linux-zen-7.2.4.zen2-1-x86_64.pkg.tar.zst",
            "linux-zen-7.2.7.zen1-1-x86_64.pkg.tar.zst",
            "linux-zen-6.19.1.zen1-1-x86_64.pkg.tar.zst",
        ]));
        let p = plan(files, &installed(&[("linux-zen", "7.2.7.zen1-1")]), |_| 1, keep_default);
        assert_eq!(removed(&p), vec!["linux-zen-6.19.1.zen1-1-x86_64.pkg.tar.zst", "linux-zen-7.0.5.zen1-1-x86_64.pkg.tar.zst"]);
        assert_eq!(p.keep.len(), 3);
    }

    #[test]
    fn tier_3_keeps_only_the_installed_version_and_the_signature_goes_with_its_file() {
        let files = read_listing(&listing(&[
            "7zip-26.01-1-x86_64.pkg.tar.zst",
            "7zip-26.01-1-x86_64.pkg.tar.zst.sig",
            "7zip-26.02-1-x86_64.pkg.tar.zst",
        ]));
        let p = plan(files, &installed(&[("7zip", "26.02-1")]), |_| 3, keep_default);
        assert_eq!(removed(&p), vec!["7zip-26.01-1-x86_64.pkg.tar.zst"]);
        assert_eq!(p.remove[0].0.sig.as_ref().unwrap().0, "7zip-26.01-1-x86_64.pkg.tar.zst.sig");
        assert_eq!(Plan::bytes(&p.remove[0].0), 200);
    }

    #[test]
    fn a_held_package_keeps_what_it_runs_and_a_downloaded_newer_version_stays() {
        // linux-lts is held at 6.18.29; a 6.18.54 download is waiting to be used.
        let files = read_listing(&listing(&[
            "linux-lts-6.18.20-1-x86_64.pkg.tar.zst",
            "linux-lts-6.18.25-1-x86_64.pkg.tar.zst",
            "linux-lts-6.18.28-1-x86_64.pkg.tar.zst",
            "linux-lts-6.18.29-1-x86_64.pkg.tar.zst",
            "linux-lts-6.18.54-1-x86_64.pkg.tar.zst",
        ]));
        let p = plan(files, &installed(&[("linux-lts", "6.18.29-1")]), |_| 1, keep_default);
        assert_eq!(removed(&p), vec!["linux-lts-6.18.20-1-x86_64.pkg.tar.zst"]);
    }

    #[test]
    fn an_uninstalled_package_goes_entirely() {
        let files = read_listing(&listing(&["postman-bin-11.0-1-x86_64.pkg.tar.zst", "postman-bin-11.1-1-x86_64.pkg.tar.zst"]));
        let p = plan(files, &installed(&[]), |_| 3, keep_default);
        assert_eq!(p.remove.len(), 2);
        assert!(p.remove.iter().all(|(_, r)| *r == Reason::NotInstalled));
    }

    #[test]
    fn the_installed_file_is_kept_even_when_the_tier_keeps_one() {
        let files = read_listing(&listing(&["vim-9.1-1-x86_64.pkg.tar.zst"]));
        let p = plan(files, &installed(&[("vim", "9.1-1")]), |_| 3, |_| 0);
        assert!(p.remove.is_empty(), "keep 0 is treated as keep 1");
    }

    #[test]
    fn two_architectures_of_one_version_share_a_slot() {
        let files = read_listing(&listing(&[
            "foo-1.0-1-any.pkg.tar.zst", "foo-1.0-1-x86_64.pkg.tar.zst",
            "foo-2.0-1-x86_64.pkg.tar.zst", "foo-0.9-1-x86_64.pkg.tar.zst",
        ]));
        let p = plan(files, &installed(&[("foo", "2.0-1")]), |_| 2, keep_default);
        assert_eq!(removed(&p), vec!["foo-0.9-1-x86_64.pkg.tar.zst"]);
    }

    #[test]
    fn only_old_download_folders_count_as_stale() {
        assert!(is_stale_download_dir("download-0Fg8wG", 2 * 86_400));
        assert!(!is_stale_download_dir("download-0Fg8wG", 60), "might belong to a running pacman");
        assert!(!is_stale_download_dir("download-../../etc", 9_999_999));
        assert!(!is_stale_download_dir("mydir", 9_999_999));
    }

    #[test]
    fn cache_dirs_come_from_pacman_conf_or_the_default() {
        let d = std::env::temp_dir().join(format!("nog-cache-test-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let conf = d.join("pacman.conf");
        std::fs::write(&conf, "[options]\n#CacheDir = /nope\nCacheDir = /a/ /b/\n").unwrap();
        assert_eq!(cache_dirs(conf.to_str().unwrap()), vec![PathBuf::from("/a/"), PathBuf::from("/b/")]);
        std::fs::write(&conf, "[options]\n").unwrap();
        assert_eq!(cache_dirs(conf.to_str().unwrap()), vec![PathBuf::from("/var/cache/pacman/pkg/")]);
        std::fs::remove_dir_all(&d).ok();
    }
}
