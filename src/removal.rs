//! v1.9.0 (F-13, #49): what a removal takes along, and who still uses it.
//!
//! `pacman -Rs` removes the packages named plus every dependency nothing else
//! *requires*. A package another installed app uses only *optionally* counts
//! as unneeded, so it goes too, and nothing says so: on 2026-10-08 removing
//! Elisa would have taken `vlc-plugin-ffmpeg`, and VLC would have lost most of
//! its video formats. Before the removal nog now reads pacman's own dry run,
//! finds the packages in it that another staying package lists as optional,
//! says so in plain words, and offers to keep each one — by marking it
//! installed on purpose (`pacman -D --asexplicit`), which is also what
//! `nog keep` does on its own.

use std::process::{Command, ExitStatus, Stdio};

use crate::machine::Installed;

/// What `pacman -Rs <named>` would remove — pacman's own dry run (`-Rsp`), so
/// nog never guesses at pacman's rules. Needs no root: it only reads the local
/// database. `Err` carries pacman's words (a name that is not installed, a
/// package something else requires…), for nog to show before anything happens.
pub fn sweep(named: &[String]) -> Result<Vec<String>, String> {
    let output = Command::new("pacman")
        .args(["-Rsp", "--print-format", "%n", "--"])
        .args(named)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("failed to launch pacman: {}", e))?;
    if !output.status.success() {
        let msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if msg.is_empty() {
            format!("pacman -Rsp exited with status {}", output.status)
        } else {
            msg.lines().map(|l| l.trim_start_matches("error: ").to_string())
                .collect::<Vec<_>>().join("\n")
        });
    }
    Ok(parse_sweep(&String::from_utf8_lossy(&output.stdout)))
}

/// One name per line; pacman prints nothing else with `--print-format %n`.
pub fn parse_sweep(stdout: &str) -> Vec<String> {
    stdout.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect()
}

/// A package the removal would take along that another installed package,
/// one that stays, still uses optionally — with who uses it and what for.
#[derive(Debug, Clone, PartialEq)]
pub struct OptionalUse {
    pub package: String,
    /// (the package that uses it, pacman's reason — "ffmpeg plugin")
    pub users: Vec<(String, String)>,
}

/// Pure. Which packages in `sweep` that were not asked for by name are listed
/// as optional by an installed package outside the sweep. Matched by name or
/// by something the swept package provides (an optional dependency can name a
/// virtual package). The packages named are never reported: the person asked
/// for those to go.
pub fn optional_uses(sweep: &[String], named: &[String], installed: &[Installed]) -> Vec<OptionalUse> {
    let going = |n: &str| sweep.iter().any(|s| s == n);
    let mut out: Vec<OptionalUse> = Vec::new();
    for p in sweep {
        if named.iter().any(|n| n == p) {
            continue;
        }
        let provides: Vec<&str> = installed.iter().find(|i| &i.name == p)
            .map(|i| i.provides.iter().map(|s| s.as_str()).collect()).unwrap_or_default();
        let mut users: Vec<(String, String)> = installed.iter()
            .filter(|i| !going(&i.name))
            .flat_map(|i| i.optdepends.iter()
                .filter(|(dep, _)| dep == p || provides.contains(&dep.as_str()))
                .map(move |(_, why)| (i.name.clone(), why.clone())))
            .collect();
        users.sort();
        users.dedup();
        if !users.is_empty() {
            out.push(OptionalUse { package: p.clone(), users });
        }
    }
    out
}

/// The words for each package that goes along but is still used: one line
/// per user, the reason in pacman's own words when it gave one.
pub fn use_lines(uses: &[OptionalUse]) -> Vec<String> {
    let mut lines = Vec::new();
    for u in uses {
        for (user, why) in &u.users {
            lines.push(if why.is_empty() {
                format!("{} goes too, but {} uses it (optional).", u.package, user)
            } else {
                format!("{} goes too, but {} uses it (optional: {}).", u.package, user, why)
            });
        }
    }
    lines
}

/// Mark packages as installed on purpose (`pacman -D --asexplicit`): a removal
/// of something else never takes them along again. Root is needed to write
/// the local database, so this goes through sudo like every change nog makes.
pub fn mark_explicit(names: &[String]) -> ExitStatus {
    crate::machine::sudo()
        .args(["pacman", "-D", "--asexplicit", "--"])
        .args(names)
        .status()
        .unwrap_or_else(|e| panic!("nog: failed to launch sudo pacman -D: {}", e))
}

