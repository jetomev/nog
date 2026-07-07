// sync_db.rs — read pacman's sync databases and extract package metadata
//
// Pacman stores per-repo metadata at /var/lib/pacman/sync/<repo>.db as
// compressed tar archives. The compression format varies by repo:
//   - Official Arch repos (core, extra, multilib) use gzip
//   - Chaotic-AUR and some third-party repos use zstd
//   - Pacman itself auto-detects via magic bytes and supports both
//
// We do the same: read the first four bytes to identify the format, then
// wrap the right decoder around the file. Everything downstream (tar
// iteration, desc parsing) is compression-agnostic.
//
// Each package directory inside the archive contains a `desc` file with
// fields like:
//
//   %NAME%
//   firefox
//
//   %BASE%
//   firefox
//
//   %VERSION%
//   149.0.2-1
//
//   %BUILDDATE%
//   1775526658
//
// We walk every enabled repo and build a map of package name to its full
// metadata (build-date + pkgbase). v1.0.4 added the pkgbase field to power
// split-PKGBUILD sibling coupling (Layer A of the pkgbase-coupling fix —
// the bug surfaced 2026-05-25 by the pipewire family).

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use flate2::read::GzDecoder;
use tar::Archive;

const SYNC_DB_DIR: &str = "/var/lib/pacman/sync";
const PACMAN_CONF: &str = "/etc/pacman.conf";

/// Per-package metadata extracted from a sync DB `desc` file.
///
/// `pkgbase` is the value of the `%BASE%` field, identifying the PKGBUILD a
/// package was built from. Multiple split packages share the same pkgbase
/// (e.g., `pipewire`, `libpipewire`, `pipewire-pulse`, ... all have
/// `pkgbase = pipewire`). Arch enforces lockstep for these via `=` version
/// dependencies — that's what the v1.0.4 pkgbase coupling rule leverages.
///
/// `pkgbase` is `None` when `%BASE%` is missing from `desc` — defensive
/// fallback; in current Arch every desc has it.
#[derive(Debug, Clone)]
pub struct PackageDesc {
    pub builddate: u64,
    pub pkgbase: Option<String>,
    /// The `%VERSION%` field (`epoch:pkgver-pkgrel`). v1.0.5 added this to
    /// power the candidate-version guard in hold evaluation: a build date is
    /// only meaningful for the version it belongs to. `None` when `%VERSION%`
    /// is missing — defensive fallback; every real desc has it.
    pub version: Option<String>,
}

/// Load the full package map (name → PackageDesc) from every enabled sync
/// database. **Cached via `OnceLock`** — first call walks the DBs (~18k
/// packages on a typical Arch install, gzip + zstd decode); subsequent
/// callers within the same process reuse the cached map. This keeps
/// `nog update` from double-walking when both `load_build_dates()` (for
/// hold evaluation) and the new `PkgbaseIndex` (for sibling coupling) need
/// the same data.
pub fn load_packages() -> &'static HashMap<String, PackageDesc> {
    static CACHED: OnceLock<HashMap<String, PackageDesc>> = OnceLock::new();
    CACHED.get_or_init(walk_all_repos)
}

/// Back-compat wrapper: derives the name → build-date map from `load_packages`.
/// Existing callers (`commands::update`, `main::debug_*`) get the same data
/// they had pre-v1.0.4 with no behavior change. New code should call
/// `load_packages` directly to also access pkgbase.
pub fn load_build_dates() -> HashMap<String, u64> {
    load_packages()
        .iter()
        .map(|(name, desc)| (name.clone(), desc.builddate))
        .collect()
}

/// Load package metadata from the sync DBs `checkupdates` just refreshed.
///
/// `checkupdates` (pacman-contrib) syncs the enabled repos into a private
/// dbpath — `$CHECKUPDATES_DB`, defaulting to `${TMPDIR:-/tmp}/checkup-db-<uid>/`
/// — as an unprivileged user, then diffs against the local DB. That private
/// copy is the database snapshot that PRODUCED the pending-update list, while
/// `/var/lib/pacman/sync` only refreshes when root syncs — for `nog update`,
/// during the handoff AFTER hold evaluation. Hold windows must be dated from
/// the same snapshot as the candidates (the 2026-07-06 stale-DB finding), so
/// `nog update` prefers this loader and falls back to `load_packages` only
/// when the directory is missing.
///
/// Returns `None` when the checkupdates dbpath (or its `sync/` subdir)
/// doesn't exist. Not cached — called at most once per `nog update`, right
/// after `checkupdates` has refreshed the directory.
pub fn load_fresh_packages() -> Option<HashMap<String, PackageDesc>> {
    let dir = checkupdates_sync_dir()?;
    if !dir.is_dir() {
        return None;
    }
    Some(walk_repos_in(&dir))
}

