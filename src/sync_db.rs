// sync_db.rs — read pacman's sync databases and extract build dates
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
//   %VERSION%
//   149.0.2-1
//
//   %BUILDDATE%
//   1775526658
//
// We walk every enabled repo and build a HashMap of package name to
// build-date Unix timestamp. This is the foundation of the date-based hold
// system — everything downstream reads from this map.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use tar::Archive;

const SYNC_DB_DIR: &str = "/var/lib/pacman/sync";
const PACMAN_CONF: &str = "/etc/pacman.conf";

/// Load a map of package-name -> build-date Unix timestamp by walking every
/// enabled sync database on disk. On repeated package names across repos,
/// the first repo (in pacman.conf order) wins — matching pacman's own
/// resolution rules.
pub fn load_build_dates() -> HashMap<String, u64> {
    let mut dates: HashMap<String, u64> = HashMap::new();

    let repos = enabled_repos().unwrap_or_else(|e| {
        eprintln!("nog warning: could not read {}: {}", PACMAN_CONF, e);
        eprintln!("nog warning: falling back to scanning every *.db file in {}", SYNC_DB_DIR);
        fallback_repos_from_disk()
    });

    for repo in repos {
        let db_path = PathBuf::from(SYNC_DB_DIR).join(format!("{}.db", repo));
        if !db_path.exists() {
            // Repo is enabled in pacman.conf but we haven't `pacman -Sy`'d yet,
            // or the file is genuinely missing. Not fatal — just skip it.
            continue;
        }

        match read_repo_dates(&db_path) {
            Ok(repo_dates) => {
                for (name, date) in repo_dates {
                    dates.entry(name).or_insert(date);
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

    dates
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
fn fallback_repos_from_disk() -> Vec<String> {
    let mut repos = Vec::new();
    if let Ok(entries) = fs::read_dir(SYNC_DB_DIR) {
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

/// Read one repo's sync database and return a map of package-name -> build-date.
/// Auto-detects gzip or zstd based on the file's magic bytes.
fn read_repo_dates(db_path: &Path) -> Result<HashMap<String, u64>, String> {
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

    let mut dates: HashMap<String, u64> = HashMap::new();

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
        // We read the desc contents to get %NAME% and %BUILDDATE% directly,
        // which is safer than trying to parse name out of the folder string.
        let mut contents = String::new();
        if entry.read_to_string(&mut contents).is_err() {
            // Desc files are always UTF-8 text; if one isn't, something is
            // genuinely wrong with this DB entry — skip it and move on.
            continue;
        }

        if let Some((name, date)) = parse_desc(&contents) {
            dates.insert(name, date);
        }
    }

    Ok(dates)
}

/// Parse the key fields we care about out of a desc file.
/// Format is a series of `%KEY%` lines followed by one or more value lines,
/// separated by blank lines. We only read %NAME% and %BUILDDATE%.
fn parse_desc(contents: &str) -> Option<(String, u64)> {
    let mut name: Option<String> = None;
    let mut date: Option<u64> = None;

    let mut lines = contents.lines();
    while let Some(line) = lines.next() {
        match line.trim() {
            "%NAME%" => {
                if let Some(v) = lines.next() {
                    name = Some(v.trim().to_string());
                }
            }
            "%BUILDDATE%" => {
                if let Some(v) = lines.next() {
                    if let Ok(n) = v.trim().parse::<u64>() {
                        date = Some(n);
                    }
                }
            }
            _ => {}
        }
    }

    match (name, date) {
        (Some(n), Some(d)) => Some((n, d)),
        _ => None,
    }
}