/// Which of these names are installed packages (`pacman -Q`), and which are not.
pub fn split_installed(names: &[String]) -> (Vec<String>, Vec<String>) {
    let installed = crate::pacman::installed_versions(names);
    names.iter().cloned().partition(|n| installed.contains_key(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, provides: &[&str], optdepends: &[(&str, &str)]) -> Installed {
        Installed {
            name: name.into(),
            provides: provides.iter().map(|s| s.to_string()).collect(),
            optdepends: optdepends.iter().map(|(d, w)| (d.to_string(), w.to_string())).collect(),
            ..Default::default()
        }
    }

    fn names(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }

    /// The 2026-10-08 case, in the failing direction: Elisa's removal takes
    /// vlc-plugin-ffmpeg; VLC stays and lists it as optional. It MUST be reported.
    #[test]
    fn a_swept_package_another_app_uses_optionally_is_reported() {
        let installed = vec![
            pkg("elisa", &[], &[]),
            pkg("vlc-plugin-ffmpeg", &[], &[]),
            pkg("vlc", &[], &[("vlc-plugin-ffmpeg", "ffmpeg plugin"), ("vlc-gui-ncurses", "for ncurses TUI")]),
        ];
        let sweep = names(&["elisa", "vlc-plugin-ffmpeg"]);
        let got = optional_uses(&sweep, &names(&["elisa"]), &installed);
        assert_eq!(got, vec![OptionalUse {
            package: "vlc-plugin-ffmpeg".into(),
            users: vec![("vlc".into(), "ffmpeg plugin".into())],
        }]);
        assert_eq!(use_lines(&got),
            vec!["vlc-plugin-ffmpeg goes too, but vlc uses it (optional: ffmpeg plugin)."]);
    }

    #[test]
    fn a_package_asked_for_by_name_is_never_reported() {
        // The person asked for vlc-plugin-ffmpeg itself to go: VLC's optional
        // use is not a surprise, it is the request.
        let installed = vec![
            pkg("vlc-plugin-ffmpeg", &[], &[]),
            pkg("vlc", &[], &[("vlc-plugin-ffmpeg", "ffmpeg plugin")]),
        ];
        let sweep = names(&["vlc-plugin-ffmpeg"]);
        assert!(optional_uses(&sweep, &sweep, &installed).is_empty());
    }

    #[test]
    fn a_user_that_goes_in_the_same_removal_does_not_count() {
        // Removing elisa AND vlc: nothing stays to use the plugin.
        let installed = vec![
            pkg("elisa", &[], &[]),
            pkg("vlc-plugin-ffmpeg", &[], &[]),
            pkg("vlc", &[], &[("vlc-plugin-ffmpeg", "ffmpeg plugin")]),
        ];
        let sweep = names(&["elisa", "vlc", "vlc-plugin-ffmpeg"]);
        assert!(optional_uses(&sweep, &names(&["elisa", "vlc"]), &installed).is_empty());
    }

    #[test]
    fn an_optional_dependency_named_by_what_it_provides_is_found() {
        // mc lists "7zip" as optional; the installed provider is 7zip-full
        // (provides 7zip). Removing ark takes 7zip-full along.
        let installed = vec![
            pkg("ark", &[], &[]),
            pkg("7zip-full", &["7zip"], &[]),
            pkg("mc", &[], &[("7zip", "7z archives")]),
        ];
        let sweep = names(&["ark", "7zip-full"]);
        let got = optional_uses(&sweep, &names(&["ark"]), &installed);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].package, "7zip-full");
        assert_eq!(got[0].users, vec![("mc".to_string(), "7z archives".to_string())]);
    }

    #[test]
    fn a_swept_package_nobody_else_uses_is_quiet() {
        let installed = vec![
            pkg("elisa", &[], &[]),
            pkg("kirigami-addons", &[], &[]),
            pkg("vlc", &[], &[("vlc-plugin-ffmpeg", "ffmpeg plugin")]),
        ];
        let sweep = names(&["elisa", "kirigami-addons"]);
        assert!(optional_uses(&sweep, &names(&["elisa"]), &installed).is_empty());
    }

    #[test]
    fn the_dry_run_is_one_name_per_line() {
        assert_eq!(parse_sweep("elisa\nvlc-plugin-ffmpeg\n\n"), names(&["elisa", "vlc-plugin-ffmpeg"]));
        assert!(parse_sweep("").is_empty());
    }

    #[test]
    fn a_reason_pacman_did_not_give_is_left_out_of_the_words() {
        let uses = vec![OptionalUse { package: "x".into(), users: vec![("y".into(), "".into())] }];
        assert_eq!(use_lines(&uses), vec!["x goes too, but y uses it (optional)."]);
    }
}