/// Resolve the `sync/` subdir of checkupdates' private dbpath, mirroring the
/// resolution in the checkupdates script itself: `$CHECKUPDATES_DB` if set,
/// else `${TMPDIR:-/tmp}/checkup-db-<uid>`.
fn checkupdates_sync_dir() -> Option<PathBuf> {
    let base = match std::env::var_os("CHECKUPDATES_DB") {
        Some(p) => PathBuf::from(p),
        None => {
            let tmp = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(tmp).join(format!("checkup-db-{}", current_uid()?))
        }
    };
    Some(base.join("sync"))
}

/// Numeric uid via `id -u` — matches the shell `$UID` checkupdates uses,
/// without adding a libc dependency for one syscall.
fn current_uid() -> Option<String> {
    let out = std::process::Command::new("id").arg("-u").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let uid = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if uid.is_empty() { None } else { Some(uid) }
}

fn walk_all_repos() -> HashMap<String, PackageDesc> {
    walk_repos_in(Path::new(SYNC_DB_DIR))
}

fn walk_repos_in(sync_dir: &Path) -> HashMap<String, PackageDesc> {
    let mut pkgs: HashMap<String, PackageDesc> = HashMap::new();

    let repos = enabled_repos().unwrap_or_else(|e| {
        eprintln!("nog warning: could not read {}: {}", PACMAN_CONF, e);
        eprintln!(
            "nog warning: falling back to scanning every *.db file in {}",
            sync_dir.display()
        );
        fallback_repos_from_disk(sync_dir)
    });

    for repo in repos {
        let db_path = sync_dir.join(format!("{}.db", repo));
        if !db_path.exists() {
            // Repo is enabled in pacman.conf but we haven't `pacman -Sy`'d yet,
            // or the file is genuinely missing. Not fatal — just skip it.
            continue;
        }

        match read_repo(&db_path) {
            Ok(repo_pkgs) => {
                for (name, desc) in repo_pkgs {
                    pkgs.entry(name).or_insert(desc);
                }
            }
            Err(e) => {
                eprintln!(
                    "nog warning: could not read sync DB for '{}': {}",
                    repo, e
                );
            }
        }
    }

    pkgs
}

/// Read the list of enabled repositories from pacman.conf, preserving order.
/// A repository is declared by a section header like `[extra]` — we ignore
/// the `[options]` section since it's not a repo.
fn enabled_repos() -> Result<Vec<String>, String> {
    let contents = fs::read_to_string(PACMAN_CONF)
        .map_err(|e| format!("{}", e))?;

    let mut repos = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let name = &trimmed[1..trimmed.len() - 1];
            if !name.is_empty() && name != "options" {
                repos.push(name.to_string());
            }
        }
    }
    Ok(repos)
}

/// Fallback: scan the sync directory directly when pacman.conf is unreadable.
/// Less accurate because we lose repo-priority ordering, but better than
/// failing entirely.
fn fallback_repos_from_disk(sync_dir: &Path) -> Vec<String> {
    let mut repos = Vec::new();
    if let Ok(entries) = fs::read_dir(sync_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if let Some(repo) = name.strip_suffix(".db") {
                    repos.push(repo.to_string());
                }
            }
        }
    }
    repos
}

/// The compression format used for a particular .db file.
enum Compression {
    Gzip,
    Zstd,
}

/// Sniff the compression format by peeking at the first four magic bytes.
/// Gzip is `1f 8b`, Zstd is `28 b5 2f fd`.
fn detect_compression(file: &mut File) -> Result<Compression, String> {
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)
        .map_err(|e| format!("could not read magic bytes: {}", e))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|e| format!("could not rewind after sniff: {}", e))?;

    if magic[0] == 0x1f && magic[1] == 0x8b {
        Ok(Compression::Gzip)
    } else if magic == [0x28, 0xb5, 0x2f, 0xfd] {
        Ok(Compression::Zstd)
    } else {
        Err(format!(
            "unrecognized compression (magic bytes: {:02x} {:02x} {:02x} {:02x})",
            magic[0], magic[1], magic[2], magic[3]
        ))
    }
}

/// Read one repo's sync database and return a map of package-name -> PackageDesc.
/// Auto-detects gzip or zstd based on the file's magic bytes.
fn read_repo(db_path: &Path) -> Result<HashMap<String, PackageDesc>, String> {
    let mut file = File::open(db_path)
        .map_err(|e| format!("open failed: {}", e))?;

    let compression = detect_compression(&mut file)?;

    // Wrap the file in the appropriate decoder, box it as a trait object so
    // the rest of the function doesn't care which format we're using.
    let decoder: Box<dyn Read> = match compression {
        Compression::Gzip => Box::new(GzDecoder::new(file)),
        Compression::Zstd => Box::new(
            zstd::stream::Decoder::new(file)
                .map_err(|e| format!("zstd decoder init failed: {}", e))?,
        ),
    };

    let mut archive = Archive::new(decoder);

    let mut pkgs: HashMap<String, PackageDesc> = HashMap::new();

    let entries = archive.entries()
        .map_err(|e| format!("archive read failed: {}", e))?;

    for entry in entries {
        let mut entry = entry.map_err(|e| format!("entry read failed: {}", e))?;

        // We only care about `desc` files.
        let path_in_tar = entry.path()
            .map_err(|e| format!("entry path unreadable: {}", e))?
            .to_path_buf();

        let file_name = match path_in_tar.file_name().and_then(|n| n.to_str()) {
            Some(s) => s,
            None => continue,
        };
        if file_name != "desc" {
            continue;
        }

        // The parent directory of `desc` is `<n>-<pkgver>-<pkgrel>`.
        // We read the desc contents to get %NAME%, %BUILDDATE%, %BASE% directly,
        // which is safer than trying to parse name out of the folder string.
        let mut contents = String::new();
        if entry.read_to_string(&mut contents).is_err() {
            // Desc files are always UTF-8 text; if one isn't, something is
            // genuinely wrong with this DB entry — skip it and move on.
            continue;
        }

        if let Some((name, desc)) = parse_desc(&contents) {
            pkgs.insert(name, desc);
        }
    }

    Ok(pkgs)
}

/// Parse the key fields we care about out of a desc file.
///
/// Format is a series of `%KEY%` lines followed by one or more value lines,
/// separated by blank lines. We read `%NAME%`, `%BUILDDATE%`, `%BASE%`, and
/// `%VERSION%`.
///
/// Returns `None` if `%NAME%` or `%BUILDDATE%` is missing (we need both for
/// any useful classification). `%BASE%` and `%VERSION%` are optional —
/// defensive fallback for non-standard desc files; in practice every Arch
/// package has both.
fn parse_desc(contents: &str) -> Option<(String, PackageDesc)> {
    let mut name: Option<String> = None;
    let mut date: Option<u64> = None;
    let mut pkgbase: Option<String> = None;
    let mut version: Option<String> = None;

    let mut lines = contents.lines();
    while let Some(line) = lines.next() {
        match line.trim() {
            "%NAME%" => {
                if let Some(v) = lines.next() {
                    name = Some(v.trim().to_string());
                }
            }
            "%VERSION%" => {
                if let Some(v) = lines.next() {
                    let trimmed = v.trim();
                    if !trimmed.is_empty() {
                        version = Some(trimmed.to_string());
                    }
                }
            }
            "%BUILDDATE%" => {
                if let Some(v) = lines.next() {
                    if let Ok(n) = v.trim().parse::<u64>() {
                        date = Some(n);
                    }
                }
            }
            "%BASE%" => {
                if let Some(v) = lines.next() {
                    let trimmed = v.trim();
                    if !trimmed.is_empty() {
                        pkgbase = Some(trimmed.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    match (name, date) {
        (Some(n), Some(d)) => Some((n, PackageDesc { builddate: d, pkgbase, version })),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESC: &str = "\
%NAME%
lib32-brotli

%VERSION%
1.2.0-2

%BASE%
lib32-brotli

%BUILDDATE%
1783600000
";

    #[test]
    fn parse_desc_reads_all_fields() {
        let (name, desc) = parse_desc(DESC).expect("desc should parse");
        assert_eq!(name, "lib32-brotli");
        assert_eq!(desc.builddate, 1783600000);
        assert_eq!(desc.pkgbase.as_deref(), Some("lib32-brotli"));
        assert_eq!(desc.version.as_deref(), Some("1.2.0-2"));
    }

    #[test]
    fn parse_desc_tolerates_missing_version_and_base() {
        let contents = "%NAME%\nghost\n\n%BUILDDATE%\n1000\n";
        let (name, desc) = parse_desc(contents).expect("desc should parse");
        assert_eq!(name, "ghost");
        assert_eq!(desc.builddate, 1000);
        assert_eq!(desc.pkgbase, None);
        assert_eq!(desc.version, None);
    }

    #[test]
    fn parse_desc_requires_name_and_builddate() {
        assert!(parse_desc("%NAME%\nonly-name\n").is_none());
        assert!(parse_desc("%BUILDDATE%\n1000\n").is_none());
    }
}
