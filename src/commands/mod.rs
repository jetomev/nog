use crate::aur::{self, Helper};
use crate::tiers::{Tier, TierManager};
use crate::reboot;
use crate::config::NogConfig;
use crate::holds::{self, HoldStatus};
use crate::pacman::{self, CheckUpdatesError, PendingUpdate, Source};
use std::collections::HashMap;
use crate::local_db;
use crate::runlog;
use crate::sightings;
use crate::cache;
use crate::flatpak;
use crate::snap;
use crate::sources;
use crate::sync_db;

pub mod surface;
pub mod maintain;
pub use surface::{search, list, info, reinstall, tier, lock, unlock, status, history};
pub use maintain::clean;

// Catppuccin Mocha palette — true-color ANSI. Centralized so every tier-colored
// surface (currently `nog update`; eventually `nog search`) stays consistent.
const C_RED: &str     = "\x1b[38;2;243;139;168m"; // #F38BA8 — Tier 1
const C_YELLOW: &str  = "\x1b[38;2;249;226;175m"; // #F9E2AF — Tier 2
const C_GREEN: &str   = "\x1b[38;2;166;227;161m"; // #A6E3A1 — Tier 3
// v1.5.4 (#28): one colour per non-official source, kept clear of the three
// tier colours. Official repositories stay plain. The word is always printed,
// so nothing depends on colour.
const C_PEACH: &str   = "\x1b[38;2;250;179;135m"; // #FAB387 — AUR
const C_BLUE: &str    = "\x1b[38;2;137;180;250m"; // #89B4FA — Flatpak
const C_MAUVE: &str   = "\x1b[38;2;203;166;247m"; // #CBA6F7 — Snap
const C_SUBTEXT: &str = "\x1b[38;2;166;173;200m"; // #A6ADC8 — muted details
const C_BOLD: &str    = "\x1b[1m";
const C_RESET: &str   = "\x1b[0m";

fn tier_color(tier: &Tier) -> &'static str {
    match tier {
        Tier::One   => C_RED,
        Tier::Two   => C_YELLOW,
        Tier::Three => C_GREEN,
    }
}

/// Resolve the AUR helper once per command invocation. Returns `None` when the
/// user has disabled AUR support or "auto" found nothing installed; returns
/// `Some` when a helper is available and should drive AUR-aware paths. Hard
/// errors (invalid config value, explicit helper missing) exit the process so
/// every caller gets the same failure semantics.
fn resolve_helper(cfg: &NogConfig) -> Option<Helper> {
    // v1.0.9 (Ironhold): the source kill switch outranks the configured
    // helper. While the AUR is deactivated, every AUR-aware path behaves as
    // if no helper were installed — detection, install routing, and the
    // upgrade handoff (which then runs through pacman, so foreign packages
    // are structurally untouchable).
    if !sources::load(sources::DEFAULT_PATH).aur {
        println!(
            "{}nog: AUR is DEACTIVATED (kill switch) — official repos only. `nog activate aur` re-enables.{}",
            C_SUBTEXT, C_RESET
        );
        return None;
    }
    match aur::detect_helper(&cfg.aur.helper) {
        Ok(opt) => opt,
        Err(e) => {
            eprintln!("nog: {}", e);
            end(1);
        }
    }
}

/// Fail fast if nog is invoked through sudo while a helper is configured.
/// yay and paru refuse to run as root, so the helper-driven code paths would
/// break later in a confusing way. Cleaner to surface the mismatch up front.
///
/// Detection is env-based: sudo sets SUDO_USER / SUDO_UID when it invokes us.
/// That's the exact case we care about; a user logged in as root directly
/// won't have these set and will just hit the helper's own root-refusal
/// message — still actionable.
fn guard_not_sudo_with_helper(helper: Option<Helper>) {
    if helper.is_none() { return; }
    if std::env::var_os("SUDO_USER").is_none() && std::env::var_os("SUDO_UID").is_none() {
        return;
    }
    eprintln!(
        "nog: detected `sudo nog` invocation with an AUR helper configured ({}).",
        helper.map(|h| h.to_string()).unwrap_or_default()
    );
    eprintln!("     AUR helpers refuse to run as root; they sudo internally when they need it.");
    eprintln!("     Re-run without sudo: `nog <command>` (nog will prompt for sudo itself).");
    end(1);
}

pub fn install(packages: &[String], yes: bool) {
    // Explicit user action — never gate or block. Just report tier classification
    // for transparency, then hand off. Tier protection lives in the passive
    // `nog update` path, not at install time.
    let cfg = load_config();
    let helper = resolve_helper(&cfg);
    guard_not_sudo_with_helper(helper);

    let tm = load_tiers();

    // v1.6.0 (F-8, Javier 3 Oct 2026): "if I ask pacman to install a package,
    // it just installs the package. nog has to do the same, not update the
    // system." An install does what was typed, like `pacman -S`; the tier-aware
    // update is `nog update`'s job. (v1.5.6 #30 ran an update first; installing
    // a package file pulled 20 unrelated updates with it.) When old package
    // lists are why an install failed, the closing notice says so instead.

    // Issue #17: a locally built package file goes to `pacman -U`. Every Forge
    // release installs one for its dogfood, and until now that step had to
    // leave nog for raw pacman.
    let files = match split_install_args(packages, |p| std::path::Path::new(p).is_file()) {
        Ok(InstallArgs::Names) => None,
        Ok(InstallArgs::Files) => Some(packages),
        Err(msg) => {
            eprintln!("nog: {}", msg);
            end(1);
        }
    };
    if let Some(files) = files {
        if !yes && !at_the_keyboard() {
            enotice(C_PEACH, &format!("Not installing: {}", packages.join(" ")), &nobody_to_answer_lines("install"));
            end(1);
        }
        install_files(files, &tm, yes);
        return;
    }

    // v1.5.8 (F-7, #39): one notice for the install itself; pacman asks
    // next (F-6, #38), showing exactly what, dependencies included.
    // v1.7.0 (F-11): `repo/name` installs from that source only (nogForge names
    // the row you picked: `aur/neofetch` is built from the AUR, never swapped for
    // a repository package that says it provides neofetch)
    if helper.is_none() {
        if let Some(p) = packages.iter().find(|p| p.starts_with("aur/")) {
            eprintln!("nog: {} is from the AUR, and no AUR helper is set up (yay or paru).", bare_name(p));
            end(1);
        }
    }
    // v1.9.0 (C3): `flatpak/<app>` and `snap/<name>` name that source only, like `aur/`.
    let state = sources::load(sources::DEFAULT_PATH);
    for p in packages {
        if p.starts_with("flatpak/") && !state.flatpak {
            eprintln!("nog: {} is a Flatpak, and Flatpak is DEACTIVATED. `nog activate flatpak` re-enables.", bare_name(p));
            end(1);
        }
        if p.starts_with("snap/") && !state.snap {
            eprintln!("nog: {} is a Snap, and Snap is DEACTIVATED. `nog activate snap` re-enables.", bare_name(p));
            end(1);
        }
    }
    // v1.8.0 (Javier, 4 Oct 2026, option A): where each package comes from,
    // before anything is downloaded or built; nog's own question only when the
    // AUR is involved (there pacman's question comes after the build), and a
    // line saying who takes over. pacman and the helper still decide the
    // transaction; this table follows pacman's own order (exact name, then a
    // package that provides it, then a group), then the AUR.
    let official = sync_db::load_packages();
    let unresolved: Vec<String> = packages.iter()
        .filter(|p| p.starts_with("aur/") || (!p.contains('/') && !official.contains_key(p.as_str())
            && providers_of(p, official).is_empty() && !pacman_resolves(p)))
        .map(|p| bare_name(p).to_string()).collect();
    let aur_versions = match helper {
        Some(h) => aur::versions_for(h, &unresolved),
        None => HashMap::new(),
    };
    let mut sources: Vec<(String, InstallSource)> = packages.iter().map(|p| {
        (p.clone(), resolve_install(p, official, &|n: &str| aur_versions.get(n).cloned(), &pacman_resolves))
    }).collect();

    // v1.9.0 (C3, the rest of the chain): what the repositories and the AUR do
    // not have is looked for in Flatpak, then Snap — the order locked in
    // docs/v2-design.md. A backend that is not set up is offered, once, to a
    // person at the keyboard; never installed unasked. `flatpak/x` and
    // `snap/x` look in that source only.
    let mut chain = Chain::new(&state, helper, yes);
    for (asked, src) in sources.iter_mut() {
        let only_flatpak = asked.starts_with("flatpak/");
        let only_snap = asked.starts_with("snap/");
        if !(only_flatpak || only_snap || (matches!(src, InstallSource::Nowhere) && !asked.contains('/'))) {
            continue;
        }
        let name = bare_name(asked);
        if !only_snap {
            match chain.flatpak(name) {
                ChainAnswer::Found(s) => { *src = s; continue; }
                ChainAnswer::Ambiguous(apps) => {
                    // v1.9.0 (F-15, #51): several answer to the name — say where
                    // nog looked, number them, and ask which one. A snap with
                    // that exact name goes on the same list (only when snapd is
                    // already up: no set-up offer belongs in this question).
                    // A script cannot pick a number, so without a keyboard, or
                    // with --yes, the list is shown and nothing is installed.
                    let mut options: Vec<InstallSource> = apps.into_iter()
                        .map(|app| InstallSource::Flatpak { app }).collect();
                    if !only_flatpak && chain.snap_on && snap::is_available() {
                        if let ChainAnswer::Found(s) = chain.snap(name) { options.push(s); }
                    }
                    let lines = where_looked_lines(name, !only_flatpak, !only_flatpak && helper.is_some(), &options);
                    if yes || !at_the_keyboard() {
                        let mut lines = lines;
                        lines.extend(nobody_to_pick_lines(&options));
                        enotice(C_PEACH, &format!("Not installing: {}", packages.join(" ")), &lines);
                        end(1);
                    }
                    notice(C_BLUE, &format!("Several answer to '{}'", name), &lines);
                    match prompt_pick(&format!("nog: Which one? [1-{}, or n for none] ", options.len()), options.len()) {
                        Some(i) => { *src = options.swap_remove(i); continue; }
                        None => {
                            println!("nog: Cancelled — nothing was changed.");
                            end(1);
                        }
                    }
                }
                ChainAnswer::NotFound => {}
            }
        }
        if !only_flatpak {
            if let ChainAnswer::Found(s) = chain.snap(name) {
                *src = s;
            }
        }
    }
    let missing: Vec<&String> = sources.iter().filter(|(_, s)| matches!(s, InstallSource::Nowhere))
        .map(|(p, _)| p).collect();
    if !missing.is_empty() {
        // A `flatpak/` or `snap/` name was looked for in that source only, so
        // the notice names only it; a plain name names the whole chain.
        let looked: Vec<&str> = if missing.iter().all(|p| p.starts_with("flatpak/")) { vec!["Flatpak"] }
            else if missing.iter().all(|p| p.starts_with("snap/")) { vec!["Snap"] }
            else { chain.looked_in(helper.is_some()) };
        let missing: Vec<String> = missing.iter().map(|p| bare_name(p).to_string()).collect();
        enotice(C_PEACH, &format!("Not installing: {}", packages.join(" ")),
            &not_found_lines(&missing, &looked, &chain.skipped));
        end(1);
    }
    let any_aur = sources.iter().any(|(_, s)| matches!(s, InstallSource::Aur { .. }));
    let any_snap = sources.iter().any(|(_, s)| matches!(s, InstallSource::Snap { .. }));
    let helper_name = helper.map(|h| h.binary()).unwrap_or("pacman");
    notice(C_GREEN, &format!("Installing {}", packages.iter().map(|p| bare_name(p)).collect::<Vec<_>>().join(" ")), &[
        "Where each comes from, before anything is downloaded or built:".to_string()]);
    print!("{}", format_install_table(&install_rows(&sources, &tm, helper_name)));
    println!();

    // v1.9.0 (#32): `--yes` answers the install's question for you — for
    // repository packages, Flatpak and Snap. Never for the AUR: the helper
    // stops to let you review each build recipe, and that review is the one
    // safety the AUR has. A script that wants an AUR build runs in a terminal.
    if yes && any_aur {
        let aur: Vec<String> = sources.iter().filter(|(_, s)| matches!(s, InstallSource::Aur { .. }))
            .map(|(p, _)| bare_name(p).to_string()).collect();
        enotice(C_PEACH, &format!("Not installing: {}", packages.join(" ")), &[
            format!("--yes does not answer the AUR, and {} would be built from it.", aur.join(", ")),
            format!("{} stops to let you review each build recipe; that review is the AUR's one safety,", helper_name),
            format!("so nog never answers it for you. Run this in a terminal: nog install {}", packages.join(" ")),
        ]);
        end(1);
    }
    // v1.5.4 (#26): the AUR helper stops to let you review each build recipe.
    // With nobody at the keyboard it read end-of-input in its menu and died
    // there, and the reason was buried in its output. Answering for you would
    // skip the one review the AUR offers, so nog refuses up front instead.
    if any_aur && !at_the_keyboard() {
        let aur: Vec<String> = sources.iter().filter(|(_, s)| matches!(s, InstallSource::Aur { .. }))
            .map(|(p, _)| bare_name(p).to_string()).collect();
        eprintln!("nog: {} not in the official repositories, so it would be built from the AUR.",
            aur.join(", "));
        eprintln!("     The AUR helper stops to let you review each build recipe, and nothing");
        eprintln!("     is at the keyboard to answer. Run this in a terminal: nog install {}",
            packages.join(" "));
        end(1);
    }
    // v1.9.0 (#32): the same gap for everything else. pacman and flatpak ask
    // before changing anything, and with no terminal they read end-of-input
    // as "no" — reported as if someone had answered. Say the real cause, and
    // the way out, before anything runs.
    if !yes && !at_the_keyboard() {
        enotice(C_PEACH, &format!("Not installing: {}", packages.join(" ")), &nobody_to_answer_lines("install"));
        end(1);
    }

    // v1.8.0 (option A): nog asks only when the AUR is involved — and, since
    // v1.9.0, Snap: `snap install` never asks, so nog's question is the one
    // chance to say no.
    if (any_aur || any_snap) && !yes && !prompt_install() {
        println!("nog: Cancelled — nothing was installed.");
        end(1);
    }

    // v1.9.0 (C3): one transaction per source, in the chain's order: pacman
    // (with the AUR through the helper), then Flatpak, then Snap. The table
    // above showed every one; each tool keeps its own voice. A step that
    // stops ends the run there, and the closing notice says what went in.
    let pacman_names: Vec<String> = sources.iter()
        .filter(|(_, s)| matches!(s, InstallSource::Repo { .. } | InstallSource::Group | InstallSource::Aur { .. }))
        .map(|(p, _)| p.clone()).collect();
    let flatpaks: Vec<(String, String)> = sources.iter().filter_map(|(_, s)| match s {
        InstallSource::Flatpak { app } => Some((app.remote.clone(), app.app_id.clone())),
        _ => None,
    }).collect();
    let snaps: Vec<(String, bool)> = sources.iter().filter_map(|(_, s)| match s {
        InstallSource::Snap { info } => Some((info.name.clone(), info.confinement == "classic")),
        _ => None,
    }).collect();
    let mut steps: Vec<(&str, String)> = Vec::new();
    if !pacman_names.is_empty() {
        steps.push(("install", format!("Installing {}", pacman_names.join(" "))));
    }
    if !flatpaks.is_empty() {
        steps.push(("flatpak", format!("Installing {} through Flatpak",
            flatpaks.iter().map(|(_, id)| id.as_str()).collect::<Vec<_>>().join(" "))));
    }
    if !snaps.is_empty() {
        steps.push(("snap", format!("Installing {} through Snap",
            snaps.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(" "))));
    }
    crate::events::steps(&steps);
    let mut done: Vec<String> = Vec::new();

    if !pacman_names.is_empty() {
        println!("{}", handoff_line(any_aur, helper_name, yes));
        let label = steps[0].1.clone();
        crate::events::start("install", &label);
        // When a helper is configured we route through it — the helper checks
        // sync repos before AUR, so official packages still install via pacman
        // under the hood. With --yes there is nothing for the helper to ask, so
        // pacman is called directly with the answer. The table above only shows
        // where each comes from; pacman and the helper still make the transaction.
        let status = match helper {
            Some(h) if !yes => aur::install(h, &pacman_names),
            _ => pacman::install(&pacman_names, yes),
        };
        crate::events::finished("install", &label, status.success(), "");
        if !status.success() {
            // v1.6.1 (F-9, #43): a name found nowhere is said plainly, not as "you answered no".
            let missing: Vec<String> = pacman_names.iter().filter(|p| !exists_anywhere(p, helper)).cloned().collect();
            if !missing.is_empty() {
                enotice(C_PEACH, &format!("Not installed: {}", packages.join(" ")),
                    &not_found_lines(&missing, &["the repositories", "the AUR"], &[]));
                end(status.code().unwrap_or(1));
            }
            stopped_installing(&pacman_names, status.code(), if yes { "pacman" } else { helper_name });
        }
        done.extend(pacman_names.iter().map(|p| bare_name(p).to_string()));
    }

    if !flatpaks.is_empty() {
        println!("{}", flatpak_handoff_line(yes));
        let label = format!("Installing {} through Flatpak",
            flatpaks.iter().map(|(_, id)| id.as_str()).collect::<Vec<_>>().join(" "));
        crate::events::start("flatpak", &label);
        let mut remotes: Vec<String> = flatpaks.iter().map(|(r, _)| r.clone()).collect();
        remotes.sort();
        remotes.dedup();
        for remote in &remotes {
            let ids: Vec<String> = flatpaks.iter().filter(|(r, _)| r == remote).map(|(_, id)| id.clone()).collect();
            let h = flatpak::install(remote, &ids, yes);
            if !h.status.success() {
                crate::events::finished("flatpak", &label, false, "");
                stopped_in_chain(&done, &ids, h.status.code(), "flatpak");
            }
            done.extend(ids);
        }
        crate::events::finished("flatpak", &label, true, "");
    }

    if !snaps.is_empty() {
        println!("nog: Handing off to snap: it installs without asking — nog asked above.");
        let label = format!("Installing {} through Snap",
            snaps.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(" "));
        crate::events::start("snap", &label);
        for classic in [false, true] {
            let names: Vec<String> = snaps.iter().filter(|(_, c)| *c == classic).map(|(n, _)| n.clone()).collect();
            if names.is_empty() { continue; }
            let h = snap::install(&names, classic);
            if !h.status.success() {
                crate::events::finished("snap", &label, false, "");
                stopped_in_chain(&done, &names, h.status.code(), "snap");
            }
            done.extend(names);
        }
        crate::events::finished("snap", &label, true, "");
    }
}

/// v1.9.0 (C3): the chain after the repositories and the AUR — Flatpak, then
/// Snap. Remembers what was already looked into, offered or skipped, so a
/// request with several names asks about a missing backend once and the
/// not-found notice can say where it did and did not look.
struct Chain {
    flatpak_on: bool,
    snap_on: bool,
    helper: Option<Helper>,
    yes: bool,
    flatpak_ready: Option<bool>,
    snap_ready: Option<bool>,
    looked: Vec<&'static str>,
    /// Plain-words reasons a source was not looked in, for the not-found notice.
    skipped: Vec<String>,
}

enum ChainAnswer {
    Found(InstallSource),
    Ambiguous(Vec<flatpak::FlatpakApp>),
    NotFound,
}

impl Chain {
    fn new(state: &sources::SourceState, helper: Option<Helper>, yes: bool) -> Chain {
        Chain { flatpak_on: state.flatpak, snap_on: state.snap, helper, yes,
                flatpak_ready: None, snap_ready: None, looked: Vec::new(), skipped: Vec::new() }
    }

    /// Where the chain looked, in order, for the not-found notice.
    fn looked_in(&self, aur: bool) -> Vec<&'static str> {
        let mut v = vec!["the repositories"];
        if aur { v.push("the AUR"); }
        v.extend(self.looked.iter().copied());
        v
    }

    fn skip(&mut self, line: String) {
        if !self.skipped.contains(&line) { self.skipped.push(line); }
    }

    /// Is Flatpak usable — set up already, or set up now on a person's yes?
    fn flatpak_usable(&mut self) -> bool {
        if let Some(r) = self.flatpak_ready { return r; }
        let ready = if !self.flatpak_on {
            self.skip("Flatpak was not checked: it is deactivated (`nog activate flatpak` re-enables).".into());
            false
        } else if flatpak::is_available() {
            if flatpak::has_remotes() { true } else {
                self.skip("Flatpak is set up but has no app store configured (`flatpak remotes` is empty).".into());
                false
            }
        } else if self.yes || !at_the_keyboard() {
            self.skip("Flatpak is not set up on this computer; `nog install flatpak` sets it up (a person at the keyboard is offered it).".into());
            false
        } else {
            println!();
            println!("nog: The repositories and the AUR don't have it. Flatpak may — but Flatpak is not set up");
            println!("     on this computer. Setting it up installs the flatpak package (extra), which comes");
            println!("     with Flathub, its main app store. pacman asks before installing it.");
            if !prompt_yes_no("nog: Set up Flatpak and look there? [y/N] ", false) {
                self.skip("Flatpak is not set up on this computer (you chose not to set it up this time).".into());
                false
            } else {
                let status = pacman::install(&["flatpak".to_string()], false);
                if status.success() && flatpak::is_available() {
                    println!("nog: Flatpak is set up. Looking there.");
                    true
                } else {
                    self.skip("Flatpak could not be set up (pacman stopped: you answered no, or it hit a problem shown above).".into());
                    false
                }
            }
        };
        self.flatpak_ready = Some(ready);
        ready
    }

    fn flatpak(&mut self, name: &str) -> ChainAnswer {
        if !self.flatpak_usable() { return ChainAnswer::NotFound; }
        if !self.looked.contains(&"Flatpak") { self.looked.push("Flatpak"); }
        match flatpak::search(name) {
            Err(e) => {
                self.skip(format!("Flatpak could not be checked: {}", e));
                ChainAnswer::NotFound
            }
            Ok(apps) => match flatpak::find(name, &apps) {
                flatpak::Found::One(app) => ChainAnswer::Found(InstallSource::Flatpak { app }),
                flatpak::Found::Several(apps) => ChainAnswer::Ambiguous(apps),
                flatpak::Found::None => ChainAnswer::NotFound,
            },
        }
    }

    /// Is Snap usable — snapd running already, or set up now on a person's yes?
    fn snap_usable(&mut self) -> bool {
        if let Some(r) = self.snap_ready { return r; }
        let ready = if !self.snap_on {
            self.skip("Snap was not checked: it is deactivated (`nog activate snap` re-enables).".into());
            false
        } else if snap::is_available() {
            true
        } else if self.helper.is_none() {
            self.skip("Snap is not set up on this computer, and snapd comes from the AUR, which needs yay or paru.".into());
            false
        } else if self.yes || !at_the_keyboard() {
            self.skip("Snap is not set up on this computer (a person at the keyboard is offered it).".into());
            false
        } else {
            let h = self.helper.unwrap();
            println!();
            println!("nog: Flatpak doesn't have it either. Snap may — but Snap is not set up on this computer.");
            println!("     Setting it up builds snapd from the AUR ({} shows the recipe and asks), starts its", h);
            println!("     service, and links /snap so 'classic' snaps work. snapd then refreshes its snaps on");
            println!("     its own schedule; nog places tier holds in it (`man nog`, SNAP SUPPORT).");
            if !prompt_yes_no("nog: Set up Snap and look there? [y/N] ", false) {
                self.skip("Snap is not set up on this computer (you chose not to set it up this time).".into());
                false
            } else if !set_up_snapd(h) {
                self.skip("Snap could not be set up (a step stopped, shown above).".into());
                false
            } else {
                println!("nog: Snap is set up. Looking there. (Snap's commands land in /var/lib/snapd/snap/bin,");
                println!("     on your PATH after you log in again; snap says so too after the first install.)");
                true
            }
        };
        self.snap_ready = Some(ready);
        ready
    }

    fn snap(&mut self, name: &str) -> ChainAnswer {
        if !self.snap_usable() { return ChainAnswer::NotFound; }
        if !self.looked.contains(&"Snap") { self.looked.push("Snap"); }
        match snap::info(name) {
            Err(e) => {
                self.skip(format!("Snap could not be checked: {}", e));
                ChainAnswer::NotFound
            }
            Ok(Some(info)) => ChainAnswer::Found(InstallSource::Snap { info }),
            Ok(None) => ChainAnswer::NotFound,
        }
    }
}

/// v1.9.0 (C3): snapd from the AUR, its socket started, `/snap` linked for
/// classic snaps (the Arch wiki's three steps), then a wait for snapd's seed
/// so the first `snap info` doesn't fail on a service still starting. Each
/// step asks or shows its work; the first that stops ends it.
fn set_up_snapd(helper: Helper) -> bool {
    println!("nog: Handing off to {} for snapd.", helper);
    if !aur::install(helper, &["snapd".to_string()]).success() {
        return false;
    }
    println!("nog: Starting snapd's service (sudo systemctl enable --now snapd.socket).");
    if !crate::machine::sudo().args(["systemctl", "enable", "--now", "snapd.socket"]).status()
        .map(|s| s.success()).unwrap_or(false) {
        return false;
    }
    if !std::path::Path::new("/snap").exists() {
        println!("nog: Linking /snap to /var/lib/snapd/snap (classic snaps expect it).");
        if !crate::machine::sudo().args(["ln", "-s", "/var/lib/snapd/snap", "/snap"]).status()
            .map(|s| s.success()).unwrap_or(false) {
            return false;
        }
    }
    println!("nog: Waiting for snapd to be ready (snap wait system seed.loaded).");
    let _ = std::process::Command::new("snap").args(["wait", "system", "seed.loaded"]).status();
    snap::is_available()
}

/// v1.9.0 (#32): is a person at the keyboard to answer a question?
fn at_the_keyboard() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

/// v1.9.0 (#32): the words when nothing is at the keyboard and `--yes` was
/// not given — before pacman reads end-of-input as "no".
fn nobody_to_answer_lines(verb: &str) -> Vec<String> {
    vec![
        "Nothing is at the keyboard to answer the question asked before anything changes".to_string(),
        "(pacman's, flatpak's, or nog's own for Snap), and with no answer it stops. Nothing was changed.".to_string(),
        format!("Run this in a terminal, or add --yes: nog answers yes for you (`nog {} --yes …`).", verb),
        "--yes covers repository packages, Flatpak and Snap; never the AUR's build review.".to_string(),
    ]
}

/// v2.1.0 (F-20, #56): the words when `nog update` has nobody to answer and no `--yes`.
/// Until now pacman read end-of-input as "no" and the run said "Cancelled", as if
/// someone had answered.
fn nobody_to_answer_update_lines() -> Vec<String> {
    vec![
        "Nothing is at the keyboard to answer the question asked before anything changes,".to_string(),
        "and with no answer nog stops. Nothing was changed.".to_string(),
        "Run this in a terminal, or add --yes: nog answers for you (`nog update --yes`).".to_string(),
        "--yes installs what is Ready; Unknown packages are kept back and listed, and the".to_string(),
        "AUR's build review waits for a person.".to_string(),
    ]
}

/// v2.1.0 (F-20, #56): --yes with nobody at the keyboard, and sudo would ask for a password
/// nobody can type. Said before anything changes, instead of failing at pacman's step.
fn sudo_needs_a_person_lines() -> Vec<String> {
    vec![
        "--yes answers nog's and pacman's questions, but sudo still asks for a password,".to_string(),
        "and nobody is at the keyboard to type it. Nothing was changed.".to_string(),
        "For a timer or a script: a sudo rule that lets this user run pacman without a".to_string(),
        "password, or NOG_ASKPASS=1 with a password window (nogForge's way). Or run it in a terminal.".to_string(),
    ]
}

/// v2.1.0 (F-20, #56): with --yes, what becomes of the Unknown packages: every one is kept
/// back. An Unknown is a decision (no usable build date), and a blanket yes would hide it;
/// `yes | nog update` used to answer it.
fn keep_unknowns_back(unknown: &[String]) -> Vec<String> {
    unknown.to_vec()
}

/// A plain yes/no question of nog's own, with a default. End-of-input is the default.
fn prompt_yes_no(question: &str, default_yes: bool) -> bool {
    use std::io::{self, Write};
    crate::events::ask(question.trim_start_matches("nog: ").trim());
    print!("{}", question);
    let _ = io::stdout().flush();
    if !at_the_keyboard() {
        println!();
        return default_yes;
    }
    let mut buf = String::new();
    match io::stdin().read_line(&mut buf) {
        Ok(0) | Err(_) => default_yes,
        Ok(_) => match buf.trim().to_lowercase().as_str() {
            "" => default_yes,
            "y" | "yes" => true,
            _ => false,
        },
    }
}

/// v1.9.0 (F-15, #51): several answer to one name. Where nog looked, in the
/// chain's order, then the options numbered: the Flatpak apps, and a snap of
/// that exact name when there is one. Pure.
fn where_looked_lines(name: &str, repos: bool, aur: bool, options: &[InstallSource]) -> Vec<String> {
    let mut lines = Vec::new();
    if repos { lines.push("Not in the repositories.".to_string()); }
    if aur { lines.push("Not in the AUR.".to_string()); }
    let flatpaks = options.iter().filter(|o| matches!(o, InstallSource::Flatpak { .. })).count();
    lines.push(format!("Flatpak, {} apps answer to '{}':", flatpaks, name));
    let width = options.iter().map(|o| option_name(o).len()).max().unwrap_or(0);
    for (i, o) in options.iter().enumerate() {
        if i == flatpaks { lines.push("Snap has one too:".to_string()); }
        lines.push(format!("  {:>2}. {:<w$}  {}", i + 1, option_name(o), option_detail(o), w = width));
    }
    lines
}

/// The name a person picks by: the Flatpak app ID, or the snap's store name.
fn option_name(o: &InstallSource) -> String {
    match o {
        InstallSource::Flatpak { app } => app.app_id.clone(),
        InstallSource::Snap { info } => info.name.clone(),
        _ => String::new(),
    }
}

/// What tells the options apart: the app's human name, version and remote;
/// the snap's confinement, version and publisher.
fn option_detail(o: &InstallSource) -> String {
    match o {
        InstallSource::Flatpak { app } => {
            let v = if app.version.is_empty() { String::new() } else { format!(", {}", app.version) };
            format!("{}{} — {}", app.name, v, app.remote)
        }
        InstallSource::Snap { info } => {
            let v = if info.version.is_empty() { String::new() } else { format!(", {}", info.version) };
            let kind = if info.confinement == "classic" { "classic" } else { "sandboxed" };
            format!("Snap — {}{}, by {}", kind, v, info.publisher)
        }
        _ => String::new(),
    }
}

/// The two lines added when nobody can pick: a script's shell, or `--yes`.
fn nobody_to_pick_lines(options: &[InstallSource]) -> Vec<String> {
    let first = options.iter().find_map(|o| match o {
        InstallSource::Flatpak { app } => Some(format!("nog install flatpak/{}", app.app_id)),
        _ => None,
    }).unwrap_or_default();
    let snap = options.iter().find_map(|o| match o {
        InstallSource::Snap { info } => Some(format!(", or nog install snap/{}", info.name)),
        _ => None,
    }).unwrap_or_default();
    vec![
        "Nothing is at the keyboard to pick one (or --yes was given), so nothing was installed.".to_string(),
        format!("Name the one you want: {}{}", first, snap),
    ]
}

/// What an answer to "Which one?" means. Pure.
#[derive(Debug, PartialEq)]
enum Pick {
    /// The option's index (the number typed, minus one).
    Chosen(usize),
    /// Enter, n, no, q: none of them.
    None,
    /// Anything else: ask again.
    Invalid,
}

fn parse_pick(answer: &str, count: usize) -> Pick {
    match answer.trim().to_lowercase().as_str() {
        "" | "n" | "no" | "q" | "none" => Pick::None,
        a => match a.parse::<usize>() {
            Ok(n) if n >= 1 && n <= count => Pick::Chosen(n - 1),
            _ => Pick::Invalid,
        },
    }
}

/// Ask which option, by number. Asks again on an answer that is not on the
/// list; Enter, n or end-of-input mean none.
fn prompt_pick(question: &str, count: usize) -> Option<usize> {
    use std::io::{self, Write};
    crate::events::ask(question.trim_start_matches("nog: ").trim());
    loop {
        print!("{}", question);
        let _ = io::stdout().flush();
        let mut buf = String::new();
        match io::stdin().read_line(&mut buf) {
            Ok(0) | Err(_) => { println!(); return None; }
            Ok(_) => match parse_pick(&buf, count) {
                Pick::Chosen(i) => return Some(i),
                Pick::None => return None,
                Pick::Invalid => println!("nog: Type a number from 1 to {}, or n for none.", count),
            },
        }
    }
}

/// v1.9.0 (C3): a later step of the chain stopped. Say what already went in.
fn stopped_in_chain(done: &[String], failed: &[String], code: Option<i32>, tool: &str) -> ! {
    let mut lines = vec![
        format!("{} stopped (status {}): you answered no, or it hit a problem shown above.", tool, code.unwrap_or(-1)),
    ];
    if done.is_empty() {
        lines.push("Nothing was changed by this install.".to_string());
    } else {
        lines.push(format!("Already installed before it stopped: {}.", done.join(" ")));
    }
    if tool == "snap" {
        lines.push("A snap that needs 'classic' confinement says so above; nog passes --classic when the store says the snap is classic.".to_string());
    }
    enotice(C_PEACH, &format!("Not installed: {}", failed.join(" ")), &lines);
    end(code.unwrap_or(1));
}

/// v1.8.0: where a package to install comes from, in pacman's own order.
#[derive(Debug, PartialEq)]
enum InstallSource {
    /// From a repository: the exact name, or `provider` which provides it
    /// (several providers: pacman asks which).
    Repo { repo: String, version: Option<String>, providers: Vec<String> },
    /// A name pacman resolves that is not a package: a group.
    Group,
    /// Built from the AUR by the helper.
    Aur { version: Option<String> },
    /// v1.9.0 (C3): a Flatpak app from one of flatpak's remotes.
    Flatpak { app: flatpak::FlatpakApp },
    /// v1.9.0 (C3): a snap from the Snap store.
    Snap { info: snap::SnapInfo },
    Nowhere,
}

/// The repository packages that provide `name` (by `%PROVIDES%`), as `(package, repo, version)`.
fn providers_of(name: &str, official: &HashMap<String, sync_db::PackageDesc>) -> Vec<(String, String, Option<String>)> {
    let mut out: Vec<(String, String, Option<String>)> = official.iter()
        .filter(|(_, d)| d.provides.iter().any(|p| p == name || p.split(['=', '<', '>']).next() == Some(name)))
        .map(|(n, d)| (n.clone(), d.repo.clone().unwrap_or_default(), d.version.clone()))
        .collect();
    out.sort();
    out
}

fn pacman_resolves(name: &str) -> bool {
    use std::process::{Command, Stdio};
    Command::new("pacman").args(["-Sp", "--print-format", "%n", "--", name])
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .status().map(|s| s.success()).unwrap_or(false)
}

/// v1.8.0: pacman's order — exact name, a provider, a group — then the AUR. Pure
/// apart from the two lookups passed in. Since v1.9.0 a `flatpak/` or `snap/`
/// name is left for the chain (`Nowhere` here means "not pacman's or the AUR's").
fn resolve_install(name: &str, official: &HashMap<String, sync_db::PackageDesc>,
                   aur: &dyn Fn(&str) -> Option<String>, resolves: &dyn Fn(&str) -> bool) -> InstallSource {
    if name.starts_with("flatpak/") || name.starts_with("snap/") {
        return InstallSource::Nowhere;
    }
    if let Some(n) = name.strip_prefix("aur/") {
        return match aur(n) { Some(v) => InstallSource::Aur { version: Some(v) }, None => InstallSource::Nowhere };
    }
    if let Some((repo, n)) = name.split_once('/') {
        return match official.get(n) {
            Some(d) if d.repo.as_deref() == Some(repo) =>
                InstallSource::Repo { repo: repo.to_string(), version: d.version.clone(), providers: vec![] },
            _ if resolves(name) => InstallSource::Repo { repo: repo.to_string(), version: None, providers: vec![] },
            _ => InstallSource::Nowhere,
        };
    }
    if let Some(d) = official.get(name) {
        return InstallSource::Repo { repo: d.repo.clone().unwrap_or_default(), version: d.version.clone(), providers: vec![] };
    }
    let prov = providers_of(name, official);
    if let Some((_, repo, version)) = prov.first() {
        let version = if prov.len() == 1 { version.clone() } else { None };
        return InstallSource::Repo { repo: repo.clone(), version, providers: prov.into_iter().map(|p| p.0).collect() };
    }
    if resolves(name) {
        return InstallSource::Group;
    }
    match aur(name) {
        Some(v) => InstallSource::Aur { version: Some(v) },
        None => InstallSource::Nowhere,
    }
}

/// v1.8.0: the install table's rows: package, version, source, tier.
fn install_rows(sources: &[(String, InstallSource)], tm: &TierManager, helper: &str) -> Vec<[String; 4]> {
    sources.iter().map(|(asked, src)| {
        let name = bare_name(asked).to_string();
        let mut tier = format!("{}", tm.classify(&name));
        let (version, from) = match src {
            InstallSource::Repo { repo, version, providers } => {
                let from = match providers.len() {
                    0 => repo.clone(),
                    1 => format!("{} — {} provides it", repo, providers[0]),
                    n => format!("{} packages provide it; pacman asks which", n),
                };
                (version.clone().unwrap_or_else(|| "—".into()), from)
            }
            InstallSource::Group => ("—".into(), "a group — pacman lists its members".into()),
            InstallSource::Aur { version } => (version.clone().unwrap_or_else(|| "—".into()),
                                               format!("AUR — built by {}", helper)),
            // v1.9.0 (C3): the app ID is what Flatpak installs; said when it
            // differs from what was typed. Tiers are kept by app ID (as in
            // `nog update`), so a pin on the ID applies.
            InstallSource::Flatpak { app } => {
                tier = format!("{}", tm.classify(&app.app_id));
                let from = if app.app_id.eq_ignore_ascii_case(&name) {
                    format!("Flatpak — {}", app.remote)
                } else {
                    format!("Flatpak — {}, as {}", app.remote, app.app_id)
                };
                (if app.version.is_empty() { "—".into() } else { app.version.clone() }, from)
            }
            InstallSource::Snap { info } => {
                let from = match info.confinement.as_str() {
                    "classic" => "Snap — classic: may change anything on the system, like a package".to_string(),
                    "strict" => "Snap — sandboxed".to_string(),
                    _ => "Snap".to_string(),
                };
                (if info.version.is_empty() { "—".into() } else { info.version.clone() }, from)
            }
            InstallSource::Nowhere => ("—".into(), "not found".into()),
        };
        [name, version, from, tier]
    }).collect()
}

fn format_install_table(rows: &[[String; 4]]) -> String {
    let head = ["Package", "Version", "Source", "Tier"];
    let w: Vec<usize> = (0..3).map(|i| rows.iter().map(|r| r[i].chars().count())
        .chain(std::iter::once(head[i].len())).max().unwrap_or(0)).collect();
    let line = |c: [&str; 4]| format!("    {:<a$}  {:<b$}  {:<c$}  {}\n", c[0], c[1], c[2], c[3], a = w[0], b = w[1], c = w[2]);
    let mut out = line(head);
    out.push_str(&format!("    {}\n", "-".repeat(w[0] + w[1] + w[2] + 6 + head[3].len() + 2)));
    for r in rows {
        out.push_str(&line([&r[0], &r[1], &r[2], &r[3]]));
    }
    out
}

/// v1.8.0: who takes over, said before they do.
fn handoff_line(aur: bool, helper: &str, yes: bool) -> String {
    if aur {
        format!("nog: Handing off to {}: it shows each build recipe to review, builds, then pacman asks once more before installing.", helper)
    } else if yes {
        "nog: Handing off to pacman with your yes (--yes): it shows what comes with it and installs without asking.".to_string()
    } else {
        "nog: Handing off to pacman: it shows what comes with it and asks before anything changes.".to_string()
    }
}

/// v1.9.0 (C3): flatpak's turn, said before it takes it.
fn flatpak_handoff_line(yes: bool) -> String {
    if yes {
        "nog: Handing off to flatpak with your yes (--yes): it shows what comes with it (the runtimes it needs) and installs without asking.".to_string()
    } else {
        "nog: Handing off to flatpak: it shows what comes with it (the runtimes it needs) and asks before anything changes.".to_string()
    }
}

/// v1.8.0 (option A): nog's own question, only when the AUR (or, v1.9.0, Snap) is involved.
fn prompt_install() -> bool {
    if !at_the_keyboard() {
        crate::events::ask("Install these? [Y/n]");
        println!("nog: Install these? [Y/n] ");
        return false;
    }
    prompt_yes_no("nog: Install these? [Y/n] ", true)
}

/// v1.7.0 (F-11): a name as typed, without its `repo/` (or `aur/`) in front.
fn bare_name(name: &str) -> &str {
    name.split_once('/').map(|(_, n)| n).unwrap_or(name)
}

/// v1.6.1 (F-9, #43): is there anything by this name to install? pacman
/// resolves it the way an install would (groups and provides included), then
/// the AUR through the helper. Asked only after an install failed.
fn exists_anywhere(name: &str, helper: Option<crate::aur::Helper>) -> bool {
    use std::process::{Command, Stdio};
    if name.starts_with("aur/") && helper.is_none() {
        return false;
    }
    let official = !name.starts_with("aur/") && Command::new("pacman").args(["-Sp", "--print-format", "%n", "--", name])
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .status().map(|s| s.success()).unwrap_or(true);
    if official {
        return true;
    }
    match helper {
        Some(h) => Command::new(h.binary()).args(["-Sai", "--", bare_name(name)]).stdin(Stdio::null()).output()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().any(|l| l.starts_with("Name")))
            .unwrap_or(true),
        None => false,
    }
}

/// v1.6.1 (F-9, #43): the words for names found nowhere. Pure. `looked` is
/// where the chain looked, in order ("the repositories", "the AUR", "Flatpak",
/// "Snap"); `skipped` says why a source was not looked in (v1.9.0).
fn not_found_lines(missing: &[String], looked: &[&str], skipped: &[String]) -> Vec<String> {
    let (what, verb) = if missing.len() == 1 { (missing[0].clone(), "was") } else { (missing.join(", "), "were") };
    let places = match looked.len() {
        0 => "anywhere".to_string(),
        1 => looked[0].to_string(),
        n => format!("{} or {}", looked[..n - 1].join(", "), looked[n - 1]),
    };
    let mut lines = vec![format!("{} {} not found in {}.", what, verb, places)];
    lines.extend(skipped.iter().cloned());
    lines.push(format!("Nothing was asked and nothing was changed. Check the spelling: `nog search {}` shows close matches.",
        missing[0]));
    lines
}

/// v1.5.8 (F-6 #38, F-7 #39): the install was declined or failed. pacman's
/// exit status cannot tell the two apart, so say both.
fn stopped_installing(what: &[String], code: Option<i32>, tool: &str) -> ! {
    let mut lines = vec![
        format!("{} stopped (status {}): you answered no, or it hit a problem shown above.",
            tool, code.unwrap_or(-1)),
        "Nothing was changed by this install.".to_string(),
    ];
    // F-8: no update before an install any more, so old package lists are a
    // likely cause of a failed download (#30). Say it; never do it unasked.
    if let Some(r) = refresh_reason(
        sync_db::lists_age_secs(), sync_db::update_stamp_age_secs(), LISTS_MAX_AGE_SECS) {
        lines.push(format!("If it could not find or download a file: {}.", r.words()));
        lines.push("`nog update` brings the system and its lists up to date; then try again.".to_string());
    }
    enotice(C_PEACH, &format!("Not installed: {}", what.join(" ")), &lines);
    end(code.unwrap_or(1));
}

/// v1.5.6 (#30): package lists older than this may be why an install failed.
const LISTS_MAX_AGE_SECS: u64 = 24 * 3600;

/// v1.5.7 (F-3, #35): why the package lists may be out of date (said when an install fails).
#[derive(Debug, PartialEq)]
enum RefreshReason {
    ListsMissing,
    NoRecord,
    RecordOld(u64),
    ListsChangedOutside,
}

impl RefreshReason {
    fn words(&self) -> String {
        match self {
            RefreshReason::ListsMissing => "the package lists are missing".to_string(),
            RefreshReason::NoRecord => "there is no record of a completed update on this computer yet".to_string(),
            RefreshReason::RecordOld(a) => format!("the last completed update was {} ago", age_words(*a)),
            RefreshReason::ListsChangedOutside => "the package lists changed after the last completed update".to_string(),
        }
    }
}

/// Pure. The stamp is written only when the lists were refreshed AND the
/// repository step completed with them, so a fresh stamp vouches for the
/// lists of that moment. Lists newer than the stamp were refreshed by
/// something that did not finish — a declined pacman prompt, or `pacman -Sy`
/// outside nog.
fn refresh_reason(lists: Option<u64>, stamp: Option<u64>, max: u64) -> Option<RefreshReason> {
    let lists = match lists {
        None => return Some(RefreshReason::ListsMissing),
        Some(a) => a,
    };
    match stamp {
        None => Some(RefreshReason::NoRecord),
        Some(s) if s > max => Some(RefreshReason::RecordOld(s)),
        Some(s) if lists < s => Some(RefreshReason::ListsChangedOutside),
        Some(_) => None,
    }
}

/// "3 days", "1 day", "5 hours" — how old the lists are, in plain words.
fn age_words(secs: u64) -> String {
    let (d, h) = (secs / 86_400, secs / 3600);
    match (d, h) {
        (0, 1) => "1 hour".into(),
        (0, h) => format!("{} hours", h),
        (1, _) => "1 day".into(),
        (d, _) => format!("{} days", d),
    }
}

/// What `nog install` was given (issue #17).
#[derive(Debug, PartialEq)]
enum InstallArgs {
    /// Package names, resolved from the repositories (and the AUR).
    Names,
    /// Package files on disk, installed as they are.
    Files,
}

/// Does this argument name a package file rather than a package?
///
/// Decided by the name: a repository package name can never contain
/// `.pkg.tar`, and every file makepkg writes does. Whether the file exists
/// is checked separately, so a typo in a path is reported as a missing file
/// instead of being searched for in the repositories.
fn looks_like_package_file(arg: &str) -> bool {
    arg.contains(".pkg.tar")
}

/// Sort the arguments, refusing the mixes nog will not guess at.
///
/// Names and files together are refused: pacman installs them with
/// different operations (`-S` and `-U`), and splitting one request into two
/// transactions could leave half of it installed. The user runs two
/// commands instead, and knows exactly what each one did.
fn split_install_args(args: &[String], exists: impl Fn(&str) -> bool) -> Result<InstallArgs, String> {
    let files: Vec<&String> = args.iter().filter(|a| looks_like_package_file(a)).collect();
    if files.is_empty() {
        return Ok(InstallArgs::Names);
    }
    if files.len() != args.len() {
        return Err(
            "package names and package files cannot be mixed in one install.\n     \
             Run `nog install` once for the names and once for the files."
                .to_string(),
        );
    }
    if let Some(missing) = files.iter().find(|f| !exists(f)) {
        return Err(format!("no such package file: {}", missing));
    }
    Ok(InstallArgs::Files)
}

/// `nog install ./foo-1.0-1-any.pkg.tar.zst` (issue #17).
///
/// The tier is read from the file's own metadata — a local build has no
/// repository entry to look up. No signature gate: this is an explicit
/// command, and like every `nog install` it does what was asked. pacman's
/// own `LocalFileSigLevel` still applies exactly as it would without nog.
fn install_files(files: &[String], tm: &TierManager, yes: bool) {
    // v1.5.8 (F-7, #39): one notice for the whole request, like a named install.
    let mut lines = Vec::new();
    let mut names = Vec::new();
    for f in files {
        match pacman::file_identity(f) {
            Some((name, version)) => {
                let tier = tm.classify(&name);
                lines.push(format!("{} {} is {} — from the file {}", name, version, tier, f));
                names.push(name);
            }
            None => {
                enotice(C_PEACH, "Not installing", &[format!("pacman cannot read {} as a package file.", f)]);
                end(1);
            }
        }
    }
    notice(C_GREEN, &format!("Installing {}", names.join(" ")), &lines);
    println!("{}", handoff_line(false, "pacman", yes));
    let label = format!("Installing {}", names.join(" "));
    crate::events::steps(&[("install", label.clone())]);
    crate::events::start("install", &label);
    let status = pacman::install_files(files, yes);
    crate::events::finished("install", &label, status.success(), "");
    if !status.success() {
        stopped_installing(&names, status.code(), "pacman");
    }
}

pub fn remove(packages: &[String], yes: bool) {
    // v1.9.0 (C3): what nog installs, nog removes. A name that is not an
    // installed package is looked for among the installed Flatpaks (by app
    // ID, its last segment, or the app's name) and snaps, each removed by its
    // own tool. A name found nowhere stops the whole request before anything.
    let (pkgs, others) = crate::removal::split_installed(packages);
    let state = sources::load(sources::DEFAULT_PATH);
    let fp_installed = if state.flatpak && flatpak::is_available() { flatpak::installed_apps() } else { Vec::new() };
    let snap_on = state.snap && snap::is_available();
    let mut flatpaks: Vec<String> = Vec::new();
    let mut snaps: Vec<String> = Vec::new();
    let mut nowhere: Vec<String> = Vec::new();
    for o in &others {
        if let Some(id) = flatpak::installed_match(o, &fp_installed) {
            flatpaks.push(id);
        } else if snap_on && snap::is_installed(o) {
            snaps.push(o.clone());
        } else {
            nowhere.push(o.clone());
        }
    }
    if !nowhere.is_empty() {
        let (what, verb) = if nowhere.len() == 1 { (nowhere[0].clone(), "is") } else { (nowhere.join(", "), "are") };
        enotice(C_PEACH, &format!("Not removing: {}", packages.join(" ")), &[
            format!("{} {} not installed on this computer: not a package, not a Flatpak, not a snap.", what, verb),
            "Nothing was asked and nothing was changed. `nog list` shows what is installed.".to_string(),
        ]);
        end(1);
    }
    // v1.9.0 (#32): pacman and flatpak ask before removing; with nothing at
    // the keyboard they read end-of-input as "no". Say so first.
    if !yes && !at_the_keyboard() {
        enotice(C_PEACH, &format!("Not removing: {}", packages.join(" ")), &nobody_to_answer_lines("remove"));
        end(1);
    }
    let mut steps: Vec<(&str, String)> = Vec::new();
    if !pkgs.is_empty() { steps.push(("remove", format!("Removing {}", pkgs.join(" ")))); }
    if !flatpaks.is_empty() { steps.push(("flatpak", format!("Removing {} through Flatpak", flatpaks.join(" ")))); }
    if !snaps.is_empty() { steps.push(("snap", format!("Removing {} through Snap", snaps.join(" ")))); }
    crate::events::steps(&steps);

    if !pkgs.is_empty() {
        remove_packages(&pkgs, yes);
    }
    if !flatpaks.is_empty() {
        notice(C_GREEN, &format!("Removing {}", flatpaks.join(" ")), &[
            if yes { "Flatpak apps: flatpak shows what goes and, with your yes (--yes), removes it without asking.".to_string() }
            else { "Flatpak apps: flatpak shows what goes and asks first.".to_string() }]);
        let label = format!("Removing {} through Flatpak", flatpaks.join(" "));
        crate::events::start("flatpak", &label);
        let h = flatpak::uninstall(&flatpaks, yes);
        crate::events::finished("flatpak", &label, h.status.success(), "");
        if !h.status.success() {
            enotice(C_PEACH, &format!("Not removed: {}", flatpaks.join(" ")), &[
                format!("flatpak stopped (status {}): you answered no, or it hit a problem shown above.",
                    h.status.code().unwrap_or(-1)),
                "Nothing more was changed by this removal.".to_string(),
            ]);
            end(h.status.code().unwrap_or(1));
        }
    }
    if !snaps.is_empty() {
        notice(C_GREEN, &format!("Removing {}", snaps.join(" ")), &[
            "Snaps: `snap remove` never asks, so nog asks here. Their data is kept by snapd for a while (snapd's own rule).".to_string()]);
        // v1.9.0 (F-16, #52): Enter means yes here, as at pacman's and flatpak's
        // question in the same removal — the person asked for it to go.
        if !yes && !prompt_yes_no(&format!("nog: Remove {}? [Y/n] ", snaps.join(" ")), true) {
            println!("nog: Cancelled — the snaps stay.");
            end(1);
        }
        let label = format!("Removing {} through Snap", snaps.join(" "));
        crate::events::start("snap", &label);
        let h = snap::remove(&snaps);
        crate::events::finished("snap", &label, h.status.success(), "");
        if !h.status.success() {
            enotice(C_PEACH, &format!("Not removed: {}", snaps.join(" ")), &[
                format!("snap stopped (status {}): it hit a problem shown above.", h.status.code().unwrap_or(-1)),
                "Nothing more was changed by this removal.".to_string(),
            ]);
            end(h.status.code().unwrap_or(1));
        }
    }
}

/// `nog remove` for installed packages: pacman's removal, with the F-13 check first.
fn remove_packages(packages: &[String], yes: bool) {
    // v1.9.0 (F-13, #49): before pacman, what the removal takes along, and
    // who still uses it. pacman's own dry run lists the sweep; a package in
    // it that another staying package lists as optional is said in plain
    // words and offered a Keep (default yes): kept means marked "installed on
    // purpose", so no removal takes it along — what `nog keep` does.
    let sweep = match crate::removal::sweep(packages) {
        Ok(s) => s,
        Err(e) => {
            enotice(C_PEACH, &format!("Not removing: {}", packages.join(" ")), &[
                format!("pacman says: {}", e.replace('\n', " / ")),
                "Nothing was asked and nothing was changed.".to_string(),
            ]);
            end(1);
        }
    };
    let installed = crate::machine::load_installed(std::path::Path::new("/var/lib/pacman/local"));
    let uses = crate::removal::optional_uses(&sweep, packages, &installed);
    if !uses.is_empty() {
        let mut lines = crate::removal::use_lines(&uses);
        lines.push("Kept means marked as installed on purpose (what `nog keep` does), so no removal takes it along.".to_string());
        notice(C_PEACH, "Going along, but still used", &lines);
        let mut keep: Vec<String> = Vec::new();
        for u in &uses {
            let kept = if yes {
                println!("nog: Keeping {} (--yes keeps what another app still uses).", u.package);
                true
            } else {
                prompt_yes_no(&format!("nog: Keep {}? [Y/n] ", u.package), true)
            };
            if kept { keep.push(u.package.clone()); }
        }
        if !keep.is_empty() {
            println!("nog: Marking {} as installed on purpose (sudo pacman -D --asexplicit).", keep.join(" "));
            let status = crate::removal::mark_explicit(&keep);
            if !status.success() {
                enotice(C_PEACH, &format!("Not removing: {}", packages.join(" ")), &[
                    format!("pacman could not mark {} (status {}): you cancelled the password, or it hit a problem shown above.",
                        keep.join(" "), status.code().unwrap_or(-1)),
                    "Nothing was removed.".to_string(),
                ]);
                end(status.code().unwrap_or(1));
            }
        }
    }
    notice(C_GREEN, &format!("Removing {}", packages.join(" ")), &[
        if yes { "With what only it needed; pacman shows the list and, with your yes (--yes), removes without asking.".to_string() }
        else { "With what only it needed; pacman shows the list and asks first.".to_string() },
    ]);
    let label = format!("Removing {}", packages.join(" "));
    crate::events::start("remove", &label);
    let status = pacman::remove(packages, yes);
    crate::events::finished("remove", &label, status.success(), "");
    if !status.success() {
        enotice(C_PEACH, &format!("Not removed: {}", packages.join(" ")), &[
            format!("pacman stopped (status {}): you answered no, or it hit a problem shown above.",
                status.code().unwrap_or(-1)),
            "Nothing was changed by this removal.".to_string(),
        ]);
        end(status.code().unwrap_or(1));
    }
}

/// v1.9.0 (F-13, #49): `nog keep <package>…` — mark packages as installed on
/// purpose, so removing something else never takes them along. pacman's
/// "install reason" (`pacman -D --asexplicit`), without leaving nog for it.
pub fn keep(packages: &[String]) {
    let (installed, missing) = crate::removal::split_installed(packages);
    if !missing.is_empty() {
        let (what, verb) = if missing.len() == 1 { (missing[0].clone(), "is") } else { (missing.join(", "), "are") };
        enotice(C_PEACH, &format!("Not keeping: {}", packages.join(" ")), &[
            format!("{} {} not installed, so there is nothing to keep.", what, verb),
            "Nothing was changed. `nog list` shows what is installed.".to_string(),
        ]);
        end(1);
    }
    notice(C_GREEN, &format!("Keeping {}", installed.join(" ")), &[
        format!("Marked as installed on purpose, so removing another package never takes {} along.",
            if installed.len() == 1 { "it" } else { "them" }),
        "(pacman's own install reason: sudo pacman -D --asexplicit.)".to_string(),
    ]);
    let label = format!("Keeping {}", installed.join(" "));
    crate::events::steps(&[("keep", label.clone())]);
    crate::events::start("keep", &label);
    let status = crate::removal::mark_explicit(&installed);
    crate::events::finished("keep", &label, status.success(), "");
    if !status.success() {
        enotice(C_PEACH, &format!("Not kept: {}", installed.join(" ")), &[
            format!("pacman stopped (status {}): you cancelled the password, or it hit a problem shown above.",
                status.code().unwrap_or(-1)),
            "Nothing was changed.".to_string(),
        ]);
        end(status.code().unwrap_or(1));
    }
}

/// v1.6.0 (#7): installed packages, for people and for nogForge (`--json`).
pub fn deactivate(source: &str) {
    set_source(source, false);
}

/// One line of the `nog clean` report.
#[derive(Debug, Clone, PartialEq)]
struct CleanRow {
    what: String,
    versions: usize,
    bytes: u64,
}

/// Render the `nog clean` report. Pure + unit-tested.
fn format_clean_report(rows: &[CleanRow], stale_dirs: usize) -> String {
    let title = "WHAT nog clean WOULD REMOVE:";
    let mut out = format!("{}\n{}\n\n", title, "=".repeat(title.len()));
    if rows.is_empty() && stale_dirs == 0 {
        out.push_str("(nothing)\n");
        return out;
    }
    let stale_label = "Leftover download folders";
    let w_what = rows.iter().map(|r| r.what.chars().count())
        .chain(["What".len(), "Total".len(), if stale_dirs > 0 { stale_label.len() } else { 0 }])
        .max().unwrap();
    let total_v: usize = rows.iter().map(|r| r.versions).sum();
    let total_b: u64 = rows.iter().map(|r| r.bytes).sum();
    let w_n = rows.iter().map(|r| r.versions.to_string().len())
        .chain(["Versions".len(), total_v.to_string().len(), stale_dirs.to_string().len()]).max().unwrap();
    let w_s = rows.iter().map(|r| human_size(r.bytes).len())
        .chain(["Size".len(), human_size(total_b).len()]).max().unwrap();
    let g = "   ";
    let width = w_what + w_n + w_s + 2 * g.len();
    out.push_str(&format!("{:<ww$}{g}{:>wn$}{g}{:>ws$}\n", "What", "Versions", "Size", ww = w_what, wn = w_n, ws = w_s, g = g));
    out.push_str(&"-".repeat(width));
    out.push('\n');
    for r in rows {
        let pad = w_what - r.what.chars().count();
        out.push_str(&format!("{}{}{g}{:>wn$}{g}{:>ws$}\n", r.what, " ".repeat(pad), r.versions, human_size(r.bytes), wn = w_n, ws = w_s, g = g));
    }
    if stale_dirs > 0 {
        out.push_str(&format!("{:<ww$}{g}{:>wn$}{g}{}\n", stale_label, stale_dirs, "size not readable", ww = w_what, wn = w_n, g = g));
    }
    out.push_str(&"-".repeat(width));
    out.push('\n');
    out.push_str(&format!("{:<ww$}{g}{:>wn$}{g}{:>ws$}\n", "Total", total_v, human_size(total_b), ww = w_what, wn = w_n, ws = w_s, g = g));
    out
}

/// Bytes as a person reads them: "19.0 GB", "812 MB", "4 KB". Powers of 1024,
/// the same as `du -h`.
fn human_size(b: u64) -> String {
    const K: f64 = 1024.0;
    let f = b as f64;
    if f >= K * K * K { format!("{:.1} GB", f / (K * K * K)) }
    else if f >= K * K { format!("{:.0} MB", f / (K * K)) }
    else if f >= K { format!("{:.0} KB", f / K) }
    else { format!("{} B", b) }
}

pub fn activate(source: &str) {
    set_source(source, true);
}

/// Shared body of `nog activate/deactivate <source>`. Validates the source
/// name and dispatches: "aur" flips the persisted flag in
/// /etc/nog/sources.toml; "chaotic-aur" additionally performs the
/// pacman.conf section toggle (backup first). Everything root-owned is
/// written via `sudo tee`/`sudo cp` — nog itself stays unprivileged.
fn set_source(source: &str, enable: bool) {
    match source {
        "aur" => set_aur(enable),
        "chaotic-aur" => set_chaotic(enable),
        "flatpak" => set_flatpak(enable),
        "snap" => set_snap(enable),
        other => {
            eprintln!("nog: unknown source '{}'. Valid sources: aur, chaotic-aur, flatpak, snap", other);
            end(1);
        }
    }
}

fn set_aur(enable: bool) {
    let mut state = sources::load(sources::DEFAULT_PATH);
    if state.aur == enable {
        println!(
            "nog: AUR is already {}.",
            if enable { "active" } else { "deactivated" }
        );
        return;
    }
    state.aur = enable;

    if let Err(e) = sources::save(sources::DEFAULT_PATH, &state) {
        eprintln!("nog: could not save source state: {}", e);
        end(1);
    }

    if enable {
        let cfg = load_config();
        println!("nog: AUR ACTIVATED — saved to {}.", sources::DEFAULT_PATH);
        println!("     Helper setting '{}' (nog.conf) is back in service.", cfg.aur.helper);
    } else {
        println!("nog: AUR DEACTIVATED — saved to {}.", sources::DEFAULT_PATH);
        println!("     • `nog update` will not query or install AUR updates;");
        println!("       the handoff runs through pacman, so foreign packages cannot move.");
        println!("     • `nog install` routes through pacman only — AUR-only packages");
        println!("       will not resolve until reactivated.");
        println!("     Re-enable with: nog activate aur");
    }
}

/// v1.1.0 (C1): flip the flatpak flag in sources.toml. Pure state — flatpak
/// itself stays installed and untouched; nog simply stops (or resumes)
/// querying and applying flatpak updates. The on-demand install offer for a
/// MISSING flatpak binary belongs to the install chain (C3), not here.
fn set_flatpak(enable: bool) {
    let mut state = sources::load(sources::DEFAULT_PATH);
    if state.flatpak == enable {
        println!(
            "nog: flatpak is already {}.",
            if enable { "active" } else { "deactivated" }
        );
        return;
    }
    state.flatpak = enable;

    if let Err(e) = sources::save(sources::DEFAULT_PATH, &state) {
        eprintln!("nog: could not save source state: {}", e);
        end(1);
    }

    if enable {
        println!("nog: flatpak ACTIVATED — saved to {}.", sources::DEFAULT_PATH);
        if !flatpak::is_available() {
            println!("     Note: the flatpak binary is not installed — the source stays");
            println!("     dormant until it is (nog can install it on demand: C3).");
        }
    } else {
        println!("nog: flatpak DEACTIVATED — saved to {}.", sources::DEFAULT_PATH);
        println!("     • `nog update` will not query or apply flatpak updates;");
        println!("       installed flatpak apps stay installed but frozen.");
        println!("     Re-enable with: nog activate flatpak");
    }
}

/// v1.2.0 (C2): flip the snap flag in sources.toml. Same pure-state shape as
/// flatpak — snapd itself is never installed, removed, or stopped by nog.
fn set_snap(enable: bool) {
    let mut state = sources::load(sources::DEFAULT_PATH);
    if state.snap == enable {
        println!(
            "nog: snap is already {}.",
            if enable { "active" } else { "deactivated" }
        );
        return;
    }
    state.snap = enable;

    if let Err(e) = sources::save(sources::DEFAULT_PATH, &state) {
        eprintln!("nog: could not save source state: {}", e);
        end(1);
    }

    if enable {
        println!("nog: snap ACTIVATED — saved to {}.", sources::DEFAULT_PATH);
        if !snap::is_available() {
            println!("     Note: snapd is not installed — the source stays dormant");
            println!("     until it is. snapd lives on the AUR; nog can install it on");
            println!("     demand (C3), or `yay -S snapd` + `systemctl enable --now snapd.socket`.");
        }
    } else {
        println!("nog: snap DEACTIVATED — saved to {}.", sources::DEFAULT_PATH);
        println!("     • `nog update` will not query or refresh snaps;");
        println!("       installed snaps stay installed but frozen.");
        println!("     Re-enable with: nog activate snap");
    }
}

/// v1.0.9 (A3): toggle the [chaotic-aur] section of pacman.conf. The repo
/// definition IS the gate — once commented out, no tool on the system
/// (pacman, helpers, libalpm GUIs) can resolve from chaotic-aur. Installed
/// chaotic packages stay installed but sit frozen: no repo, no updates.
fn set_chaotic(enable: bool) {
    let cfg = load_config();
    let conf_path = &cfg.paths.pacman_conf;
    let text = match std::fs::read_to_string(conf_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("nog: could not read {}: {}", conf_path, e);
            end(1);
        }
    };

    match sources::toggle_repo_section(&text, "chaotic-aur", enable) {
        sources::RepoToggle::NotFound => {
            eprintln!("nog: no [chaotic-aur] section found in {} (neither active nor nog-disabled).", conf_path);
            if enable {
                eprintln!("     Nothing to restore — if you want chaotic-aur, add the repo per its docs first.");
            }
            end(1);
        }
        sources::RepoToggle::AlreadyInState => {
            println!(
                "nog: chaotic-aur is already {}.",
                if enable { "active" } else { "deactivated" }
            );
        }
        sources::RepoToggle::Changed(new_text) => {
            // 1. Timestamped backup of pacman.conf before we touch it.
            let stamp = timestamp_for_backup();
            let backup = format!("{}.nog-bak-{}", conf_path, stamp);
            let cp = crate::machine::sudo()
                .args(["cp", "--preserve=all", conf_path, &backup])
                .status();
            match cp {
                Ok(s) if s.success() => {}
                _ => {
                    eprintln!("nog: could not back up {} — aborting without changes.", conf_path);
                    end(1);
                }
            }

            // 2. Write the toggled pacman.conf.
            if let Err(e) = crate::tiers::write_as_root(conf_path, &new_text) {
                eprintln!("nog: could not write {}: {}", conf_path, e);
                eprintln!("     your original is safe at {}", backup);
                end(1);
            }

            // 3. Mirror the state in sources.toml (informational — pacman.conf
            //    is the enforcing artifact for this source).
            let mut state = sources::load(sources::DEFAULT_PATH);
            state.chaotic_aur = enable;
            if let Err(e) = sources::save(sources::DEFAULT_PATH, &state) {
                eprintln!("nog: warning — pacman.conf updated but sources.toml not saved: {}", e);
            }

            if enable {
                println!("nog: chaotic-aur ACTIVATED.");
                println!("     • [chaotic-aur] restored in {} (backup: {})", conf_path, backup);
            } else {
                println!("nog: chaotic-aur DEACTIVATED.");
                println!("     • [chaotic-aur] commented out in {} (backup: {})", conf_path, backup);
                println!("     • installed chaotic packages stay installed but frozen — no repo, no updates.");
                println!("     Re-enable with: nog activate chaotic-aur");
            }

            // 4. Refresh the sync DBs so the change takes effect immediately.
            println!();
            println!("nog: refreshing package databases ...");
            let status = pacman::run(&["-Sy"]);
            if !status.success() {
                eprintln!("nog: warning — database refresh failed; run `sudo pacman -Sy` manually.");
            }
        }
    }
}

/// Timestamp for backup filenames via the system `date` (the runlog
/// precedent — no chrono dependency). Falls back to a constant suffix if
/// `date` is unavailable; a stable-named backup beats no backup.
fn timestamp_for_backup() -> String {
    std::process::Command::new("date")
        .arg("+%Y%m%d-%H%M%S")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "undated".to_string())
}

/// Why this package landed in the Ready bucket. Distinguishes the normal
/// "hold window passed" case from the `--realign` override that pulled a held
/// kernel into Ready to recover from a kernel/headers version mismatch.
#[derive(Clone)]
enum ReadyReason {
    Expired { days_past_window: u64 },
    Realigned,
    /// v1.5.6 (#31): a keyring package — never held.
    Keys,
    /// v1.6.0 (#27): released after newer builds kept arriving during the
    /// hold. Before v1.6.0 this package would still be waiting.
    AfterWaiting(Waited),
    /// v1.6.0 (#7): you promoted it (`--promote`, nogForge's Promote): ready
    /// now, installed with the rest. `Some(p)`: it moved with `p`, which you
    /// promoted, because the coupling rules say they go together.
    Promoted(Option<String>),
}

/// v1.6.0 (#27): how long an update has been waiting, and how many new
/// versions came and went meanwhile. Only carried when at least one did.
#[derive(Clone, Debug, PartialEq)]
struct Waited {
    since: String,
    skipped: usize,
}

impl Waited {
    fn describe(&self) -> String {
        let v = if self.skipped == 1 { "version" } else { "versions" };
        format!("waiting since {} · {} newer {} skipped", self.since, self.skipped, v)
    }
}

/// Why this package landed in the Held bucket. Drives the reason string shown in
/// the Held listing.
#[derive(Clone)]
enum HeldReason {
    /// Normal hold — the tier's window is still open (`days_remaining` left).
    Window,
    /// v1.6.0 (#27): the window is still open, and newer builds have arrived
    /// meanwhile. They no longer restart it; the row says so.
    Waiting(Waited),
    /// v1.6.0 (#27): the window is over, but the newest build is younger than
    /// the tier's safety wait (`days_remaining` left on that).
    SafetyWait(Option<Waited>),
    /// Expert-mode `manual_signoff = true` on a Tier 1 package. Released with
    /// `nog unlock`.
    ManualSignoff,
    /// v1.0.6 (issue #1): held only because coupling this `lib32-<X>`/base `<X>`
    /// pair keeps a version-locked multilib package from splitting across
    /// buckets. Carries the partner it is waiting on. Its own window may already
    /// have expired; the countdown shown is the partner's.
    CoupledTo(String),
    /// v1.6.0 (#7): you kept it back (`--keep`, nogForge's unticked box). Its
    /// coupled partners then follow it through the same coupling rules.
    KeptBack,
    /// v2.0.0 (C4): locked with `nog lock` — never updated until `nog unlock`.
    Locked,
    /// v2.0.0 (#5): built after a held linchpin's new series (glibc 2.45,
    /// gcc-libs 16) and so probably linked against it; released with it.
    BuiltAfter(String),
}

/// v1.5.6 (#31): the keyring packages. Holding them back is itself the
/// breakage — every later signature check fails until the new keys land — so
/// they are never held and are installed before the rest.
const KEYRINGS: [&str; 2] = ["archlinux-keyring", "chaotic-keyring"];

/// How a run of the update ended.
#[derive(Debug, PartialEq)]
enum UpdateEnd {
    /// pacman ran, or nothing was pending.
    Done,
    /// v2.1.0 (F-22): the update ran, and a later step (the AUR, Flatpak, Snap) did not
    /// complete. Exits with `INCOMPLETE`, so a script, the installer and nogForge can tell.
    Incomplete,
    /// Cancelled, or stopped before the end (a plan for nogForge).
    Stopped,
    /// v1.5.7 (F-4, #36): an error ended the run; carries the exit status.
    Failed(i32),
}

/// v2.1.0 (F-22, 2026-10-22): the exit status of an update that ran but left a step
/// incomplete. Until now it exited 0 and its run line said "done": a snapd build whose
/// password timed out read as a clean update, and nogForge showed a green "done".
pub const INCOMPLETE: i32 = 3;

/// v2.1.0 (F-20, #56): `nog update --yes`, remembered for the whole run.
static UPDATE_YES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn update_yes() -> bool {
    UPDATE_YES.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn update(realign: bool, yes: bool) {
    UPDATE_YES.store(yes, std::sync::atomic::Ordering::SeqCst);
    match run_update(realign) {
        UpdateEnd::Failed(code) => end(code),
        UpdateEnd::Incomplete => end(INCOMPLETE),
        UpdateEnd::Done | UpdateEnd::Stopped => {}
    }
}

/// v2.1.0 (F-22): the word a run's line in the runs log carries for its exit status.
fn outcome_word(code: i32) -> &'static str {
    match code {
        0 => "done",
        INCOMPLETE => "incomplete",
        _ => "stopped",
    }
}

/// v1.5.8: the run being framed — what was asked, when, by whom, and the
/// log files it wrote. Javier's rule (2 Oct 2026): every nog run starts with
/// nog's version and the run requested, and ends with where it was logged
/// and a thank-you — whatever the command. `begin` prints the start; every
/// way out of a command goes through `end`, so no command can skip the end.
struct RunFrame {
    label: String,
    date: String,
    time: String,
    user: String,
    logs: Vec<std::path::PathBuf>,
}

static RUN: std::sync::Mutex<Option<RunFrame>> = std::sync::Mutex::new(None);

/// Print the start of a run and remember it for `end`.
pub fn begin(label: &str) {
    let (date, time) = now_date_time();
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "unknown".to_string());
    println!("{}", format_banner(label, &date, &time, &user));
    if let Ok(mut r) = RUN.lock() {
        *r = Some(RunFrame { label: label.to_string(), date, time, user, logs: Vec::new() });
    }
}

/// v1.5.8: the start of every run, after the `nog update` banner it grew
/// out of: nog's version and what the run is, framed, then the run as typed
/// and when and by whom. Ends with the line break before the blank line the
/// caller's println adds. Plain `=` rules: they draw on a text console too.
fn format_banner(label: &str, date: &str, time: &str, user: &str) -> String {
    let what = label.split_whitespace().next().unwrap_or("");
    let mut title = what.to_string();
    if let Some(c) = title.get(0..1) {
        title = c.to_uppercase() + &title[1..];
    }
    let heading = if title.is_empty() {
        format!("nog v{}", env!("CARGO_PKG_VERSION"))
    } else {
        format!("nog v{}  ·  {}", env!("CARGO_PKG_VERSION"), title)
    };
    let run = format!("Run:   nog {}", label);
    let width = rule_width(&heading, &run);
    let rule = format!("{}{}{}{}", C_BOLD, C_MAUVE, "=".repeat(width), C_RESET);
    format!("\n{rule}\n{b}  {heading}{r}\n{rule}\n  {run}\n  Date:  {date}   {time}\n  User:  {user}\n",
        rule = rule, b = C_BOLD, r = C_RESET, heading = heading, run = run, date = date, time = time, user = user)
}

/// v1.7.0 (Javier's desktop test, 4 Oct 2026): the banner's rules fit the
/// heading and the run line, but never pass 80 columns. Four file paths in one
/// `nog install` made them ~330 characters; a long run line now simply runs
/// past the rule.
const RULE_MAX: usize = 80;

fn rule_width(heading: &str, run: &str) -> usize {
    [41, heading.chars().count() + 4, run.chars().count() + 4]
        .into_iter().max().unwrap_or(41).min(RULE_MAX)
}

/// A log file this run wrote, for the end of the run to name.
fn record_log(path: std::path::PathBuf) {
    if let Ok(mut r) = RUN.lock() {
        if let Some(f) = r.as_mut() {
            if !f.logs.contains(&path) {
                f.logs.push(path);
            }
        }
    }
}

/// The end of every run: one line in the day's runs log, then the closing
/// notice (what was logged where, and thanks), then exit with `code`.
pub fn end(code: i32) -> ! {
    use std::io::Write;
    static ENDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if ENDING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        std::process::exit(code);
    }
    crate::events::end(code);
    let frame = RUN.lock().ok().and_then(|mut r| r.take());
    let Some(mut f) = frame else {
        std::process::exit(code);
    };
    let outcome = outcome_word(code);
    let dir = NogConfig::load_default().paths.run_logs;
    let mut problem = None;
    match runlog::today_and_cutoff() {
        Some((today, _)) => {
            let status = code.to_string();
            match runlog::append_runs_row(&dir, &today,
                &[&f.date, &f.time, &f.user, &f.label, &status, outcome]) {
                Ok(path) => {
                    if !f.logs.contains(&path) {
                        f.logs.insert(0, path);
                    }
                }
                Err(e) => problem = Some(format!("This run could not be logged: {}", e)),
            }
        }
        None => problem = Some("This run could not be logged: `date` is unavailable.".to_string()),
    }
    // v1.6.0: the whole run, as on screen (record.rs), is the log a person
    // reads; the CSVs are nog's own bookkeeping (hold clock, nogForge's
    // tables) and stay unnamed (Javier, 3 Oct: "option A"). Unrecorded runs
    // (no terminal) still name their CSVs.
    if let Some(p) = crate::record::current() {
        f.logs = vec![p];
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    if let Some(p) = problem {
        lines.push(p);
    }
    if !f.logs.is_empty() {
        lines.push("Logged in:".to_string());
        for l in &f.logs {
            let shown = l.display().to_string();
            let shown = match shown.strip_prefix(&home) {
                Some(rest) if !home.is_empty() => format!("~{}", rest),
                _ => shown,
            };
            lines.push(format!("  {}", shown));
        }
    }
    lines.push("Thank you for using nog!".to_string());
    let heading = if code == 0 {
        format!("Done · {}", f.label)
    } else {
        format!("Stopped · {} (status {})", f.label, code)
    };
    notice(if code == 0 { C_GREEN } else { C_PEACH }, &heading, &lines);
    let _ = std::io::stdout().flush();
    std::process::exit(code);
}

/// v1.5.8 (F-7, #39): the one shape for a message meant for a person.
/// Javier's rule (2 Oct 2026): exactly one blank line before and one after,
/// a heading that stands out in a fast-moving screen, the explanation
/// indented under it. Every message goes through here so the form cannot
/// drift one message at a time. Callers print nothing blank around it.
fn notice(colour: &str, heading: &str, lines: &[String]) {
    println!("{}", format_notice(colour, heading, lines));
}

/// `notice`, on stderr — for a stop or a failure.
fn enotice(colour: &str, heading: &str, lines: &[String]) {
    eprintln!("{}", format_notice(colour, heading, lines));
}

/// Pure: the text of a notice, leading blank line included; the trailing
/// blank line comes from the caller's println.
fn format_notice(colour: &str, heading: &str, lines: &[String]) -> String {
    let mut out = format!("\n{}{}==> {}{}\n", C_BOLD, colour, heading, C_RESET);
    for l in lines {
        out.push_str(&format!("    {}\n", l));
    }
    out
}

/// v1.5.7 (F-2, #34): checkupdates needs fakeroot, which pacman-contrib only
/// lists as optional. Name the missing piece when that is why it failed.
/// `-Syu`, not `-S`: on lists this old a lone `-S` is a partial upgrade.
fn checkupdates_hint(msg: &str) -> Option<&'static str> {
    if msg.contains("fakeroot") {
        Some("     checkupdates needs `fakeroot`. Install it together with a full update:\n       sudo pacman -Syu fakeroot")
    } else {
        None
    }
}

/// v1.5.6 (#31): set pacman's key store up when it has never been set up.
fn ensure_keystore() -> bool {
    if pacman::keystore_ready() {
        return true;
    }
    println!();
    println!("{}nog: pacman's key store has not been set up on this computer yet.{}", C_BOLD, C_RESET);
    println!("     Without it no package's signature can be checked. Setting it up now");
    println!("     (pacman-key --init, then --populate) — this asks for your password.");
    if !pacman::init_keystore() {
        eprintln!("nog: setting up the key store did not complete — stopping. Run:");
        eprintln!("       sudo pacman-key --init && sudo pacman-key --populate");
        return false;
    }
    println!("nog: key store ready.");
    true
}

/// The update itself.
fn run_update(realign: bool) -> UpdateEnd {
    crate::events::start("check", "Checking for updates");
    let cfg = load_config();
    let helper = resolve_helper(&cfg);
    guard_not_sudo_with_helper(helper);
    let tm = load_tiers();

    let (run_date, run_time, run_user) = print_update_header(false);
    if !ensure_keystore() {
        return UpdateEnd::Failed(1);
    }
    println!("nog: Checking for pending updates ...");
    let mut pending = match pacman::checkupdates_capture() {
        Ok(list) => list,
        Err(CheckUpdatesError::Missing) => {
            eprintln!("nog: `checkupdates` not found. Please install `pacman-contrib`:");
            eprintln!("       sudo pacman -S pacman-contrib");
            return UpdateEnd::Failed(1);
        }
        Err(CheckUpdatesError::Other(msg)) => {
            eprintln!("nog: checkupdates failed: {}", msg);
            if let Some(hint) = checkupdates_hint(&msg) {
                eprintln!("{}", hint);
            }
            return UpdateEnd::Failed(1);
        }
    };
    let official_count = pending.len();

    // Fold AUR pending upgrades into the same list when a helper is configured.
    // We track which names came from AUR so we can look up their build dates
    // via the helper's cached metadata below.
    let mut aur_names: Vec<String> = Vec::new();
    let mut aur_count = 0usize;
    // v1.6.0 (#27): which sources answered this run. The hold record only
    // forgets a package when its own source said it is no longer pending.
    let mut checked_sources: std::collections::HashSet<&str> = ["pacman"].into_iter().collect();
    if let Some(h) = helper {
        match aur::pending_updates(h) {
            Ok(aur_list) => {
                checked_sources.insert("aur");
                aur_count = aur_list.len();
                for u in &aur_list {
                    aur_names.push(u.name.clone());
                }
                pending.extend(aur_list);
            }
            Err(e) => {
                eprintln!("nog: warning — could not query AUR updates from {}: {}", h, e);
                eprintln!("     proceeding with official repo updates only; the foreign fence");
                eprintln!("     will shield ALL foreign packages from this run's handoff.");
            }
        }
    }

    // v1.1.0 (C1): fold flatpak pending updates into the same list. The
    // source is active by default; missing binary = dormant, query failure =
    // fail CLOSED for this source (report + skip apply; nothing assumed quiet).
    let flatpak_active = sources::load(sources::DEFAULT_PATH).flatpak;
    let mut flatpak_names: Vec<String> = Vec::new();
    let mut flatpak_dates: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut flatpak_count = 0usize;
    let flatpak_present = flatpak_active && flatpak::is_available();
    if flatpak_present {
        match flatpak::pending_updates() {
            Ok(fp_list) => {
                checked_sources.insert("flatpak");
                flatpak_count = fp_list.len();
                let installed = flatpak::installed_versions();
                flatpak_dates = flatpak::commit_dates_for(&fp_list);
                for u in &fp_list {
                    flatpak_names.push(u.app_id.clone());
                    pending.push(flatpak::to_pending(u, &installed));
                }
            }
            Err(e) => {
                eprintln!("nog: warning — could not query flatpak updates: {}", e);
                eprintln!("     proceeding without flatpak; nothing flatpak will be touched this run.");
            }
        }
    }

    // v1.2.0 (C2): snap, same pattern as flatpak. snapd is AUR-only on Arch,
    // so absence is normal and silent — never an error (ruling #4).
    let snap_active = sources::load(sources::DEFAULT_PATH).snap;
    let mut snap_names: Vec<String> = Vec::new();
    let mut snap_dates: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut snap_count = 0usize;
    let snap_present = snap_active && snap::is_available();
    if snap_present {
        match snap::pending_updates() {
            Ok(sn_list) => {
                checked_sources.insert("snap");
                snap_count = sn_list.len();
                let installed = snap::installed_versions();
                snap_dates = snap::publish_dates_for(&sn_list);
                for u in &sn_list {
                    snap_names.push(u.name.clone());
                    pending.push(snap::to_pending(u, &installed));
                }
            }
            Err(e) => {
                eprintln!("nog: warning — could not query snap updates: {}", e);
                eprintln!("     proceeding without snap; nothing snap will be touched this run.");
            }
        }
    }

    // v1.5.4 (#28): the per-source counts are the SUMMARY table now, printed
    // once the buckets are final. Here we only note how each source took part,
    // so a failed query reads "could not check", never 0 (#25).
    let _ = (official_count, aur_count, flatpak_count, snap_count);
    let switches = sources::load(sources::DEFAULT_PATH);
    let presence = |active: bool, installed: bool, checked: bool| match (active, installed, checked) {
        (false, true, _) => Presence::Off,
        (_, false, _) => Presence::Absent,
        (true, true, true) => Presence::Checked,
        (true, true, false) => Presence::Failed,
    };
    let source_presence = SourcePresence {
        aur: match helper {
            Some(_) => if checked_sources.contains("aur") { Presence::Checked } else { Presence::Failed },
            None if !switches.aur => Presence::Off,
            None => Presence::Absent,
        },
        flatpak: presence(flatpak_active, flatpak_present || (!flatpak_active && flatpak::is_available()), checked_sources.contains("flatpak")),
        snap: presence(snap_active, snap_present || (!snap_active && snap::is_available()), checked_sources.contains("snap")),
        chaotic_off: !switches.chaotic_aur,
    };

    if pending.is_empty() {
        let rows = summary_rows(&sync_db::repo_order(), &HashMap::new(), &[], &[], &[], &source_presence);
        println!();
        print!("{}", format_summary(&rows, true));
        println!();
        println!("nog: System is up to date — nothing to do.");
        write_run_log(&cfg, &run_date, &run_time, &run_user, Vec::new(), "up to date");
        return UpdateEnd::Done;
    }

    // v1.0.5: evaluate holds against the SAME database snapshot that produced
    // the candidate list. `checkupdates` syncs fresh DBs into its private
    // dbpath; the system DB at /var/lib/pacman/sync only refreshes when root
    // syncs — for `nog update`, during the handoff AFTER this report. Reading
    // the system DB dated every first-sighting update from its predecessor's
    // builddate (years old in the worst case) and waved it through its window
    // — the 2026-07-06 finding: all 14 "Ready" packages that day were 1-4
    // days old and belonged in Held.
    let mut packages = match sync_db::load_fresh_packages() {
        Some(p) => p,
        None => {
            eprintln!("nog: warning — checkupdates DB not found; using the system sync DB.");
            eprintln!("     Hold windows may be dated from stale build dates.");
            sync_db::load_packages().clone()
        }
    };

    // Then extend with AUR build dates fetched via the helper's cached metadata
    // (`<helper> -Sai`). Only query for AUR names that weren't already resolved
    // by the sync-DB pass. If the helper is unreachable or the date is
    // unparseable, those packages fall back to the Unknown bucket — the
    // per-package y/N prompt still handles them cleanly. AUR entries carry no
    // version, so they skip the candidate-version guard.
    if let Some(h) = helper {
        let missing: Vec<String> = aur_names.iter()
            .filter(|name| !packages.contains_key(name.as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            for (name, builddate) in aur::build_dates_for(h, &missing) {
                packages.insert(name, sync_db::PackageDesc {
                    builddate,
                    pkgbase: None,
                    version: None,
                    provides: Vec::new(),
                    repo: None,
                });
            }
        }
    }

    // Flatpak commit dates → the same builddate map the hold windows read.
    // Apps whose date could not be resolved stay absent → Unknown bucket.
    for (app_id, ts) in &flatpak_dates {
        packages.entry(app_id.clone()).or_insert(sync_db::PackageDesc {
            builddate: *ts,
            pkgbase: None,
            version: None,
            provides: Vec::new(),
            repo: None,
        });
    }

    // Snap publish dates → the same builddate map.
    for (name, ts) in &snap_dates {
        packages.entry(name.clone()).or_insert(sync_db::PackageDesc {
            builddate: *ts,
            pkgbase: None,
            version: None,
            provides: Vec::new(),
            repo: None,
        });
    }

    let now = std::time::SystemTime::now();

    // v1.6.0 (#27): bring the hold record up to date, so each window is
    // clocked from the first time an update was seen, not the newest build.
    let hold_record = update_hold_record(&cfg, &pending, &packages, &checked_sources, now);

    // Evaluate every pending update and bucket it.
    let mut ready: Vec<(PendingUpdate, Tier, ReadyReason)> = Vec::new();
    let mut held: Vec<(PendingUpdate, Tier, u64, HeldReason)> = Vec::new(); // (upd, tier, days_remaining, reason)
    let mut unknown: Vec<(PendingUpdate, Tier)> = Vec::new();

    for upd in &pending {
        let tier = tm.classify(&upd.name);
        let record = hold_record.get(&sightings::key(upd.source.as_str(), &upd.name));
        let status = holds::evaluate_candidate(
            &upd.name,
            tier.clone(),
            &upd.new_version,
            &packages,
            &cfg.holds,
            record.map(|r| r.first_seen),
            now,
        );
        // Shown on the row when newer builds have come and gone (#27).
        let history = record.filter(|r| r.skipped() > 0).map(|r| Waited {
            since: sightings::short_date(r.first_seen),
            skipped: r.skipped(),
        });

        // v1.5.6 (#31): keys are never held — not by date, not by sign-off.
        if upd.source == Source::Pacman && KEYRINGS.contains(&upd.name.as_str()) {
            ready.push((upd.clone(), tier, ReadyReason::Keys));
            continue;
        }

        // Expert-mode override: `manual_signoff = true` on Tier 1 forces every
        // Tier 1 package into the held bucket regardless of date. Escape hatch
        // is `nog unlock <pkg> --promote`.
        let signoff_hold = tm.is_manual_signoff(&upd.name);

        match status {
            _ if signoff_hold => {
                // Report 0 days remaining as a placeholder; the UI shows the
                // "manual sign-off" reason instead of a countdown.
                held.push((upd.clone(), tier, 0, HeldReason::ManualSignoff));
            }
            HoldStatus::Expired { days_past_window } => match history {
                Some(w) => ready.push((upd.clone(), tier, ReadyReason::AfterWaiting(w))),
                None => ready.push((upd.clone(), tier, ReadyReason::Expired { days_past_window })),
            },
            HoldStatus::Holding { days_remaining } => match history {
                Some(w) => held.push((upd.clone(), tier, days_remaining, HeldReason::Waiting(w))),
                None => held.push((upd.clone(), tier, days_remaining, HeldReason::Window)),
            },
            HoldStatus::SafetyWait { days_remaining } => {
                held.push((upd.clone(), tier, days_remaining, HeldReason::SafetyWait(history)));
            }
            HoldStatus::Unknown => {
                unknown.push((upd.clone(), tier));
            }
        }
    }

    // v2.0.0 (C4): a locked package is held, whatever its window says, and is
    // never promoted. An unreadable lock file fails closed — nothing is treated
    // as locked — and says so.
    let locked_names: Vec<String> = match crate::locks::load(crate::locks::DEFAULT_PATH) {
        Ok(l) => l,
        Err(e) => { eprintln!("nog: warning — {} (no package is treated as locked this run)", e); Vec::new() }
    };
    if !locked_names.is_empty() {
        let mut kept_ready = Vec::new();
        for entry in ready.drain(..) {
            if locked_names.contains(&entry.0.name) { held.push((entry.0.clone(), entry.1.clone(), 0, HeldReason::Locked)); } else { kept_ready.push(entry); }
        }
        ready = kept_ready;
        let mut kept_unknown = Vec::new();
        for entry in unknown.drain(..) {
            if locked_names.contains(&entry.0.name) { held.push((entry.0.clone(), entry.1.clone(), 0, HeldReason::Locked)); } else { kept_unknown.push(entry); }
        }
        unknown = kept_unknown;
        for h in held.iter_mut() {
            if locked_names.contains(&h.0.name) && !matches!(h.3, HeldReason::Locked) { h.2 = 0; h.3 = HeldReason::Locked; }
        }
    }
    // F-17 (rc2): a lock with nothing pending is not in any table; name it
    // below the tables so a lock never goes invisible.
    let idle_locks: Vec<String> = {
        let in_plan: Vec<String> = ready.iter().map(|e| e.0.name.clone())
            .chain(held.iter().map(|h| h.0.name.clone()))
            .chain(unknown.iter().map(|e| e.0.name.clone())).collect();
        crate::locks::idle(&locked_names, &in_plan)
    };
    // v2.0.0 (#5): the linchpin rule. While glibc or gcc-libs is held with a
    // pending series change, repository packages built after that new build
    // move to Held with it (they were probably linked against it). A same-series
    // rebuild moves nothing. `--promote` on a package overrides, as everywhere.
    {
        let built = |u: &PendingUpdate, remaining: u64| holds::Built {
            name: u.name.clone(), old_version: u.old_version.clone(), new_version: u.new_version.clone(),
            built: packages.get(&u.name).map(|d| d.builddate).unwrap_or(0), remaining,
        };
        let r: Vec<holds::Built> = ready.iter().filter(|(u, _, _)| u.source == Source::Pacman).map(|(u, _, _)| built(u, 0)).collect();
        let h: Vec<holds::Built> = held.iter().filter(|(u, _, _, _)| u.source == Source::Pacman).map(|(u, _, rem, _)| built(u, *rem)).collect();
        let demotions = holds::linchpin_demotions(&r, &h);
        if !demotions.is_empty() {
            let promote = crate::machine::promote();
            let mut kept = Vec::new();
            for entry in ready.drain(..) {
                match demotions.iter().find(|(n, _, _)| n == &entry.0.name) {
                    Some((_, linchpin, rem)) if !promote.contains(&entry.0.name) =>
                        held.push((entry.0.clone(), entry.1.clone(), *rem, HeldReason::BuiltAfter(linchpin.clone()))),
                    _ => kept.push(entry),
                }
            }
            ready = kept;
        }
    }

    // v1.6.0 (#7): what you chose to keep back is held before the coupling
    // rules run, so they decide what must stay back with it — the answer
    // nogForge shows under an unticked box. Keys are never kept back (#31).
    let mut keep = crate::machine::keep();
    // v1.6.1: `nog update a b c` names what goes in, on purpose (Javier, 4 Oct
    // 2026: "a specific list, it's intentional"). Everything else that is ready
    // is kept back this time, so the coupling rules below still say what must
    // move together with what you named — and the handoff is the same safe one.
    let only = crate::machine::only();
    if !only.is_empty() {
        let promote = crate::machine::promote();
        keep.extend(ready.iter().map(|(u, _, _)| &u.name)
            .chain(unknown.iter().map(|(u, _)| &u.name))
            .filter(|n| !only.contains(n) && !promote.contains(n))
            .cloned());
    }
    let keep = keep;
    if !keep.is_empty() {
        let wanted = |u: &PendingUpdate| keep.iter().any(|k| k == &u.name)
            && !(u.source == Source::Pacman && KEYRINGS.contains(&u.name.as_str()));
        let (kept, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut ready).into_iter().partition(|(u, _, _)| wanted(u));
        ready = rest;
        for (u, t, _) in kept {
            held.push((u, t, 0, HeldReason::KeptBack));
        }
        let (kept, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut unknown).into_iter().partition(|(u, _)| wanted(u));
        unknown = rest;
        for (u, t) in kept {
            held.push((u, t, 0, HeldReason::KeptBack));
        }
    }

    // v1.6.0 (#7): what you promoted is ready now, before the coupling rules
    // run; they pull its partners along instead of pulling it back (below).
    let promote: Vec<String> = crate::machine::promote().into_iter().filter(|p| !keep.contains(p)).collect();
    let mut promoted: std::collections::HashSet<String> = std::collections::HashSet::new();
    if !promote.is_empty() {
        let (up, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut held).into_iter()
            .partition(|(u, _, _, r)| promote.iter().any(|p| p == &u.name) && !matches!(r, HeldReason::KeptBack | HeldReason::Locked));
        held = rest;
        for (u, t, _, _) in up {
            promoted.insert(u.name.clone());
            ready.push((u, t, ReadyReason::Promoted(None)));
        }
        let (up, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut unknown).into_iter()
            .partition(|(u, _)| promote.iter().any(|p| p == &u.name));
        unknown = rest;
        for (u, t) in up {
            promoted.insert(u.name.clone());
            ready.push((u, t, ReadyReason::Promoted(None)));
        }
    }

    // Desync detection: for each Tier 1 package that is installed, check
    // whether its <X>-headers companion is installed at a *different* version.
    // That's the post-incident fingerprint of the 2026-05-13 nvidia breakage —
    // headers raced ahead of the held kernel and the next DKMS rebuild errored
    // with "Missing <KVER> kernel modules tree."
    let kernel_names = tm.tier1_packages();
    let mut to_query: Vec<String> = kernel_names.clone();
    to_query.extend(kernel_names.iter().map(|k| format!("{}-headers", k)));
    let installed = pacman::installed_versions(&to_query);

    let mut desyncs: Vec<(String, String, String)> = Vec::new(); // (kernel, kver, hver)
    for k in &kernel_names {
        let kver = match installed.get(k) { Some(v) => v, None => continue };
        let hpkg = format!("{}-headers", k);
        let hver = match installed.get(&hpkg) { Some(v) => v, None => continue };
        if kver != hver {
            desyncs.push((k.clone(), kver.clone(), hver.clone()));
        }
    }

    if !desyncs.is_empty() {
        println!();
        println!("{}{}nog: ⚠ kernel / headers version mismatch detected:{}", C_BOLD, C_RED, C_RESET);
        for (k, kver, hver) in &desyncs {
            println!("       {:<22} {}", k, kver);
            println!("       {:<22} {}", format!("{}-headers", k), hver);
        }
        println!("{}     DKMS rebuilds against the newer headers will fail because the{}",
            C_SUBTEXT, C_RESET);
        println!("{}     kernel modules tree for that version isn't installed.{}",
            C_SUBTEXT, C_RESET);

        if realign {
            // Forward path: pull each desynced kernel out of the Held bucket
            // when its pending upgrade version matches the installed headers
            // version. The transaction will then upgrade kernel-to-match-headers
            // in a single coherent step and the next DKMS rebuild succeeds.
            let mut new_held: Vec<(PendingUpdate, Tier, u64, HeldReason)> = Vec::new();
            let mut realigned_count = 0usize;
            for entry in held.drain(..) {
                let (upd, tier, _, _) = &entry;
                let matched = desyncs.iter().any(|(k, _, hver)| {
                    &upd.name == k && &upd.new_version == hver
                });
                if matched {
                    println!("{}     --realign: {} {} → {} pulled into Ready.{}",
                        C_SUBTEXT, upd.name, upd.old_version, upd.new_version, C_RESET);
                    ready.push((upd.clone(), tier.clone(), ReadyReason::Realigned));
                    realigned_count += 1;
                } else {
                    new_held.push(entry);
                }
            }
            held = new_held;
            if realigned_count == 0 {
                println!("{}     --realign: no held kernel matches the installed headers version{}",
                    C_SUBTEXT, C_RESET);
                println!("{}     (recovery may require `sudo pacman -U` from the cache instead).{}",
                    C_SUBTEXT, C_RESET);
            }
        } else {
            println!("{}     To recover, re-run with `--realign`:{}", C_SUBTEXT, C_RESET);
            println!("{}         nog update --realign{}", C_SUBTEXT, C_RESET);
            println!("{}     This pulls held kernels into the upgrade so they match the headers.{}",
                C_SUBTEXT, C_RESET);
        }
    }

    // v1.0.6 (issue #1) / v1.2.1 (issue #11): couple split package families.
    //
    // Version-locked packages have their hold windows dated independently, so
    // they can expire on different days and land in opposite buckets. Releasing
    // half a family either aborts the transaction (when pacman can see the
    // lockstep, via a versioned `=` dep) or — far worse — succeeds and leaves a
    // broken system (when it cannot: the Qt6 stack shares no pkgbase and carries
    // no version constraint, and splitting it took a desktop to a black screen).
    //
    // Four rules now apply — lib32 pairs, pkgbase siblings, version cohorts, and
    // (v1.3.1, issue #13) dropped sonames; see `holds::coupling_demotions`. Runs
    // last, so it sees the post-realign buckets and feeds the ignore list below.
    //
    // The fourth rule exists because the first three all match on *names*, and
    // the fourth relationship is only visible in the dependency graph: a package
    // whose hold expired can bump a shared library version out from under one
    // still inside its window, and pacman refuses the entire transaction.
    //
    // The loop is the other half of issue #11. A single pass is not enough: when
    // one rule demotes a package, that package's *own* coupling relationships are
    // then evaluated against a bucket state that no longer matches what the pass
    // was computed from. The v1.0.6 code demoted `libelf` for its lib32 partner
    // and never revisited `elfutils`, which shares libelf's pkgbase. Iterating to
    // a fixpoint makes every rule transitive, including any added later.
    {
        // Ready only ever shrinks, so this must converge; the cap turns "must"
        // into something the code actually enforces rather than assumes.
        const MAX_PASSES: usize = 16;
        let mut passes = 0;

        // v1.3.1 (issue #13): the soname rule reads the dependency graph, which
        // lives in the *local* database — what is installed — rather than the
        // sync database, which only describes what packages will become. Built
        // once and shared by every pass: it describes the installed system, and
        // nothing installs while nog is still deciding.
        let soname_data = {
            let installed = local_db::load_installed();
            if installed.is_empty() {
                eprintln!(
                    "{}nog: warning — the local package database could not be read;{}",
                    C_SUBTEXT, C_RESET
                );
                eprintln!(
                    "{}     soname coupling is inactive for this run. pacman will still{}",
                    C_SUBTEXT, C_RESET
                );
                eprintln!(
                    "{}     refuse a transaction that would break a dependency.{}",
                    C_SUBTEXT, C_RESET
                );
            }
            let mut data = holds::SonameData {
                new_provides: packages
                    .iter()
                    .map(|(n, d)| (n.clone(), d.provides.clone()))
                    .collect(),
                installed_provides: installed
                    .iter()
                    .map(|(n, d)| (n.clone(), d.provides.clone()))
                    .collect(),
                installed_depends: installed
                    .into_iter()
                    .map(|(n, d)| (n, d.depends))
                    .collect(),
                ..Default::default()
            };

            // v1.4.2 (issue #16): `%DEPENDS%` is only what a package admits
            // to. When an update would drop a soname, read the binaries
            // themselves for who really links it. Rare enough — about one
            // update in 130 — that the cost of opening every program on the
            // system is paid only when it can matter.
            let pending: Vec<&str> = ready
                .iter()
                .map(|(u, _, _)| u.name.as_str())
                .chain(held.iter().map(|(u, _, _, _)| u.name.as_str()))
                .collect();
            let dropped = holds::dropped_sonames(&data, &pending);
            if !dropped.is_empty() {
                println!(
                    "{}Checking installed programs for {} library version(s) this update removes...{}",
                    C_SUBTEXT,
                    dropped.len(),
                    C_RESET
                );
                let linkage = local_db::scan_linkage(&dropped);
                data.linked_by = linkage.linked_by;
                data.file_providers = linkage.file_providers;
            }

            // v2.1.0 (F-25, #61): an interpreter changing series (perl 5.42 → 5.44)
            // leaves its module folder behind; who still has files there is read
            // from the file lists, only when such a move is pending.
            let versions: Vec<(&str, &str, &str)> = ready
                .iter()
                .map(|(u, _, _)| (u.name.as_str(), u.old_version.as_str(), u.new_version.as_str()))
                .chain(held.iter().map(|(u, _, _, _)| (u.name.as_str(), u.old_version.as_str(), u.new_version.as_str())))
                .collect();
            let moves = holds::interpreter_moves(&versions);
            if !moves.is_empty() && !data.installed_provides.is_empty() {
                let dirs: Vec<String> = moves.iter().map(|(_, d)| d.clone()).collect();
                let by_dir = local_db::owners_of_dirs(&dirs);
                for (interp, dir) in moves {
                    data.module_dir_owners.insert(interp, by_dir.get(&dir).cloned().unwrap_or_default());
                }
            }
            data
        };

        loop {
            let to_coupling = |name: &str, upd: &PendingUpdate, remaining: u64| holds::CouplingPkg {
                name: name.to_string(),
                old_version: upd.old_version.clone(),
                new_version: upd.new_version.clone(),
                pkgbase: packages.get(name).and_then(|d| d.pkgbase.clone()),
                remaining,
            };
            let ready_pkgs: Vec<holds::CouplingPkg> = ready
                .iter()
                .map(|(u, _, _)| to_coupling(&u.name, u, 0))
                .collect();
            let held_pkgs: Vec<holds::CouplingPkg> = held
                .iter()
                .map(|(u, _, r, _)| to_coupling(&u.name, u, *r))
                .collect();

            let demotions = holds::coupling_demotions(&ready_pkgs, &held_pkgs, &soname_data);
            if demotions.is_empty() {
                break;
            }

            // v1.6.0 (#7): a promoted package isn't pulled back to wait for its
            // partner; the partner is promoted with it — unless you kept the
            // partner back, or it has no update to move with (then it's blocked).
            let mut pulled = false;
            for (name, partner) in &demotions {
                if !promoted.contains(name) || keep.contains(partner) {
                    continue;
                }
                if let Some(pos) = held.iter().position(|(u, _, _, _)| &u.name == partner) {
                    let (u, t, _, _) = held.remove(pos);
                    promoted.insert(u.name.clone());
                    ready.push((u, t, ReadyReason::Promoted(Some(name.clone()))));
                    pulled = true;
                }
            }
            if pulled {
                passes += 1;
                if passes < MAX_PASSES {
                    continue;
                }
            }

            let mut kept: Vec<(PendingUpdate, Tier, ReadyReason)> = Vec::new();
            for entry in ready.drain(..) {
                let (upd, tier, _) = &entry;
                match demotions.iter().find(|(name, _)| name == &upd.name) {
                    Some((_, partner)) => {
                        // Inherit the partner's remaining days so both rows show
                        // the same countdown and clear together.
                        let remaining = held
                            .iter()
                            .find(|(u, _, _, _)| &u.name == partner)
                            .map(|(_, _, r, _)| *r)
                            .unwrap_or(0);
                        held.push((
                            upd.clone(),
                            tier.clone(),
                            remaining,
                            HeldReason::CoupledTo(partner.clone()),
                        ));
                    }
                    None => kept.push(entry),
                }
            }
            ready = kept;

            passes += 1;
            if passes >= MAX_PASSES {
                // Reaching this means a rule is oscillating rather than settling.
                // Stop with the buckets as they stand — over-holding is the safe
                // direction — but say so, because it is a bug in a rule.
                eprintln!(
                    "{}nog: coupling did not converge after {} passes; \
                     proceeding with the current plan.{}",
                    C_SUBTEXT, MAX_PASSES, C_RESET
                );
                eprintln!(
                    "{}     Please report this at https://github.com/jetomev/nog/issues{}",
                    C_SUBTEXT, C_RESET
                );
                break;
            }
        }
    }

    // v1.5.6 (#31): no coupling rule may hold the keys back either.
    let (keys_held, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut held).into_iter()
        .partition(|(u, _, _, _)| u.source == Source::Pacman && KEYRINGS.contains(&u.name.as_str()));
    held = rest;
    for (u, t, _, _) in keys_held {
        ready.push((u, t, ReadyReason::Keys));
    }

    // v1.0.9 (A4, issue #6): Held reads soonest-to-release first. Ties break
    // alphabetically so the order is stable run-to-run. ManualSignoff rows
    // carry the placeholder 0 and surface at the top — they need the user's
    // attention anyway. The CSV snapshot below mirrors this order.
    held.sort_by(|(a, _, ar, _), (b, _, br, _)| ar.cmp(br).then_with(|| a.name.cmp(&b.name)));

    let labels: HashMap<String, String> = pending.iter()
        .map(|u| (sightings::key(u.source.as_str(), &u.name), source_label(u, &packages)))
        .collect();
    if crate::machine::plan_mode() {
        crate::machine::emit(&plan_json(&ready, &held, &unknown, &labels, &source_presence, &cfg.holds));
        return UpdateEnd::Stopped;
    }
    // v1.6.1: a named list shows only what it will update. A named package that
    // is held — not promoted, or waiting on a partner — stops the run before any
    // question or change, with what to do instead.
    let named = !only.is_empty();
    if named {
        let held_view: Vec<(&str, String, Option<&str>)> = held.iter()
            .map(|(u, _, r, why)| (u.name.as_str(), held_note(*r, why),
                match why { HeldReason::CoupledTo(p) => Some(p.as_str()), _ => None }))
            .collect();
        let pending_names: Vec<&str> = ready.iter().map(|(u, _, _)| u.name.as_str())
            .chain(unknown.iter().map(|(u, _)| u.name.as_str()))
            .collect();
        let problems = named_problems(&only, &pending_names, &held_view);
        if !problems.is_empty() {
            println!();
            println!("{}nog: Not updating — {} of the packages you named can't go in now:{}",
                C_BOLD, problems.len(), C_RESET);
            println!();
            for p in &problems {
                println!("  {}", p);
            }
            println!();
            println!("nog: Nothing was installed or changed.");
            crate::events::finished("check", "Checking for updates", false,
                &format!("{} of the packages you named can't go in now", problems.len()));
            return UpdateEnd::Failed(1);
        }
        let src = |upd: &PendingUpdate| label_for(&labels, upd);
        let rows: Vec<TableRow> = ready.iter()
            .map(|(upd, tier, reason)| TableRow::from(upd, tier, src(upd), ready_note(reason)))
            .collect();
        println!();
        print!("{}", format_table("UPDATING ONLY WHAT YOU NAMED", &rows, true));
        if !unknown.is_empty() {
            let rows: Vec<TableRow> = unknown.iter()
                .map(|(upd, tier)| TableRow::from(upd, tier, src(upd), "no build date in sync DB".to_string()))
                .collect();
            println!();
            print!("{}", format_table("UNKNOWN", &rows, true));
        }
    } else {
        let r: Vec<&PendingUpdate> = ready.iter().map(|(u, _, _)| u).collect();
        let h: Vec<&PendingUpdate> = held.iter().map(|(u, _, _, _)| u).collect();
        let k: Vec<&PendingUpdate> = unknown.iter().map(|(u, _)| u).collect();
        let rows = summary_rows(&sync_db::repo_order(), &labels, &r, &h, &k, &source_presence);
        println!();
        print!("{}", format_summary(&rows, true));
        print_buckets(&ready, &held, &unknown, &labels);
    }
    let _ = (&flatpak_names, &snap_names);

    // v1.4.1 (issue #18) — a tier hold that snapd does not know about is not a
    // hold. snapd auto-refreshes on ITS timer (four times a day by default),
    // so listing only cleared snaps at refresh time enforced nothing: on
    // 2026-09-15 nog showed `core20` as held with "1 day remaining" while
    // snapd refreshed it in the same minute.
    //
    // Placed here, immediately after the buckets are final and BEFORE any
    // handoff, so the window is shut as early as nog can shut it. It does not
    // block nog's own refresh later: snapd leaves a named snap's explicit
    // refresh unblocked, and nog always names what it refreshes.
    if !snap_names.is_empty() {
        let all_held: Vec<(String, u64)> = held
            .iter()
            .map(|(u, _, days, _)| (u.name.clone(), *days))
            .collect();
        let held_snaps = snap::held_snap_windows(&all_held, &snap_names);
        let groups = snap::hold_args(&held_snaps);
        if !groups.is_empty() {
            println!();
            println!("{}nog: Holding {} snap(s) in snapd so its own timer cannot refresh them ...{}",
                C_BOLD, held_snaps.len(), C_RESET);
            println!("{}     (a snap hold needs root — sudo may prompt){}", C_SUBTEXT, C_RESET);
            for (hours, names) in &groups {
                let st = snap::place_hold(*hours, names);
                if !st.success() {
                    // A hold that did not land must never be reported as one:
                    // silently carrying on is precisely the shape of #18.
                    println!("{}     NOT HELD (status {}): {}{}",
                        C_RED, st.code().unwrap_or(-1), names.join(", "), C_RESET);
                    println!("{}     snapd can still refresh those on its own schedule.{}",
                        C_RED, C_RESET);
                }
            }
        }
    }

    // v1.0.8: snapshot the final buckets for the run log. Taken after the
    // realign/coupling passes so the CSV mirrors the printed tables exactly.
    let log_rows = runlog_rows(&ready, &held, &unknown);

    // Interactive step: decide what to do with Unknowns. Each gets a y/N prompt.
    // EOF or non-TTY stdin → default all remaining to skip, with a warning.
    let mut extra_ignore: Vec<String> = Vec::new();
    // v2.1.0 (F-20, #56): nothing at the keyboard and no --yes. Until now pacman read
    // end-of-input as "no" and the run said "Cancelled", as if someone had answered.
    if !update_yes() && !at_the_keyboard() {
        enotice(C_PEACH, "Not updating", &nobody_to_answer_update_lines());
        write_run_log(&cfg, &run_date, &run_time, &run_user,
            settle_rows(&log_rows, &extra_ignore, &RunEnd::Cancelled), "");
        return UpdateEnd::Failed(1);
    }
    if update_yes() && !at_the_keyboard() && !pacman::sudo_ready_unattended() {
        enotice(C_PEACH, "Not updating", &sudo_needs_a_person_lines());
        write_run_log(&cfg, &run_date, &run_time, &run_user,
            settle_rows(&log_rows, &extra_ignore, &RunEnd::Cancelled), "");
        return UpdateEnd::Failed(1);
    }
    if !unknown.is_empty() && update_yes() {
        let names: Vec<String> = unknown.iter().map(|(u, _)| u.name.clone()).collect();
        println!();
        println!("{}nog: --yes keeps {} Unknown package(s) back: {}{}", C_BOLD, names.len(), names.join(", "), C_RESET);
        println!("{}     Each has no usable build date, so updating it is a decision, and --yes{}", C_SUBTEXT, C_RESET);
        println!("{}     never makes it for you. Decide them at a terminal: nog update{}", C_SUBTEXT, C_RESET);
        extra_ignore.extend(keep_unknowns_back(&names));
    } else if !unknown.is_empty() {
        println!();
        println!("{}nog: {} package(s) have no usable build date in any sync DB.{}",
            C_SUBTEXT, unknown.len(), C_RESET);
        println!("{}      Usually an AUR-only, locally-built, or disabled-repo package — or a{}",
            C_SUBTEXT, C_RESET);
        println!("{}      DB entry that doesn't match the pending candidate's version.{}",
            C_SUBTEXT, C_RESET);
        println!();

        let mut auto_skip_rest = false;
        for (upd, tier) in &unknown {
            if auto_skip_rest {
                extra_ignore.push(upd.name.clone());
                continue;
            }
            match prompt_unknown(&upd.name, tier, &upd.old_version, &upd.new_version) {
                PromptOutcome::Yes => { /* allow through */ }
                PromptOutcome::No => extra_ignore.push(upd.name.clone()),
                PromptOutcome::Eof => {
                    eprintln!("{}nog: no interactive input available — skipping remaining unknowns.{}",
                        C_SUBTEXT, C_RESET);
                    extra_ignore.push(upd.name.clone());
                    auto_skip_rest = true;
                }
            }
        }
    }

    // Final ignore list = tier-held packages + user-skipped unknowns.
    let extra_ignore_log = extra_ignore.clone();
    let mut ignore: Vec<String> = held.iter().map(|(u, _, _, _)| u.name.clone()).collect();
    ignore.extend(extra_ignore);

    if ready.is_empty() && ignore.len() == pending.len() {
        println!();
        println!("nog: Nothing to install — every pending update is held.");
        crate::events::finished("check", "Checking for updates", true, "nothing to install: every update is on hold");
        write_run_log(&cfg, &run_date, &run_time, &run_user,
            settle_rows(&log_rows, &extra_ignore_log, &RunEnd::AllHeld), "");
        return UpdateEnd::Done;
    }

    // v1.0.9 (Ironhold): the foreign fence — every foreign package is ignored
    // unless nog cleared it THIS run. See holds::foreign_fence.
    //
    // v1.3.0 (issue #10) demoted this from the primary defence to a second
    // layer, and it is worth being honest about which it now is. The fence was
    // built because the old `-Syu` handoff let the helper resolve AUR updates
    // itself, so it could upgrade a held package our earlier query had failed to
    // name — the 2026-08-01 bypass. The AUR step no longer works that way: it is
    // handed an explicit list of cleared names, so an unnamed package cannot
    // move whatever the query did.
    //
    // The fence stays because it still does real work. It rides along as
    // `--ignore` on the AUR step, where dependency resolution during an AUR
    // build can still reach for a held package. That is a different hole from
    // the one it was built for, and it is still open.
    if helper.is_some() {
        let mut cleared: Vec<String> = ready.iter().map(|(u, _, _)| u.name.clone()).collect();
        cleared.extend(
            unknown.iter()
                .map(|(u, _)| u.name.clone())
                .filter(|n| !ignore.contains(n)),
        );
        let fence = holds::foreign_fence(&pacman::foreign_package_names(), &cleared, &ignore);
        // v1.6.1: with a named list the fence still applies; only its note is
        // left out, because the screen shows just what you named.
        if !fence.is_empty() {
            if !named {
            println!();
            println!(
                "{}nog: foreign fence — {} AUR/local package(s) held back as a dependency{}",
                C_SUBTEXT, fence.len(), C_RESET
            );
            println!(
                "{}     (the AUR step installs only what nog named; this also blocks them as deps).{}",
                C_SUBTEXT, C_RESET
            );
            }
            ignore.extend(fence);
        }
    }

    // F-17 (rc2): locked packages that have no update right now.
    if !idle_locks.is_empty() && !named {
        println!();
        println!(
            "{}nog: locked by you, nothing pending — {} (up to date; `nog unlock` releases {}).{}",
            C_SUBTEXT, idle_locks.join(", "), if idle_locks.len() == 1 { "it" } else { "them" }, C_RESET
        );
    }

    // v1.7.0: the plan is ready — say so, and which steps will run
    {
        let mut planned: Vec<(&str, String)> = Vec::new();
        let n_ready = ready.len() + unknown.iter().filter(|(u, _)| !ignore.contains(&u.name)).count();
        if ready.iter().any(|(u, _, _)| u.source == Source::Pacman && KEYRINGS.contains(&u.name.as_str())) {
            planned.push(("keys", "New package keys".to_string()));
        }
        planned.push(("pacman", "Official packages".to_string()));
        let rn: Vec<String> = ready.iter().map(|(u, _, _)| u.name.clone()).collect();
        let un: Vec<String> = unknown.iter().map(|(u, _)| u.name.clone()).collect();
        if let Some(h) = helper {
            let a = aur::apply_list(&aur_names, &rn, &un, &ignore).len();
            if a > 0 {
                planned.push(("aur", format!("AUR packages ({}, built by {})", a, h)));
            }
        }
        if !flatpak_names.is_empty() && !flatpak::apply_list(&flatpak_names, &rn, &un, &ignore).is_empty() {
            planned.push(("flatpak", "Flatpak apps".to_string()));
        }
        if !snap_names.is_empty() && !snap::apply_list(&snap_names, &rn, &un, &ignore).is_empty() {
            planned.push(("snap", "Snaps".to_string()));
        }
        crate::events::steps(&std::iter::once(("check", "Checking for updates".to_string()))
            .chain(planned.into_iter()).collect::<Vec<_>>());
        crate::events::finished("check", "Checking for updates", true,
            &format!("{} to install, {} on hold", n_ready, held.len()));
    }

    // First review gate. Each source's own tool presents its transaction and
    // asks again — deliberate layers, so an expert can still catch and cancel
    // at the point where the detail is in front of them. One prompt per tool
    // that actually runs; sources with nothing to do stay silent.
    println!();
    let go = if update_yes() {
        println!("nog: Begin the handoff? yes (--yes)");
        true
    } else {
        prompt_proceed()
    };
    if !go {
        println!("nog: Cancelled — nothing was installed.");
        write_run_log(&cfg, &run_date, &run_time, &run_user,
            settle_rows(&log_rows, &extra_ignore_log, &RunEnd::Cancelled), "");
        return UpdateEnd::Stopped;
    }

    // v1.3.0 (issue #10): one package manager per source, in nog's own order —
    // pacman -> AUR helper -> flatpak -> snap.
    //
    // Until now the whole repo+AUR upgrade went to the AUR helper in a single
    // `-Syu`, so yay rebuilt and re-narrated a plan pacman was about to execute
    // anyway: every held package was announced twice, once by each tool. Worse,
    // it blurred the source boundary nog spends the whole report making visible.
    //
    // Failure handling differs by step, deliberately. pacman is foundational —
    // AUR packages are compiled against official libraries, so a failed repo
    // upgrade must never be followed by builds against a half-upgraded system.
    // That one cancels. Every later step is independent enough that a failure is
    // worth reporting and asking about rather than deciding unilaterally.
    let ready_names: Vec<String> = ready.iter().map(|(u, _, _)| u.name.clone()).collect();
    let unknown_names: Vec<String> = unknown.iter().map(|(u, _)| u.name.clone()).collect();
    let mut step_failures: Vec<String> = Vec::new();
    // v1.4.3 (issue #19): what each source's step actually did, so every row
    // of the run log can say what became of its own package.
    let mut steps: HashMap<Source, StepState> = HashMap::new();

    // Step 0 — v1.5.6 (#31): new keys before anything they will be needed for.
    let keyrings: Vec<String> = ready.iter()
        .filter(|(u, _, _)| u.source == Source::Pacman && KEYRINGS.contains(&u.name.as_str()))
        .map(|(u, _, _)| u.name.clone())
        .collect();
    if !keyrings.is_empty() {
        println!();
        println!("{}nog: Installing the new keys first ({}) ...{}", C_BOLD, keyrings.join(", "), C_RESET);
        crate::events::start("keys", "New package keys");
        let k = pacman::install_keyrings(&keyrings);
        crate::events::finished("keys", "New package keys", k.status.success(), &keyrings.join(", "));
        if !k.status.success() {
            eprintln!();
            eprintln!("{}nog: the keys did not install (status {}) — stopping.{}",
                C_BOLD, k.status.code().unwrap_or(-1), C_RESET);
            if let Some(r) = &k.reason {
                eprintln!("     {}", r);
            }
            eprintln!("     Every later package's signature would be checked against old keys.");
            return UpdateEnd::Failed(k.status.code().unwrap_or(1));
        }
    }

    // Step 1 — official repositories (including binary repos like chaotic-aur).
    println!();
    println!("{}nog: Handing off official packages to pacman ...{}", C_BOLD, C_RESET);
    // v1.6.1 (Javier, 4 Oct 2026, option b): with a named list, pacman still
    // prints one "ignoring package upgrade" warning per package it skips. Those
    // lines come from pacman straight to the terminal (v1.5.8 F-8 keeps it that
    // way, so its question can't overtake its table); nog says so first.
    if named {
        let skipped = held.iter().filter(|(u, _, _, _)| u.source == Source::Pacman).count();
        if let Some(note) = skip_note(skipped) {
            println!("{}{}{}", C_SUBTEXT, note, C_RESET);
        }
    }
    crate::events::start("pacman", "Official packages");
    let pac = pacman::update_excluding(&ignore, update_yes());
    crate::events::finished("pacman", "Official packages", pac.status.success(),
        &if pac.status.success() { String::new() } else { format!("pacman stopped (status {})", pac.status.code().unwrap_or(-1)) });
    steps.insert(Source::Pacman, StepState::from(&pac));
    if !pac.status.success() {
        let code = pac.status.code().unwrap_or(-1);
        eprintln!();
        eprintln!("{}nog: pacman exited with status {} — cancelling.{}", C_BOLD, code, C_RESET);
        eprintln!("     That is either a declined prompt or a pacman error — the exit");
        eprintln!("     status alone cannot tell the two apart.");
        eprintln!("     No other source was touched. AUR packages are built against");
        eprintln!("     official libraries, so nog will not build them on a system");
        eprintln!("     whose repo upgrade did not complete.");
        write_run_log(&cfg, &run_date, &run_time, &run_user,
            settle_rows(&log_rows, &extra_ignore_log, &RunEnd::Steps(steps)), "");
        return UpdateEnd::Failed(pac.status.code().unwrap_or(1));
    }
    // v1.5.7 (F-3, #35): pacman refreshed the lists and completed with them.
    sync_db::mark_update_done();

    // Step 2 — the AUR, by name. Only what nog cleared this run is ever passed,
    // so held AUR packages are not merely ignored, they are never mentioned.
    if let Some(h) = helper {
        let aur_apply = aur::apply_list(&aur_names, &ready_names, &unknown_names, &ignore);
        // v2.1.0 (F-20, #56): the AUR's build review needs a person (#26, #32); --yes never
        // covers it. With nobody at the keyboard they wait for the next run at a terminal.
        if !aur_apply.is_empty() && update_yes() && !at_the_keyboard() {
            println!();
            println!("{}nog: kept back from the AUR: {}{}", C_BOLD, aur_apply.join(", "), C_RESET);
            println!("{}     A build is reviewed by a person, and nobody is at the keyboard. They are{}", C_SUBTEXT, C_RESET);
            println!("{}     still Ready; run nog update in a terminal to build them.{}", C_SUBTEXT, C_RESET);
        } else if !aur_apply.is_empty() {
            println!();
            println!("{}nog: Handing off {} AUR package(s) to {} ...{}",
                C_BOLD, aur_apply.len(), h, C_RESET);
            println!("{}     ({} shows its own build and transaction below){}",
                C_SUBTEXT, h, C_RESET);
            let label = format!("AUR packages ({}, built by {})", aur_apply.len(), h);
            crate::events::start("aur", &label);
            let aur_run = aur::upgrade_cleared(h, &aur_apply, &ignore);
            crate::events::finished("aur", &label, aur_run.status.success(), "");
            steps.insert(Source::Aur, StepState::from(&aur_run));
            if !aur_run.status.success() {
                let code = aur_run.status.code().unwrap_or(-1);
                step_failures.push(failure_label("aur", code, aur_run.reason.as_deref()));
                if !prompt_continue_after_failure(&h.to_string(), code, aur_run.reason.as_deref()) {
                    write_run_log(&cfg, &run_date, &run_time, &run_user,
                        settle_rows(&log_rows, &extra_ignore_log, &RunEnd::Steps(steps)), "");
                    print_incomplete(&step_failures);
                    return UpdateEnd::Incomplete;
                }
            }
        }
    }

    // v1.1.0 (C1): flatpak apply — only the refs nog cleared THIS run
    // (Ready, or an Unknown the user approved). flatpak has no --ignore, so
    // listing exactly the cleared app IDs IS the hold mechanism: held
    // flatpaks are simply never named.
    if !flatpak_names.is_empty() {
        let fp_apply = flatpak::apply_list(&flatpak_names, &ready_names, &unknown_names, &ignore);
        if !fp_apply.is_empty() {
            println!();
            println!("{}nog: Handing off {} app(s) to flatpak ...{}", C_BOLD, fp_apply.len(), C_RESET);
            println!("{}     (flatpak shows its own transaction below){}", C_SUBTEXT, C_RESET);
            crate::events::start("flatpak", "Flatpak apps");
            let fp_run = flatpak::update(&fp_apply);
            crate::events::finished("flatpak", "Flatpak apps", fp_run.status.success(), "");
            steps.insert(Source::Flatpak, StepState::from(&fp_run));
            if !fp_run.status.success() {
                let code = fp_run.status.code().unwrap_or(-1);
                step_failures.push(failure_label("flatpak", code, fp_run.reason.as_deref()));
                if !prompt_continue_after_failure("flatpak", code, fp_run.reason.as_deref()) {
                    write_run_log(&cfg, &run_date, &run_time, &run_user,
                        settle_rows(&log_rows, &extra_ignore_log, &RunEnd::Steps(steps)), "");
                    print_incomplete(&step_failures);
                    return UpdateEnd::Incomplete;
                }
            }
        }
    }

    // v1.2.0 (C2): snap apply — same naming rule as flatpak. `snap refresh`
    // needs root, so nog escalates through sudo for this step only.
    if !snap_names.is_empty() {
        let sn_apply = snap::apply_list(&snap_names, &ready_names, &unknown_names, &ignore);
        if !sn_apply.is_empty() {
            println!();
            println!("{}nog: Handing off {} snap(s) to snapd ...{}", C_BOLD, sn_apply.len(), C_RESET);
            println!("{}     (snap refresh needs root — sudo may prompt; snap shows its own progress){}",
                C_SUBTEXT, C_RESET);
            crate::events::start("snap", "Snaps");
            let sn_run = snap::refresh(&sn_apply);
            crate::events::finished("snap", "Snaps", sn_run.status.success(), "");
            steps.insert(Source::Snap, StepState::from(&sn_run));
            if !sn_run.status.success() {
                let code = sn_run.status.code().unwrap_or(-1);
                step_failures.push(failure_label("snap", code, sn_run.reason.as_deref()));
                if !prompt_continue_after_failure("snap", code, sn_run.reason.as_deref()) {
                    write_run_log(&cfg, &run_date, &run_time, &run_user,
                        settle_rows(&log_rows, &extra_ignore_log, &RunEnd::Steps(steps)), "");
                    print_incomplete(&step_failures);
                    return UpdateEnd::Incomplete;
                }
            }
        }
    }

    println!();
    // v1.4.3 (issue #19): the outcome is written per package, from what its
    // own source's step did. It used to be the run's verdict copied onto
    // every row, which recorded held packages as installed. "did not
    // complete" rather than "failed": a non-zero exit is equally a user
    // declining the tool's own prompt, and the run log is permanent.
    if step_failures.is_empty() {
        println!("nog: Update finished!");
    } else {
        print_incomplete(&step_failures);
    }
    write_run_log(&cfg, &run_date, &run_time, &run_user,
        settle_rows(&log_rows, &extra_ignore_log, &RunEnd::Steps(steps)), "");

    // Issue #9 — reboot advice. Deliberately placed after the handoff, so the
    // probes see what pacman actually did rather than what nog asked for; and
    // after the run log, so a misbehaving probe can never cost the permanent
    // record. The user chose the packages that reached this point.
    let cleared_for_reboot: Vec<(String, String)> = ready
        .iter()
        .map(|(u, _, _)| (u.name.clone(), u.new_version.clone()))
        .chain(unknown.iter().filter_map(|(u, _)| {
            if ignore.iter().any(|i| i == &u.name) {
                None
            } else {
                Some((u.name.clone(), u.new_version.clone()))
            }
        }))
        .collect();
    let advice = print_reboot_advice(&cleared_for_reboot, &tm);
    // Issue #22: the advice leaves a trace. A separate file, written after
    // the run log is safely on disk, so a probe problem cannot touch it.
    if !advice.is_empty() {
        if let Some((today, _)) = runlog::today_and_cutoff() {
            match runlog::append_reboot(
                &cfg.paths.run_logs, &today, &run_date, &run_time, &run_user, &advice,
            ) {
                Ok(path) => record_log(path),
                Err(e) => eprintln!("{}nog: warning — reboot advice not logged: {}{}", C_SUBTEXT, e, C_RESET),
            }
        }
    }

    if step_failures.is_empty() { UpdateEnd::Done } else { UpdateEnd::Incomplete }
}

/// v2.1.0 (F-22): "Update finished, with N step(s) that did not complete", and which.
fn print_incomplete(step_failures: &[String]) {
    println!("{}nog: Update finished, with {} step(s) that did not complete.{}",
        C_BOLD, step_failures.len(), C_RESET);
    println!("{}     Incomplete: {}{}", C_SUBTEXT, step_failures.join(", "), C_RESET);
    println!("{}     Run nog update again to finish them.{}", C_SUBTEXT, C_RESET);
}

/// v2.1.0 (F-21): a failed step's label for the closing summary — its status, and the
/// cause too when nog knows it for certain (a password nobody typed in time).
fn failure_label(step: &str, code: i32, reason: Option<&str>) -> String {
    match reason {
        Some(r) if r == crate::handoff::PASSWORD_TIMED_OUT => format!("{} (status {}: {})", step, code, r),
        _ => format!("{} (status {})", step, code),
    }
}

/// Issue #9: say something when the running machine no longer matches what is
/// installed. Silent unless there is something to say — a notice that appears
/// after every run is one nobody reads, which is how the original twenty minutes
/// were lost.
///
/// Verified findings are printed in Tier 1 red because they are facts about a
/// broken state; the announcement that follows is muted, because it is advice.
///
/// Returns what it concluded as `(level, text)` pairs for the reboot log
/// (issue #22): empty when nothing relevant was handed off, one `checked`
/// line when the probes ran and found nothing, otherwise one pair per line
/// printed.
fn print_reboot_advice(cleared: &[(String, String)], tm: &TierManager) -> Vec<(String, String)> {
    let tier1 = tm.tier1_packages();

    // Probe only if something relevant was handed off. On an ordinary run this
    // is empty and nog does no extra work at all.
    let candidates: Vec<String> = cleared
        .iter()
        .filter(|(name, _)| reboot::classify(name, &tier1).is_some())
        .map(|(name, _)| name.clone())
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }

    let probe = reboot::SystemProbe::read(&candidates);
    let lines = reboot::render(&reboot::assess(cleared, &tier1, &probe));
    if lines.is_empty() {
        return vec![(
            "checked".to_string(),
            format!("no reboot needed after: {}", candidates.join(" ")),
        )];
    }

    println!();
    for line in &lines {
        if line.starts_with("IMPORTANT:") {
            println!("{}{}{}{}", C_BOLD, C_RED, line, C_RESET);
        } else if line.starts_with("NOTE:") {
            println!("{}{}{}{}", C_BOLD, C_YELLOW, line, C_RESET);
        } else {
            println!("{}{}{}", C_SUBTEXT, line, C_RESET);
        }
    }
    advice_levels(&lines)
}

/// Pair each printed advice line with the level of the heading it sits
/// under (issue #22). A notice is an `IMPORTANT:` or `NOTE:` line followed
/// by explanation lines; in a spreadsheet each row must still say which one
/// it belongs to. Blank spacer lines carry nothing and are dropped.
fn advice_levels(lines: &[String]) -> Vec<(String, String)> {
    let mut level = "info".to_string();
    let mut out = Vec::new();
    for line in lines {
        if let Some((head, _)) = line.split_once(':') {
            if head == "IMPORTANT" || head == "NOTE" {
                level = head.to_string();
            }
        }
        let text = line.trim();
        if !text.is_empty() {
            out.push((level.clone(), text.to_string()));
        }
    }
    out
}

enum PromptOutcome { Yes, No, Eof }

fn prompt_unknown(pkg: &str, tier: &Tier, old: &str, new: &str) -> PromptOutcome {
    use std::io::{self, Write};
    let color = tier_color(tier);
    loop {
        print!(
            "  {}{}{} ({} {} -> {}) — update anyway? [y/N] ",
            color, pkg, C_RESET, tier, old, new
        );
        if io::stdout().flush().is_err() {
            return PromptOutcome::Eof;
        }
        let mut buf = String::new();
        match io::stdin().read_line(&mut buf) {
            Ok(0) => return PromptOutcome::Eof,
            Ok(_) => {
                let t = buf.trim().to_lowercase();
                if t == "y" || t == "yes" { return PromptOutcome::Yes; }
                if t.is_empty() || t == "n" || t == "no" { return PromptOutcome::No; }
                // anything else: reprompt
            }
            Err(_) => return PromptOutcome::Eof,
        }
    }
}

/// Print the v1.0.7 update banner: name, date, time, and the invoking user.
/// Date/time come from the system `date` command — nog already spawns
/// subprocesses, and this keeps the dependency tree free of a datetime crate.
/// Returns `(date, time, user)` so the run log (v1.0.8) records the exact
/// context the banner showed.
fn print_update_header(show: bool) -> (String, String, String) {
    let (date, time) = now_date_time();
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "unknown".to_string());
    // v1.5.8: the start of every run is `begin`'s notice; the old banner
    // is gone, and `nog install` updating first shows none either.
    if !show {
        return (date, time, user);
    }
    println!();
    println!("=============");
    println!("{}nog v{}{}", C_BOLD, env!("CARGO_PKG_VERSION"), C_RESET);
    println!("{}Update!{}", C_BOLD, C_RESET);
    println!("=============");
    println!();
    println!("Date: {}", date);
    println!("Time: {}", time);
    println!("User: {}", user);
    println!();
    (date, time, user)
}

/// `(MM/DD/YYYY, HH:MM AM/PM)` via the system `date`. Falls back to placeholders
/// if `date` is unavailable rather than failing the run.
fn now_date_time() -> (String, String) {
    if let Ok(o) = std::process::Command::new("date").arg("+%m/%d/%Y|%I:%M %p").output() {
        if o.status.success() {
            let s = String::from_utf8_lossy(&o.stdout);
            if let Some((d, t)) = s.trim().split_once('|') {
                return (d.to_string(), t.to_string());
            }
        }
    }
    ("--/--/----".to_string(), "--:-- --".to_string())
}

/// The pre-handoff review gate. Default is yes (`[Y/n]`); a non-interactive
/// stdin (EOF) declines rather than auto-installing.
fn prompt_proceed() -> bool {
    use std::io::{self, Write};
    crate::events::ask("Begin the handoff? [Y/n]");
    print!("nog: Begin the handoff? [Y/n] ");
    if io::stdout().flush().is_err() {
        return false;
    }
    let mut buf = String::new();
    match io::stdin().read_line(&mut buf) {
        Ok(0) => false,
        Ok(_) => {
            let t = buf.trim().to_lowercase();
            t.is_empty() || t == "y" || t == "yes"
        }
        Err(_) => false,
    }
}

/// v1.3.0 (issue #10): a source failed, and the remaining sources are
/// independent of it. Report it and let the user decide.
///
/// Default is **no** — the opposite of the pre-handoff gate, and deliberately
/// so. Agreeing to an update is not agreeing to push past a failure in it, and
/// a non-interactive run must never carry on through an error it cannot show
/// anyone. Ctrl-D stops, same as answering no.
fn prompt_continue_after_failure(step: &str, code: i32, reason: Option<&str>) -> bool {
    use std::io::{self, IsTerminal, Write};
    eprintln!();
    eprintln!("{}nog: the {} step exited with status {}.{}", C_BOLD, step, code, C_RESET);
    if reason == Some(crate::handoff::PASSWORD_TIMED_OUT) {
        // v2.1.0 (F-21): the cause is known, so it is said.
        eprintln!("{}     The cause: {}. Official packages already{}", C_SUBTEXT, reason.unwrap_or(""), C_RESET);
    } else {
        eprintln!("{}     That is either a declined prompt or an error — the exit status{}",
            C_SUBTEXT, C_RESET);
        eprintln!("{}     alone cannot tell the two apart. Official packages already{}",
            C_SUBTEXT, C_RESET);
    }
    eprintln!("{}     upgraded; either way it is confined to {}, and the remaining{}",
        C_SUBTEXT, step, C_RESET);
    eprintln!("{}     sources are independent of it.{}", C_SUBTEXT, C_RESET);
    if update_yes() {
        // v2.1.0 (F-20, #56): --yes takes the question's default. Agreeing to an update is
        // not agreeing to push past a failure in it; the rest waits for the next run.
        println!("nog: Continue with the remaining sources? no (--yes takes the default)");
        return false;
    }
    print!("nog: Continue with the remaining sources? [y/N] ");
    if io::stdout().flush().is_err() {
        return false;
    }
    let mut buf = String::new();
    let read = io::stdin().read_line(&mut buf);
    // Issue #14: at a terminal the user's Enter ends the prompt line. From a
    // pipe or /dev/null nothing does, and the next message lands on the same
    // line as the question. End it ourselves, on the stream it was printed to.
    if !io::stdin().is_terminal() {
        println!();
    }
    match read {
        Ok(0) => {
            eprintln!("nog: no input — stopping here.");
            false
        }
        Ok(_) => {
            let t = buf.trim().to_lowercase();
            t == "y" || t == "yes"
        }
        Err(_) => false,
    }
}

/// The tier's plain 1/2/3 number (the `Tier` column in the update tables).
fn tier_num(t: &Tier) -> u8 {
    match t {
        Tier::One => 1,
        Tier::Two => 2,
        Tier::Three => 3,
    }
}

/// Per-tier color keyed by the plain number (used to tint the `Tier` cell).
fn tier_color_num(n: u8) -> &'static str {
    match n {
        1 => C_RED,
        2 => C_YELLOW,
        _ => C_GREEN,
    }
}

/// One row in an update section table.
struct TableRow {
    pkg: String,
    /// v1.5.4 (#28): where the package comes from — the repository (`core`,
    /// `extra`, …) for official packages, else `AUR`, `Flatpak` or `Snap`.
    source: String,
    old: String,
    new: String,
    tier: u8,
    note: String,
}

impl TableRow {
    fn from(upd: &PendingUpdate, tier: &Tier, source: String, note: String) -> TableRow {
        TableRow {
            pkg: upd.name.clone(),
            source,
            old: upd.old_version.clone(),
            new: upd.new_version.clone(),
            tier: tier_num(tier),
            note,
        }
    }
}

/// Render one v1.0.7 update section as an aligned table. Pure + unit-tested.
///
/// Column widths are computed from the plain text; when `colorize` is set the
/// `Tier` digit is wrapped in its per-tier color with the padding left OUTSIDE
/// the escape codes, so alignment is byte-for-byte identical colored or not.
/// An empty section renders its header and `(none)`. Terminal width is
/// intentionally ignored — long version strings simply widen the columns.
fn format_table(title: &str, rows: &[TableRow], colorize: bool) -> String {
    let title_line = format!("{}:", title);
    let mut out = format!("{}\n{}\n\n", title_line, "=".repeat(title_line.len()));

    if rows.is_empty() {
        out.push_str("(none)\n");
        return out;
    }

    let pkg_hdr = format!("Package ({})", rows.len());
    let w_pkg = std::iter::once(pkg_hdr.len())
        .chain(rows.iter().map(|r| r.pkg.len()))
        .max()
        .unwrap();
    let w_src = std::iter::once("Source".len())
        .chain(rows.iter().map(|r| r.source.len()))
        .max()
        .unwrap();
    let w_old = std::iter::once("Old Version".len())
        .chain(rows.iter().map(|r| r.old.len()))
        .max()
        .unwrap();
    let w_new = std::iter::once("New Version".len())
        .chain(rows.iter().map(|r| r.new.len()))
        .max()
        .unwrap();
    let w_tier = "Tier".len(); // the tier digit is always a single char
    // Notes are the one column that carries non-ASCII (`·`, `—`), so measure
    // them in characters: `len()` counts bytes and would overshoot the rule.
    let w_note = std::iter::once("Note".len())
        .chain(rows.iter().map(|r| r.note.chars().count()))
        .max()
        .unwrap();
    let g = "  ";

    out.push_str(&format!(
        "{:<wp$}{g}{:<ws$}{g}{:<wo$}{g}{:<wn$}{g}{:<wt$}{g}{}\n",
        pkg_hdr, "Source", "Old Version", "New Version", "Tier", "Note",
        wp = w_pkg, ws = w_src, wo = w_old, wn = w_new, wt = w_tier, g = g,
    ));
    // Rule under the column headers, sized to the table's real width so it
    // never runs short of the Note column or past it.
    let table_width =
        w_pkg + w_src + w_old + w_new + w_tier + w_note + 5 * g.len();
    out.push_str(&"-".repeat(table_width));
    out.push('\n');

    for r in rows {
        let tier_cell = if colorize {
            format!(
                "{}{}{}{}",
                tier_color_num(r.tier), r.tier, C_RESET, " ".repeat(w_tier - 1)
            )
        } else {
            format!("{:<wt$}", r.tier, wt = w_tier)
        };
        // Padding stays outside the colour codes, like the tier digit.
        let src_cell = match (colorize, source_color(&r.source)) {
            (true, Some(c)) => format!("{}{}{}{}", c, r.source, C_RESET, " ".repeat(w_src - r.source.len())),
            _ => format!("{:<ws$}", r.source, ws = w_src),
        };
        out.push_str(&format!(
            "{:<wp$}{g}{}{g}{:<wo$}{g}{:<wn$}{g}{}{g}{}\n",
            r.pkg, src_cell, r.old, r.new, tier_cell, r.note,
            wp = w_pkg, wo = w_old, wn = w_new, g = g,
        ));
    }
    out
}

/// Map a Ready bucket entry to its `Note` text.
fn ready_note(reason: &ReadyReason) -> String {
    match reason {
        ReadyReason::Expired { days_past_window: 0 } => "hold just expired".to_string(),
        ReadyReason::Expired { days_past_window: 1 } => "1 day past window".to_string(),
        ReadyReason::Expired { days_past_window } => format!("{} days past window", days_past_window),
        ReadyReason::Realigned => "realigned to match installed headers".to_string(),
        ReadyReason::Keys => "keys · never held, installed first".to_string(),
        ReadyReason::AfterWaiting(w) => w.describe(),
        ReadyReason::Promoted(None) => "promoted by you".to_string(),
        ReadyReason::Promoted(Some(p)) => format!("promoted with {}", p),
    }
}

/// Map a Held bucket entry to its `Note` text.
/// v1.6.0 (#7): the plan as data, for nogForge. Same buckets, same words.
fn plan_json(
    ready: &[(PendingUpdate, Tier, ReadyReason)],
    held: &[(PendingUpdate, Tier, u64, HeldReason)],
    unknown: &[(PendingUpdate, Tier)],
    labels: &HashMap<String, String>,
    presence: &SourcePresence,
    holds: &crate::config::HoldsConfig,
) -> serde_json::Value {
    use serde_json::json;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let row = |u: &PendingUpdate, t: &Tier| json!({
        "name": u.name, "source": label_for(labels, u), "tier": crate::machine::tier_number(t),
        "old": u.old_version, "new": u.new_version,
    });
    let ready_rows: Vec<_> = ready.iter().map(|(u, t, r)| {
        let mut v = row(u, t);
        v["note"] = json!(ready_note(r));
        v
    }).collect();
    let held_rows: Vec<_> = held.iter().map(|(u, t, rem, r)| {
        let mut v = row(u, t);
        v["note"] = json!(held_note(*rem, r));
        v["days_remaining"] = json!(rem);
        v["ready_on"] = match r {
            HeldReason::ManualSignoff | HeldReason::KeptBack | HeldReason::Locked => json!(null),
            HeldReason::CoupledTo(_) if *rem == 0 => json!(null),
            _ => json!(now + rem * 86_400),
        };
        v["kept_back"] = json!(matches!(r, HeldReason::KeptBack));
        v["coupled_to"] = match r { HeldReason::CoupledTo(p) | HeldReason::BuiltAfter(p) => json!(p), _ => json!(null) };
        v["locked"] = json!(matches!(r, HeldReason::Locked));
        v
    }).collect();
    let unknown_rows: Vec<_> = unknown.iter().map(|(u, t)| row(u, t)).collect();
    let word = |p: &Presence| match p {
        Presence::Checked => "checked", Presence::Failed => "could not check",
        Presence::Off => "off", Presence::Absent => "not installed",
    };
    json!({
        "nog": env!("CARGO_PKG_VERSION"), "kind": "plan",
        "sources": {"aur": word(&presence.aur), "flatpak": word(&presence.flatpak), "snap": word(&presence.snap),
                    "chaotic_aur_off": presence.chaotic_off},
        "ready": ready_rows, "held": held_rows, "unknown": unknown_rows,
        "holds": {"tier1_days": holds.tier1_days, "tier2_days": holds.tier2_days, "tier3_days": holds.tier3_days},
    })
}

/// v1.6.1: the line before pacman's own list of what it skips (named list only).
fn skip_note(skipped: usize) -> Option<String> {
    match skipped {
        0 => None,
        1 => Some("nog: pacman will first list the 1 package it is skipping (it is on hold). That's normal; \
                   only what you named goes in.".to_string()),
        n => Some(format!("nog: pacman will first list the {} packages it is skipping (they are on hold). \
                   That's normal; only what you named goes in.", n)),
    }
}

/// v1.6.1: why each package you named can't go in now, in plain words with
/// what to do instead. Empty means every named one is ready (or promoted).
/// `held` = (name, its held note, the partner it waits on).
fn named_problems(only: &[String], pending: &[&str], held: &[(&str, String, Option<&str>)]) -> Vec<String> {
    let mut out = Vec::new();
    for n in only {
        if pending.contains(&n.as_str()) {
            continue;
        }
        match held.iter().find(|(h, _, _)| *h == n.as_str()) {
            Some((_, note, Some(partner))) => out.push(format!(
                "{} must go in together with {} ({}). Name both: nog update {} {}",
                n, partner, note, n, partner)),
            Some((_, note, None)) if note == "kept back by you" => out.push(format!(
                "{} is named and kept back at the same time (--keep). Leave out one or the other.", n)),
            Some((_, note, None)) => out.push(format!(
                "{} is on hold ({}) and was not promoted. To bring it in now: nog update {} --promote {}",
                n, note, n, n)),
            None => out.push(format!(
                "{} has no update waiting: it is up to date, or not installed.", n)),
        }
    }
    out
}

fn held_note(remaining: u64, reason: &HeldReason) -> String {
    match reason {
        HeldReason::KeptBack => "kept back by you".to_string(),
        HeldReason::Locked => "locked by you — `nog unlock` releases it".to_string(),
        HeldReason::BuiltAfter(l) => match remaining {
            0 => format!("built after the new {} · moves with it", l),
            1 => format!("1 day · built after the new {}, moves with it", l),
            n => format!("{} days · built after the new {}, moves with it", n, l),
        },
        HeldReason::ManualSignoff =>
            "manual sign-off required — run `nog unlock` to release".to_string(),
        // Countdown first, so the Note column stays scannable by its leading
        // number: every held row begins with "N day(s)", coupled or not.
        HeldReason::CoupledTo(partner) => match remaining {
            // No countdown to inherit, so this is not a coupling that clears
            // on a date — it is a block. v1.3.1: the soname rule can name a
            // partner with no pending update at all (a foreign or AUR package
            // built against the old library). "0 days" would read as "releases
            // today", the exact opposite of the truth, so the row says what it
            // means instead and drops the countdown vocabulary entirely.
            0 => format!("blocked by {}", partner),
            1 => format!("1 day · coupled to {}", partner),
            n => format!("{} days · coupled to {}", n, partner),
        },
        HeldReason::Window => match remaining {
            1 => "1 day remaining".to_string(),
            n => format!("{} days remaining", n),
        },
        HeldReason::Waiting(w) => format!("{} · {}", held_note(remaining, &HeldReason::Window), w.describe()),
        HeldReason::SafetyWait(w) => {
            let days = if remaining == 1 { "1 day".to_string() } else { format!("{} days", remaining) };
            match w {
                Some(w) => format!("{} · newest build too new · {}", days, w.describe()),
                None => format!("{} · newest build too new", days),
            }
        }
    }
}

/// v1.5.4 (#28): one row of the SUMMARY table.
#[derive(Debug, Clone, PartialEq)]
struct SummaryRow {
    source: String,
    state: SummaryState,
}

#[derive(Debug, Clone, PartialEq)]
enum SummaryState {
    Counts { ready: usize, held: usize, unknown: usize },
    /// The source's own query failed this run (#25). Never shown as 0.
    CouldNotCheck,
    /// Switched off with `nog deactivate`.
    Off,
}

/// Render the SUMMARY table. Pure + unit-tested. The "Ask you" column (the
/// Unknown bucket) appears only when some row has something in it.
fn format_summary(rows: &[SummaryRow], colorize: bool) -> String {
    let title = "SUMMARY:";
    let mut out = format!("{}\n{}\n\n", title, "=".repeat(title.len()));
    let show_unknown = rows.iter().any(|r| matches!(r.state, SummaryState::Counts { unknown, .. } if unknown > 0));

    let (mut t_ready, mut t_held, mut t_unknown) = (0usize, 0usize, 0usize);
    let mut cells: Vec<(String, Vec<String>)> = Vec::new();
    for r in rows {
        let c = match &r.state {
            SummaryState::Counts { ready, held, unknown } => {
                t_ready += ready; t_held += held; t_unknown += unknown;
                let mut v = vec![ready.to_string(), held.to_string()];
                if show_unknown { v.push(unknown.to_string()); }
                v.push((ready + held + unknown).to_string());
                v
            }
            SummaryState::CouldNotCheck => vec!["could not check".to_string()],
            SummaryState::Off => vec!["off".to_string()],
        };
        cells.push((r.source.clone(), c));
    }

    let mut headers = vec!["Ready now", "On hold"];
    if show_unknown { headers.push("Ask you"); }
    headers.push("Total");
    let mut total = vec![t_ready.to_string(), t_held.to_string()];
    if show_unknown { total.push(t_unknown.to_string()); }
    total.push((t_ready + t_held + t_unknown).to_string());

    let w_src = rows.iter().map(|r| r.source.len()).chain(["Source".len(), "Total".len()]).max().unwrap();
    let widths: Vec<usize> = headers.iter().enumerate().map(|(i, h)| {
        cells.iter().filter(|(_, c)| c.len() == headers.len()).map(|(_, c)| c[i].len())
            .chain([h.len(), total[i].len()]).max().unwrap()
    }).collect();
    let g = "   ";
    let numbers_width: usize = widths.iter().sum::<usize>() + g.len() * (widths.len() - 1);
    let table_width = w_src + g.len() + numbers_width;

    let line = |src: &str, vals: &[String], color: Option<&str>| -> String {
        let src_cell = match color {
            Some(c) => format!("{}{}{}{}", c, src, C_RESET, " ".repeat(w_src - src.len())),
            None => format!("{:<w$}", src, w = w_src),
        };
        if vals.len() == widths.len() {
            let nums: Vec<String> = vals.iter().zip(&widths).map(|(v, w)| format!("{:>w$}", v, w = *w)).collect();
            format!("{}{}{}\n", src_cell, g, nums.join(g))
        } else {
            // "could not check" / "off": one note across the number columns.
            format!("{}{}{}\n", src_cell, g, vals.join(" "))
        }
    };

    let hdr: Vec<String> = headers.iter().zip(&widths).map(|(h, w)| format!("{:>w$}", h, w = *w)).collect();
    out.push_str(&format!("{:<w$}{}{}\n", "Source", g, hdr.join(g), w = w_src));
    out.push_str(&"-".repeat(table_width));
    out.push('\n');
    for (src, c) in &cells {
        let color = if colorize { source_color(src) } else { None };
        out.push_str(&line(src, c, color));
    }
    out.push_str(&"-".repeat(table_width));
    out.push('\n');
    out.push_str(&line("Total", &total, None));
    out
}

/// v1.5.4 (#28): the SUMMARY rows, in a fixed order: the official
/// repositories as pacman.conf lists them, anything else official, then AUR,
/// Flatpak, Snap. A source that is not installed has no row; one switched off
/// says `off`; one whose query failed says `could not check`.
fn summary_rows(
    repos: &[String],
    labels: &HashMap<String, String>,
    ready: &[&PendingUpdate],
    held: &[&PendingUpdate],
    unknown: &[&PendingUpdate],
    states: &SourcePresence,
) -> Vec<SummaryRow> {
    let mut counts: Vec<(String, [usize; 3])> = repos.iter().map(|r| (r.clone(), [0; 3])).collect();
    let mut bump = |label: String, i: usize| {
        match counts.iter_mut().find(|(l, _)| *l == label) {
            Some((_, c)) => c[i] += 1,
            None => { let mut c = [0; 3]; c[i] = 1; counts.push((label, c)); }
        }
    };
    for (i, bucket) in [ready, held, unknown].iter().enumerate() {
        for u in bucket.iter() {
            if u.source == Source::Pacman {
                bump(label_for(labels, u), i);
            }
        }
    }
    let mut rows: Vec<SummaryRow> = counts.into_iter().map(|(source, c)| SummaryRow {
        source,
        state: SummaryState::Counts { ready: c[0], held: c[1], unknown: c[2] },
    }).collect();
    if states.chaotic_off && !repos.iter().any(|r| r == "chaotic-aur") {
        rows.push(SummaryRow { source: "chaotic-aur".into(), state: SummaryState::Off });
    }
    for (name, src, presence) in [
        ("AUR", Source::Aur, &states.aur),
        ("Flatpak", Source::Flatpak, &states.flatpak),
        ("Snap", Source::Snap, &states.snap),
    ] {
        let state = match presence {
            Presence::Absent => continue,
            Presence::Off => SummaryState::Off,
            Presence::Failed => SummaryState::CouldNotCheck,
            Presence::Checked => {
                let n = |b: &[&PendingUpdate]| b.iter().filter(|u| u.source == src).count();
                SummaryState::Counts { ready: n(ready), held: n(held), unknown: n(unknown) }
            }
        };
        rows.push(SummaryRow { source: name.to_string(), state });
    }
    rows
}

/// How each non-pacman source took part in this run.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Presence { Absent, Off, Failed, Checked }

#[derive(Debug, Clone, Copy)]
struct SourcePresence {
    aur: Presence,
    flatpak: Presence,
    snap: Presence,
    chaotic_off: bool,
}

/// v1.5.4 (#28): the Source word for a pending update. Official packages name
/// the repository they come from, read from the same fresh sync databases the
/// hold dates come from; one not found there says `official`.
fn source_label(upd: &PendingUpdate, packages: &HashMap<String, sync_db::PackageDesc>) -> String {
    match upd.source {
        Source::Pacman => packages
            .get(&upd.name)
            .and_then(|d| d.repo.clone())
            .unwrap_or_else(|| "official".to_string()),
        Source::Aur => "AUR".to_string(),
        Source::Flatpak => "Flatpak".to_string(),
        Source::Snap => "Snap".to_string(),
    }
}

fn label_for(labels: &HashMap<String, String>, upd: &PendingUpdate) -> String {
    labels
        .get(&sightings::key(upd.source.as_str(), &upd.name))
        .cloned()
        .unwrap_or_else(|| upd.source.as_str().to_string())
}

fn source_color(source: &str) -> Option<&'static str> {
    match source {
        "AUR" => Some(C_PEACH),
        "Flatpak" => Some(C_BLUE),
        "Snap" => Some(C_MAUVE),
        _ => None,
    }
}

fn print_buckets(
    ready: &[(PendingUpdate, Tier, ReadyReason)],
    held: &[(PendingUpdate, Tier, u64, HeldReason)],
    unknown: &[(PendingUpdate, Tier)],
    labels: &HashMap<String, String>,
) {
    // Ruling #2 of the v2 arc: the user always sees WHERE a package comes
    // from. v1.5.4 (#28): in its own Source column, so the Note column is hold
    // information only. AUR rows used to carry no mark at all.
    let src = |upd: &PendingUpdate| label_for(labels, upd);
    let ready_rows: Vec<TableRow> = ready.iter()
        .map(|(upd, tier, reason)| TableRow::from(upd, tier, src(upd), ready_note(reason)))
        .collect();
    let held_rows: Vec<TableRow> = held.iter()
        .map(|(upd, tier, remaining, reason)| TableRow::from(upd, tier, src(upd), held_note(*remaining, reason)))
        .collect();
    let unknown_rows: Vec<TableRow> = unknown.iter()
        .map(|(upd, tier)| TableRow::from(upd, tier, src(upd), "no build date in sync DB".to_string()))
        .collect();

    println!();
    print!("{}", format_table("READY TO INSTALL", &ready_rows, true));
    println!();
    print!("{}", format_table("ON HOLD FROM INSTALL", &held_rows, true));
    println!();
    print!("{}", format_table("UNKNOWN", &unknown_rows, true));
}

/// Map the final buckets to run-log rows (v1.0.8) — same order and note
/// text as the printed tables, so the CSV is a faithful mirror of what the
/// user saw.
fn runlog_rows(
    ready: &[(PendingUpdate, Tier, ReadyReason)],
    held: &[(PendingUpdate, Tier, u64, HeldReason)],
    unknown: &[(PendingUpdate, Tier)],
) -> Vec<runlog::RunRow> {
    let row = |bucket: &str, upd: &PendingUpdate, tier: &Tier, note: String| runlog::RunRow {
        source: upd.source.as_str().to_string(),
        bucket: bucket.to_string(),
        package: upd.name.clone(),
        old_version: upd.old_version.clone(),
        new_version: upd.new_version.clone(),
        tier: tier_num(tier).to_string(),
        note,
        // Filled in by `settle_rows` once the run has ended.
        outcome: String::new(),
        detail: String::new(),
    };
    let mut rows = Vec::new();
    for (upd, tier, reason) in ready {
        rows.push(row("ready", upd, tier, ready_note(reason)));
    }
    for (upd, tier, remaining, reason) in held {
        rows.push(row("held", upd, tier, held_note(*remaining, reason)));
    }
    for (upd, tier) in unknown {
        rows.push(row("unknown", upd, tier, "no build date in sync DB".to_string()));
    }
    rows
}

/// v1.6.0 (#27): load, update and save the hold record. Returns the record
/// this run evaluates with. Every failure is a warning: without a record the
/// holds fall back to the pre-v1.6.0 rule for this run, never to "no hold".
fn update_hold_record(
    cfg: &NogConfig,
    pending: &[PendingUpdate],
    packages: &HashMap<String, sync_db::PackageDesc>,
    checked_sources: &std::collections::HashSet<&str>,
    now: std::time::SystemTime,
) -> sightings::Sightings {
    let path = runlog::expand_home(&cfg.paths.hold_record);
    let previous = sightings::load(&path);
    let now_ts = now.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);

    // The candidate's build date, only when the date belongs to THIS version
    // (the same guard `holds::evaluate_candidate` applies).
    let seen: Vec<sightings::Seen> = pending.iter().map(|u| {
        let built = packages.get(&u.name)
            .filter(|d| d.version.as_deref().map_or(true, |v| v == u.new_version))
            .map(|d| d.builddate);
        sightings::Seen {
            source: u.source.as_str(),
            name: &u.name,
            installed: &u.old_version,
            candidate: &u.new_version,
            candidate_built: built,
        }
    }).collect();

    // The run logs date holds that began before the record existed. Read
    // only when some package is new to the record — once per run at most.
    let mut logs: Option<Vec<sightings::LogRow>> = None;
    let history = |source: &str, name: &str, installed: &str| {
        let rows = logs.get_or_insert_with(|| {
            sightings::load_logs(&runlog::expand_home(&cfg.paths.run_logs))
        });
        sightings::history_from(rows, source, name, installed)
    };

    let next = sightings::update(&previous, &seen, checked_sources, history, now_ts);
    if next != previous {
        if let Err(e) = sightings::save(&path, &next) {
            eprintln!("{}nog: warning — could not save the hold record: {}{}", C_SUBTEXT, e, C_RESET);
            eprintln!("{}     holds are still applied; a new build may restart a countdown this run.{}",
                C_SUBTEXT, C_RESET);
        }
    }
    next
}

/// Write the run record and prune expired logs (v1.0.8). Every failure path
/// is a warning, never an abort — the update itself has already succeeded or
/// failed on its own terms by the time this runs.
/// What one source's step did this run (issue #19).
#[derive(Debug, Clone, PartialEq)]
enum StepState {
    Done,
    DidNotComplete { code: i32, reason: Option<String> },
}

impl From<&crate::handoff::Handoff> for StepState {
    fn from(h: &crate::handoff::Handoff) -> Self {
        if h.status.success() {
            StepState::Done
        } else {
            StepState::DidNotComplete {
                code: h.status.code().unwrap_or(-1),
                reason: h.reason.clone(),
            }
        }
    }
}

/// How the run ended, from the log's point of view.
enum RunEnd {
    /// Nothing was cleared, so nothing was handed off.
    AllHeld,
    /// The user answered no at nog's own gate, before any handoff.
    Cancelled,
    /// Handoffs ran. A source missing from the map never got its turn —
    /// an earlier step stopped the run.
    Steps(HashMap<Source, StepState>),
}

/// Give every row its own outcome (v1.4.3, issue #19).
///
/// A held package is `held` and a declined unknown is `skipped`, however the
/// run went. Everything else — Ready, and unknowns the user approved — takes
/// the result of **its own source's** step, so an AUR failure no longer
/// colours the pacman rows that installed fine, and a source the run never
/// reached says `not run` rather than borrowing a verdict.
fn settle_rows(rows: &[runlog::RunRow], skipped: &[String], end: &RunEnd) -> Vec<runlog::RunRow> {
    rows.iter()
        .cloned()
        .map(|mut r| {
            let (outcome, detail) = if r.bucket == "held" {
                ("held".to_string(), String::new())
            } else if r.bucket == "unknown" && skipped.contains(&r.package) {
                ("skipped".to_string(), String::new())
            } else {
                match end {
                    RunEnd::Cancelled => ("cancelled".to_string(), String::new()),
                    RunEnd::AllHeld => ("not run".to_string(), String::new()),
                    RunEnd::Steps(steps) => match steps.get(&source_of(&r.source)) {
                        Some(StepState::Done) => ("installed".to_string(), String::new()),
                        Some(StepState::DidNotComplete { code, reason }) => (
                            format!("did not complete (status {})", code),
                            reason.clone().unwrap_or_default(),
                        ),
                        None => ("not run".to_string(), String::new()),
                    },
                }
            };
            r.outcome = outcome;
            r.detail = detail;
            r
        })
        .collect()
}

fn source_of(word: &str) -> Source {
    match word {
        "aur" => Source::Aur,
        "flatpak" => Source::Flatpak,
        "snap" => Source::Snap,
        _ => Source::Pacman,
    }
}

fn write_run_log(
    cfg: &NogConfig,
    date: &str,
    time: &str,
    user: &str,
    rows: Vec<runlog::RunRow>,
    marker: &str,
) {
    let (today, cutoff) = match runlog::today_and_cutoff() {
        Some(pair) => pair,
        None => {
            eprintln!("{}nog: warning — `date` unavailable; run not logged.{}",
                C_SUBTEXT, C_RESET);
            return;
        }
    };
    let record = runlog::RunRecord {
        date: date.to_string(),
        time: time.to_string(),
        user: user.to_string(),
        rows,
        marker: marker.to_string(),
    };
    match runlog::append_run(&cfg.paths.run_logs, &today, &record) {
        Ok(path) => {
            // v1.5.8: named once, in the closing notice of the run.
            record_log(path.clone());
            match runlog::prune_old(&cfg.paths.run_logs, &cutoff) {
                Ok(pruned) if !pruned.is_empty() => println!(
                    "{}nog: pruned {} run log(s) older than {} days.{}",
                    C_SUBTEXT, pruned.len(), runlog::RETENTION_DAYS, C_RESET),
                Ok(_) => {}
                Err(e) => eprintln!("{}nog: warning — run-log prune failed: {}{}",
                    C_SUBTEXT, e, C_RESET),
            }
        }
        Err(e) => eprintln!("{}nog: warning — run not logged: {}{}",
            C_SUBTEXT, e, C_RESET),
    }
}

#[cfg(test)]
mod output_tests {
    use super::*;

    // ---- v2.1.0: nog update --yes (F-20 #56), the password reason (F-21), incomplete (F-22) ----

    #[test]
    fn an_incomplete_update_is_said_in_its_exit_status_and_its_run_line() {
        // F-22 (2026-10-22): snapd's build failed and the run said "done", status 0
        assert_eq!(outcome_word(0), "done");
        assert_eq!(outcome_word(INCOMPLETE), "incomplete");
        assert_eq!(outcome_word(1), "stopped");
        assert_ne!(INCOMPLETE, 0);
        assert_ne!(INCOMPLETE, 1, "1 already means stopped or an error");
        assert_ne!(INCOMPLETE, 2, "2 is a mistyped command (clap)");
    }

    #[test]
    fn a_failed_step_names_the_password_when_that_was_the_cause() {
        let pw = crate::handoff::PASSWORD_TIMED_OUT;
        assert_eq!(failure_label("aur", 1, Some(pw)), format!("aur (status 1: {})", pw));
        // any other reason stays out of the one-line summary (it is on screen and in the log)
        assert_eq!(failure_label("aur", 1, Some("error: target not found: foo")), "aur (status 1)");
        assert_eq!(failure_label("flatpak", 3, None), "flatpak (status 3)");
    }

    #[test]
    fn nobody_at_the_keyboard_for_an_update_is_said_as_the_cause() {
        // F-20 (#56): an update from a script used to read "Cancelled", as if someone answered no
        let l = nobody_to_answer_update_lines().join(" ");
        assert!(l.starts_with("Nothing is at the keyboard"));
        assert!(l.contains("Nothing was changed"));
        assert!(l.contains("nog update --yes"));
        assert!(l.contains("Unknown packages are kept back"));
        assert!(l.contains("AUR's build review waits for a person"));
        assert!(!l.contains("answered no") && !l.contains("Cancelled"));
    }

    #[test]
    fn a_password_nobody_can_type_is_said_before_anything_changes() {
        let l = sudo_needs_a_person_lines().join(" ");
        assert!(l.contains("sudo still asks for a password"));
        assert!(l.contains("Nothing was changed"));
        assert!(l.contains("run pacman without a") && l.contains("NOG_ASKPASS=1"));
    }

    #[test]
    fn yes_never_decides_an_unknown() {
        // F-20 (#56): `yes | nog update` said yes to every Unknown; --yes keeps them all back
        let unknown = vec!["foo".to_string(), "bar".to_string()];
        assert_eq!(keep_unknowns_back(&unknown), unknown);
        assert!(keep_unknowns_back(&[]).is_empty());
    }

    #[test]
    fn table_aligns_and_counts() {
        let rows = vec![
            TableRow { pkg: "libnm".into(), source: "extra".into(), old: "1.56.1-1".into(), new: "1.56.1-2".into(), tier: 2, note: "9 days past window".into() },
            TableRow { pkg: "wine-staging".into(), source: "multilib".into(), old: "11.12-1".into(), new: "11.13-1".into(), tier: 3, note: "hold just expired".into() },
        ];
        let t = format_table("READY TO INSTALL", &rows, false);
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(lines[0], "READY TO INSTALL:");
        assert_eq!(lines[1], "=".repeat("READY TO INSTALL:".len()));
        assert_eq!(lines[2], "");
        let hdr = lines[3];
        assert!(hdr.starts_with("Package (2)"));
        for label in ["Source", "Old Version", "New Version", "Tier", "Note"] {
            assert!(hdr.contains(label), "header missing {label}");
        }
        // The rule under the headers spans the whole table: never shorter than
        // the header row, and exactly as wide as the widest Note cell reaches.
        let rule = lines[4];
        assert!(rule.chars().all(|c| c == '-'), "rule not a dashed rule: {rule:?}");
        assert_eq!(rule.len(), hdr.find("Note").unwrap() + "9 days past window".len());
        assert!(rule.len() >= hdr.len());

        // Alignment guarantee: every column's value begins exactly under its header.
        let (r0, r1) = (lines[5], lines[6]);
        assert!(r0.starts_with("libnm"));
        assert!(r1.starts_with("wine-staging"));
        for (label, v0, v1) in [
            ("Source", "extra", "multilib"),
            ("Old Version", "1.56.1-1", "11.12-1"),
            ("New Version", "1.56.1-2", "11.13-1"),
            ("Tier", "2", "3"),
            ("Note", "9 days past window", "hold just expired"),
        ] {
            let idx = hdr.find(label).unwrap();
            assert!(r0[idx..].starts_with(v0), "row0 {label}: {:?}", &r0[idx..]);
            assert!(r1[idx..].starts_with(v1), "row1 {label}: {:?}", &r1[idx..]);
        }
    }

    /// The rule is measured in characters, not bytes: a note carrying `·` or
    /// `—` must not push it past the row it is supposed to underline.
    #[test]
    fn rule_measures_notes_in_characters_not_bytes() {
        let rows = vec![
            TableRow { pkg: "elfutils".into(), source: "core".into(), old: "0.195-1".into(), new: "0.196-1".into(),
                       tier: 3, note: "3 days · coupled to lib32-libelf".into() },
        ];
        let t = format_table("ON HOLD FROM INSTALL", &rows, false);
        let lines: Vec<&str> = t.lines().collect();
        let (rule, row) = (lines[4], lines[5]);
        assert_eq!(
            rule.chars().count(),
            row.chars().count(),
            "rule and its widest row disagree:\n{rule}\n{row}"
        );
    }


    // ---- v1.5.4: the SUMMARY table and the Source column (#28, #25) ---------

    fn upd(name: &str, source: Source) -> PendingUpdate {
        PendingUpdate { name: name.into(), old_version: "1".into(), new_version: "2".into(), source }
    }

    fn all_checked() -> SourcePresence {
        SourcePresence { aur: Presence::Checked, flatpak: Presence::Checked, snap: Presence::Absent, chaotic_off: false }
    }

    #[test]
    fn the_summary_counts_by_repository_in_pacman_conf_order() {
        let repos: Vec<String> = ["core", "extra", "multilib"].iter().map(|s| s.to_string()).collect();
        let (a, b, c, d) = (upd("linux-zen", Source::Pacman), upd("vim", Source::Pacman),
                            upd("fresh-editor-bin", Source::Aur), upd("git", Source::Pacman));
        let labels: HashMap<String, String> = [
            ("pacman:linux-zen", "extra"), ("pacman:vim", "extra"), ("pacman:git", "core"),
            ("aur:fresh-editor-bin", "AUR"),
        ].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let rows = summary_rows(&repos, &labels, &[&a], &[&b, &c, &d], &[], &all_checked());
        let names: Vec<&str> = rows.iter().map(|r| r.source.as_str()).collect();
        assert_eq!(names, vec!["core", "extra", "multilib", "AUR", "Flatpak"], "Snap is not installed: no row");
        assert_eq!(rows[1].state, SummaryState::Counts { ready: 1, held: 1, unknown: 0 });
        assert_eq!(rows[3].state, SummaryState::Counts { ready: 0, held: 1, unknown: 0 });
    }

    #[test]
    fn a_failed_source_says_so_and_is_never_zero() {
        // #25: the whole point. A failed AUR query used to read "0 AUR updates".
        let mut p = all_checked();
        p.aur = Presence::Failed;
        p.flatpak = Presence::Off;
        let rows = summary_rows(&[], &HashMap::new(), &[], &[], &[], &p);
        assert_eq!(rows[0], SummaryRow { source: "AUR".into(), state: SummaryState::CouldNotCheck });
        assert_eq!(rows[1], SummaryRow { source: "Flatpak".into(), state: SummaryState::Off });
        let t = format_summary(&rows, false);
        assert!(t.contains("AUR       could not check") || t.lines().any(|l| l.starts_with("AUR") && l.ends_with("could not check")), "{t}");
        assert!(!t.lines().any(|l| l.starts_with("AUR") && l.contains(" 0")), "{t}");
    }

    #[test]
    fn the_summary_aligns_and_totals() {
        let rows = vec![
            SummaryRow { source: "core".into(), state: SummaryState::Counts { ready: 1, held: 6, unknown: 0 } },
            SummaryRow { source: "chaotic-aur".into(), state: SummaryState::Counts { ready: 10, held: 45, unknown: 0 } },
            SummaryRow { source: "Snap".into(), state: SummaryState::Off },
        ];
        let t = format_summary(&rows, false);
        let l: Vec<&str> = t.lines().collect();
        assert_eq!(l[0], "SUMMARY:");
        assert!(!l[3].contains("Ask you"), "no Unknown anywhere: no column");
        let hdr = l[3];
        let end = |label: &str| hdr.find(label).unwrap() + label.len();
        // Numbers are right-aligned under their headers.
        assert_eq!(l[5].find('1').unwrap() + 1, end("Ready now"));
        assert_eq!(&l[6][end("On hold") - 2..end("On hold")], "45");
        assert!(l[7].starts_with("Snap") && l[7].ends_with("off"));
        let total = l.last().unwrap();
        assert!(total.starts_with("Total"));
        assert_eq!(&total[end("Total") - 2..end("Total")], "62");
        assert_eq!(l[4].len(), l[8].len(), "both rules the same width");
    }

    #[test]
    fn the_ask_you_column_appears_only_when_needed() {
        let rows = vec![SummaryRow { source: "extra".into(), state: SummaryState::Counts { ready: 0, held: 1, unknown: 2 } }];
        let t = format_summary(&rows, false);
        assert!(t.contains("Ask you"));
        assert!(t.lines().last().unwrap().trim_end().ends_with('3'));
    }

    #[test]
    fn every_source_has_a_word_and_aur_is_never_blank() {
        let mut pk: HashMap<String, sync_db::PackageDesc> = HashMap::new();
        pk.insert("snapd".into(), sync_db::PackageDesc {
            builddate: 0, pkgbase: None, version: None, provides: Vec::new(), repo: Some("extra".into()),
        });
        assert_eq!(source_label(&upd("snapd", Source::Pacman), &pk), "extra");
        assert_eq!(source_label(&upd("snapd", Source::Snap), &pk), "Snap", "the #20 trap: same name, other source");
        assert_eq!(source_label(&upd("walker-bin", Source::Aur), &pk), "AUR");
        assert_eq!(source_label(&upd("org.x", Source::Flatpak), &pk), "Flatpak");
        assert_eq!(source_label(&upd("ghost", Source::Pacman), &pk), "official");
    }

    #[test]
    fn colour_never_moves_a_column() {
        let rows = vec![
            TableRow { pkg: "a".into(), source: "AUR".into(), old: "1".into(), new: "2".into(), tier: 3, note: "x".into() },
            TableRow { pkg: "b".into(), source: "extra".into(), old: "1".into(), new: "2".into(), tier: 3, note: "y".into() },
        ];
        let strip = |t: String| { let mut o = String::new(); let mut esc = false;
            for c in t.chars() { if c == '\x1b' { esc = true; continue; } if esc { if c == 'm' { esc = false; } continue; } o.push(c); } o };
        assert_eq!(strip(format_table("READY TO INSTALL", &rows, true)), format_table("READY TO INSTALL", &rows, false));
    }

    #[test]
    fn the_clean_report_aligns_and_totals() {
        let rows = vec![
            CleanRow { what: "No longer installed".into(), versions: 300, bytes: 2 * 1024 * 1024 * 1024 },
            CleanRow { what: "Tier 3 · older than the newest 1".into(), versions: 2100, bytes: 11 * 1024 * 1024 * 1024 },
        ];
        let t = format_clean_report(&rows, 134);
        let l: Vec<&str> = t.lines().collect();
        assert_eq!(l[0], "WHAT nog clean WOULD REMOVE:");
        assert!(l[3].starts_with("What") && l[3].trim_end().ends_with("Size"));
        assert!(l[5].starts_with("No longer installed") && l[5].ends_with("2.0 GB"));
        assert!(l[6].starts_with("Tier 3 · older") && l[6].ends_with("11.0 GB"));
        assert!(l[7].starts_with("Leftover download folders") && l[7].ends_with("size not readable"));
        assert!(l[9].starts_with("Total") && l[9].ends_with("13.0 GB") && l[9].contains("2400"));
        assert_eq!(l[4].chars().count(), l[5].chars().count(), "rule as wide as a row");
    }

    #[test]
    fn an_empty_clean_report_says_nothing() {
        assert!(format_clean_report(&[], 0).ends_with("(nothing)\n"));
    }

    #[test]
    fn sizes_read_like_du() {
        assert_eq!(human_size(19 * 1024 * 1024 * 1024 + 50 * 1024 * 1024), "19.0 GB");
        assert_eq!(human_size(812 * 1024 * 1024), "812 MB");
        assert_eq!(human_size(4096), "4 KB");
        assert_eq!(human_size(12), "12 B");
    }

    #[test]
    fn empty_table_renders_none() {
        let t = format_table("UNKNOWN", &[], false);
        assert!(t.starts_with("UNKNOWN:\n"));
        assert!(t.contains("\n\n(none)\n"));
    }

    /// Every Held note that *has* a countdown opens with it, so the Note
    /// column can be scanned down its leading number. A coupled row is no
    /// exception — the partner is the tail of the note, never its head.
    #[test]
    fn promoted_says_so_and_names_what_it_moved_with() {
        // v1.6.0 (#7): Promote makes it ready (Javier, 3 Oct: "shouldn't promote just bring the
        // package to due, so it enters the ready list?"); its partner comes along, and says why
        assert_eq!(ready_note(&ReadyReason::Promoted(None)), "promoted by you");
        assert_eq!(ready_note(&ReadyReason::Promoted(Some("linux-zen".into()))), "promoted with linux-zen");
    }

    #[test]
    fn kept_back_says_so_and_has_no_date() {
        // v1.6.0 (#7): what you kept back (nogForge's unticked box) is not a countdown
        assert_eq!(held_note(0, &HeldReason::KeptBack), "kept back by you");
        assert_eq!(held_note(12, &HeldReason::KeptBack), "kept back by you");
    }

    #[test]
    fn a_named_list_stops_on_a_held_one_and_says_what_to_do() {
        // v1.6.1 (Javier, 4 Oct 2026): "messages in case a package that is being
        // held is requested to be updated without being promoted first, and don't execute it"
        let only: Vec<String> = ["vde2", "git", "ldb", "nope", "freerdp"].iter().map(|s| s.to_string()).collect();
        let held = vec![
            ("git", "1 day remaining".to_string(), None),
            ("ldb", "3 days · coupled to libwbclient".to_string(), Some("libwbclient")),
            ("freerdp", "kept back by you".to_string(), None),
        ];
        let p = named_problems(&only, &["vde2", "wolfssl"], &held);
        assert_eq!(p.len(), 4, "vde2 is ready: no problem; the other four each get one line");
        assert_eq!(p[0], "git is on hold (1 day remaining) and was not promoted. To bring it in now: nog update git --promote git");
        assert!(p[1].starts_with("ldb must go in together with libwbclient"));
        assert!(p[1].ends_with("Name both: nog update ldb libwbclient"));
        assert!(p[2].starts_with("nope has no update waiting"));
        assert!(p[3].contains("kept back at the same time"));
        assert!(named_problems(&only[..1], &["vde2"], &held).is_empty(), "all ready: the run goes on");
    }

    fn desc(repo: &str, version: &str, provides: &[&str]) -> sync_db::PackageDesc {
        sync_db::PackageDesc { builddate: 0, pkgbase: None, version: Some(version.into()),
            provides: provides.iter().map(|s| s.to_string()).collect(), repo: Some(repo.into()) }
    }

    #[test]
    fn an_install_shows_where_each_package_comes_from_in_pacmans_order() {
        // v1.8.0 (Javier, 4 Oct 2026, option A)
        let mut official = HashMap::new();
        official.insert("cowsay".to_string(), desc("extra", "3.8.4-1", &[]));
        official.insert("unifetch".to_string(), desc("chaotic-aur", "1.3.3-2", &["neofetch=7.1.0"]));
        official.insert("jre-a".to_string(), desc("extra", "21-1", &["java-runtime"]));
        official.insert("jre-b".to_string(), desc("extra", "25-1", &["java-runtime"]));
        let aur = |n: &str| if n == "neofetch" || n == "pfetch-git" { Some("7.1.0-2".to_string()) } else { None };
        let resolves = |n: &str| n == "base-devel";
        let r = |n: &str| resolve_install(n, &official, &aur, &resolves);
        assert_eq!(r("cowsay"), InstallSource::Repo { repo: "extra".into(), version: Some("3.8.4-1".into()), providers: vec![] });
        assert_eq!(r("neofetch"), InstallSource::Repo { repo: "chaotic-aur".into(), version: Some("1.3.3-2".into()),
            providers: vec!["unifetch".into()] }, "a plain name: a repository provider wins, as in pacman (and it says so)");
        assert_eq!(r("aur/neofetch"), InstallSource::Aur { version: Some("7.1.0-2".into()) }, "aur/ is the AUR's own");
        assert!(matches!(r("java-runtime"), InstallSource::Repo { ref providers, version: None, .. } if providers.len() == 2));
        assert_eq!(r("base-devel"), InstallSource::Group);
        assert_eq!(r("pfetch-git"), InstallSource::Aur { version: Some("7.1.0-2".into()) });
        assert_eq!(r("zzz-nope"), InstallSource::Nowhere);
        assert_eq!(r("extra/cowsay"), InstallSource::Repo { repo: "extra".into(), version: Some("3.8.4-1".into()), providers: vec![] });
    }

    #[test]
    fn the_install_table_and_who_takes_over() {
        let rows = vec![["cowsay".to_string(), "3.8.4-1".into(), "extra".into(), "Tier 3".into()],
                        ["pfetch-git".to_string(), "r432-1".into(), "AUR — built by yay".into(), "Tier 3".into()]];
        let t = format_install_table(&rows);
        let lines: Vec<&str> = t.lines().collect();
        assert!(lines[0].trim_start().starts_with("Package"));
        assert!(lines[3].contains("AUR — built by yay"));
        assert_eq!(lines[2].find("extra"), lines[3].find("AUR"), "the Source column lines up");
        assert!(handoff_line(true, "yay", false).starts_with("nog: Handing off to yay: it shows each build recipe"));
        assert!(handoff_line(false, "yay", false).starts_with("nog: Handing off to pacman: it shows what comes with it and asks"));
        // v1.9.0 (#32): with --yes pacman is told the answer, and the line says so
        assert!(handoff_line(false, "yay", true).contains("--yes"));
        assert!(handoff_line(false, "yay", true).contains("without asking"));
        assert!(flatpak_handoff_line(false).contains("asks before anything changes"));
        assert!(flatpak_handoff_line(true).contains("--yes"));
    }

    #[test]
    fn nogs_own_tier_list_carries_no_distro_choices() {
        // Javier, 2026-10-23: trim nog's default to generic packages; the desktop
        // and its apps belong in KognogOS's own, fuller list.
        let tm = TierManager::from_toml_for_tests(include_str!("../../config/tier-pins.toml"));
        for app in ["plasma-meta", "plasma-desktop", "sddm", "firefox", "dolphin", "konsole", "kate",
                    "grubforge", "alacritty", "fish", "alacrittyforge"] {
            assert_eq!(tm.classify(app), Tier::Three, "{} is a distro's choice", app);
        }
        for generic in ["pipewire", "pipewire-pulse", "wireplumber", "networkmanager"] {
            assert_eq!(tm.classify(generic), Tier::Two, "{}", generic);
        }
        assert_eq!(tm.classify("glibc"), Tier::One);
    }

    #[test]
    fn the_install_table_says_what_flatpak_and_snap_install() {
        // v1.9.0 (C3): the app ID differs from the name typed, so it is said; a
        // classic snap says what classic means before nog asks.
        let tm = TierManager::from_toml_for_tests(include_str!("../../config/tier-pins.toml"));
        let sources = vec![
            ("gimp".to_string(), InstallSource::Flatpak { app: flatpak::FlatpakApp {
                app_id: "org.gimp.GIMP".into(), version: "3.2.6".into(), name: "GIMP".into(), remote: "flathub".into() } }),
            ("flatpak/org.gimp.GIMP".to_string(), InstallSource::Flatpak { app: flatpak::FlatpakApp {
                app_id: "org.gimp.GIMP".into(), version: "".into(), name: "GIMP".into(), remote: "flathub".into() } }),
            ("hello-world".to_string(), InstallSource::Snap { info: snap::SnapInfo {
                name: "hello-world".into(), version: "6.4".into(), publisher: "Canonical".into(), confinement: "strict".into() } }),
            ("code".to_string(), InstallSource::Snap { info: snap::SnapInfo {
                name: "code".into(), version: "1.0".into(), publisher: "vscode".into(), confinement: "classic".into() } }),
        ];
        let rows = install_rows(&sources, &tm, "yay");
        assert_eq!(rows[0], ["gimp", "3.2.6", "Flatpak — flathub, as org.gimp.GIMP", "Tier 3"]);
        assert_eq!(rows[1][1], "—");
        assert_eq!(rows[1][2], "Flatpak — flathub");
        assert_eq!(rows[2][2], "Snap — sandboxed");
        assert!(rows[3][2].starts_with("Snap — classic: may change anything"));
    }

    #[test]
    fn several_answers_say_where_nog_looked_and_number_the_options() {
        // F-15 (#51), Javier 2026-10-14: "Package not available in the repositories.
        // Package not available in the AUR. Flatpak options: 1. 2. 3. Which one?"
        let options = vec![
            InstallSource::Flatpak { app: flatpak::FlatpakApp { app_id: "org.one.Editor".into(), version: "1".into(), name: "Editor".into(), remote: "flathub".into() } },
            InstallSource::Flatpak { app: flatpak::FlatpakApp { app_id: "org.two.Editor".into(), version: "".into(), name: "Editor".into(), remote: "flathub".into() } },
            InstallSource::Snap { info: snap::SnapInfo { name: "editor".into(), version: "2.0".into(), publisher: "Someone".into(), confinement: "strict".into() } },
        ];
        let l = where_looked_lines("editor", true, true, &options);
        assert_eq!(l[0], "Not in the repositories.");
        assert_eq!(l[1], "Not in the AUR.");
        assert_eq!(l[2], "Flatpak, 2 apps answer to 'editor':");
        assert_eq!(l[3].trim(), "1. org.one.Editor  Editor, 1 — flathub");
        assert_eq!(l[4].trim(), "2. org.two.Editor  Editor — flathub");
        assert_eq!(l[5], "Snap has one too:");
        assert_eq!(l[6].trim(), "3. editor          Snap — sandboxed, 2.0, by Someone");
        // No helper: the AUR was not looked in, so it is not claimed.
        let l = where_looked_lines("editor", true, false, &options[..2]);
        assert_eq!(l[1], "Flatpak, 2 apps answer to 'editor':");
        assert!(!l.iter().any(|x| x.contains("Snap")));
        // `flatpak/editor`: only Flatpak was looked in, so nothing else is claimed.
        let l = where_looked_lines("editor", false, false, &options[..2]);
        assert_eq!(l[0], "Flatpak, 2 apps answer to 'editor':");
        // A script's shell: the list stands, and the way out names both kinds.
        let n = nobody_to_pick_lines(&options);
        assert!(n[0].starts_with("Nothing is at the keyboard to pick one"));
        assert_eq!(n[1], "Name the one you want: nog install flatpak/org.one.Editor, or nog install snap/editor");
        assert_eq!(nobody_to_pick_lines(&options[..2])[1], "Name the one you want: nog install flatpak/org.one.Editor");
    }

    #[test]
    fn the_pick_is_a_number_on_the_list_and_enter_or_n_is_none() {
        assert_eq!(parse_pick("1\n", 5), Pick::Chosen(0));
        assert_eq!(parse_pick(" 5 ", 5), Pick::Chosen(4));
        assert_eq!(parse_pick("\n", 5), Pick::None);
        assert_eq!(parse_pick("n\n", 5), Pick::None);
        assert_eq!(parse_pick("N", 5), Pick::None);
        assert_eq!(parse_pick("no", 5), Pick::None);
        // Off the list, or not a number: ask again, never install option 0 or 6.
        assert_eq!(parse_pick("0", 5), Pick::Invalid);
        assert_eq!(parse_pick("6", 5), Pick::Invalid);
        assert_eq!(parse_pick("y", 5), Pick::Invalid);
        assert_eq!(parse_pick("org.one.Editor", 5), Pick::Invalid);
    }

    #[test]
    fn nobody_at_the_keyboard_is_said_as_the_cause_not_as_a_no() {
        // v1.9.0 (#32, seen 2026-10-07): three runs from a shell with no terminal
        // were reported as "you answered no". Nobody answered anything.
        let l = nobody_to_answer_lines("install");
        assert!(l[0].starts_with("Nothing is at the keyboard"));
        assert!(l[1].contains("flatpak's"), "a Flatpak install is asked by flatpak, not pacman");
        assert!(l[2].contains("nog install --yes"));
        assert!(l[3].contains("never the AUR"));
        assert!(!l.join(" ").contains("answered no"));
        assert!(nobody_to_answer_lines("remove")[2].contains("nog remove --yes"));
    }

    #[test]
    fn the_banner_rule_never_passes_80_columns() {
        assert_eq!(rule_width("nog v1.7.0  ·  Update", "Run:   nog update"), 41, "short runs keep the classic width");
        assert_eq!(rule_width("nog v1.7.0  ·  Install", &format!("Run:   nog install {}", "x".repeat(60))), 80);
        assert_eq!(rule_width("h", &"y".repeat(330)), RULE_MAX, "four file paths: capped, not 330");
    }

    #[test]
    fn a_repo_or_aur_prefix_is_kept_for_the_tool_and_dropped_for_nog() {
        // v1.7.0 (F-11, the KognogOS VM 4 Oct 2026): "neofetch" picked from the AUR
        // was installed as chaotic-aur's unifetch, which provides neofetch
        assert_eq!(bare_name("aur/neofetch"), "neofetch");
        assert_eq!(bare_name("extra/git"), "git");
        assert_eq!(bare_name("git"), "git");
        // aur/ is always the AUR, even when a repository provides the name:
        let official: HashMap<String, sync_db::PackageDesc> = [("neofetch".to_string(), desc("chaotic-aur", "7.1.0-2", &[]))].into();
        let aur = |n: &str| (n == "neofetch").then(|| "7.1.0-2".to_string());
        assert_eq!(resolve_install("aur/neofetch", &official, &aur, &|_| false),
                   InstallSource::Aur { version: Some("7.1.0-2".into()) });
        // v1.9.0 (C3): flatpak/ and snap/ are left for the chain, never pacman's
        assert_eq!(resolve_install("flatpak/org.gimp.GIMP", &official, &aur, &|_| true), InstallSource::Nowhere);
        assert_eq!(resolve_install("snap/hello", &official, &aur, &|_| true), InstallSource::Nowhere);
    }

    #[test]
    fn a_name_found_nowhere_is_said_plainly() {
        // v1.6.1 (F-9, #43, Javier 3 Oct 2026): "nogforge" wasn't on the AUR, and nog said
        // "pacman stopped: you answered no" — nobody was asked, and it wasn't pacman.
        let l = not_found_lines(&["nogforge".to_string()], &["the repositories", "the AUR"], &[]);
        assert_eq!(l[0], "nogforge was not found in the repositories or the AUR.");
        assert!(l[1].starts_with("Nothing was asked and nothing was changed."));
        assert!(l[1].contains("`nog search nogforge`"));
        let l = not_found_lines(&["a".to_string(), "b".to_string()], &["the repositories"], &[]);
        assert_eq!(l[0], "a, b were not found in the repositories.", "no helper: no AUR named");
        assert!(!l.join(" ").contains("answered no"));
        // v1.9.0 (C3): the whole chain named, and why a source was skipped
        let l = not_found_lines(&["wonderpen".to_string()], &["the repositories", "the AUR", "Flatpak", "Snap"],
            &["Snap was not checked: it is deactivated (`nog activate snap` re-enables).".to_string()]);
        assert_eq!(l[0], "wonderpen was not found in the repositories, the AUR, Flatpak or Snap.");
        assert_eq!(l[1], "Snap was not checked: it is deactivated (`nog activate snap` re-enables).");
        assert!(l[2].starts_with("Nothing was asked"));
    }

    #[test]
    fn a_named_list_says_pacman_will_list_what_it_skips() {
        // v1.6.1 (Javier, 4 Oct 2026: option b) — the ~70 "ignoring package upgrade" lines explained first
        assert_eq!(skip_note(0), None, "nothing skipped: no line");
        assert!(skip_note(1).unwrap().contains("the 1 package it is skipping (it is on hold)"));
        let n = skip_note(70).unwrap();
        assert!(n.contains("the 70 packages it is skipping (they are on hold)"));
        assert!(n.ends_with("only what you named goes in."));
    }

    #[test]
    fn held_notes_all_lead_with_the_countdown() {
        let cases = [
            (1, HeldReason::Window, "1 day remaining"),
            (12, HeldReason::Window, "12 days remaining"),
            (1, HeldReason::CoupledTo("lib32-libelf".into()), "1 day · coupled to lib32-libelf"),
            (3, HeldReason::CoupledTo("lib32-libelf".into()), "3 days · coupled to lib32-libelf"),
        ];
        for (remaining, reason, expected) in cases {
            let note = held_note(remaining, &reason);
            assert_eq!(note, expected);
            assert!(
                note.starts_with(&remaining.to_string()),
                "note does not lead with its countdown: {note:?}"
            );
        }
    }

    #[test]
    fn stale_or_missing_lists_need_a_refresh() {
        // #30: the copied disc's lists were weeks old; a missing list is a refresh.
        let m = LISTS_MAX_AGE_SECS;
        assert_eq!(refresh_reason(None, Some(60), m), Some(RefreshReason::ListsMissing));
        assert_eq!(refresh_reason(Some(3600), None, m), Some(RefreshReason::NoRecord));
        assert_eq!(refresh_reason(Some(3 * 86_400), Some(2 * 86_400), m), Some(RefreshReason::RecordOld(2 * 86_400)));
        // F-3: pacman refreshed the lists, then its prompt was declined —
        // lists newer than the last completed update.
        assert_eq!(refresh_reason(Some(60), Some(3600), m), Some(RefreshReason::ListsChangedOutside));
        // A completed update: lists synced, then the stamp written.
        assert_eq!(refresh_reason(Some(3700), Some(3600), m), None);
        assert_eq!(refresh_reason(Some(m), Some(m), m), None);
        // Lists older than a day but synced at the last update (pacman leaves
        // an unchanged list untouched): no refresh needed.
        assert_eq!(refresh_reason(Some(5 * 86_400), Some(3600), m), None);
        assert_eq!(age_words(20 * 86_400 + 5), "20 days");
        assert_eq!(RefreshReason::RecordOld(2 * 86_400).words(), "the last completed update was 2 days ago");
        assert_eq!(age_words(86_400), "1 day");
        assert_eq!(age_words(3 * 3600 + 9), "3 hours");
    }

    #[test]
    fn the_banner_names_the_run_and_keeps_the_blank_line_rule() {
        let b = format_banner("install cowsay", "10/02/2026", "11:30 AM", "javier");
        let printed = format!("{}\n", b);
        assert!(printed.starts_with("\n") && !printed.starts_with("\n\n"));
        assert!(printed.ends_with("javier\n\n") && !printed.ends_with("\n\n\n"));
        assert!(b.contains("  ·  Install"));
        assert!(b.contains("Run:   nog install cowsay"));
        assert!(b.contains("Date:  10/02/2026   11:30 AM"));
    }

    #[test]
    fn a_notice_has_one_blank_line_before_and_one_after() {
        // F-7 (#39), Javier's rule: exactly one blank line around a message.
        // format_notice carries the leading one; notice()'s println ends the
        // last line and adds the trailing one — never two, never none.
        let t = format_notice("", "Heading", &["first".to_string(), "second".to_string()]);
        let printed = format!("{}\n", t);
        assert!(printed.starts_with("\n") && !printed.starts_with("\n\n"));
        assert!(printed.ends_with("second\n\n") && !printed.ends_with("\n\n\n"));
        assert!(t.contains("==> Heading"));
        assert!(t.contains("\n    first\n    second\n"));
    }

    #[test]
    fn checkupdates_hint_names_fakeroot() {
        // F-2 (#34): the exact message from the KognogOS VM.
        let h = checkupdates_hint("==> ERROR: Cannot find the fakeroot binary").unwrap();
        assert!(h.contains("sudo pacman -Syu fakeroot"));
        assert!(checkupdates_hint("error: failed to synchronize all databases").is_none());
    }

    #[test]
    fn a_failed_update_is_not_done() {
        // F-4 (#36): nog install refuses on anything but Done.
        assert_ne!(UpdateEnd::Failed(1), UpdateEnd::Done);
        assert_ne!(UpdateEnd::Stopped, UpdateEnd::Done);
    }

    #[test]
    fn keyrings_say_why_they_were_not_held() {
        assert!(KEYRINGS.contains(&"archlinux-keyring") && KEYRINGS.contains(&"chaotic-keyring"));
        assert_eq!(ready_note(&ReadyReason::Keys), "keys · never held, installed first");
    }

    #[test]
    fn a_long_wait_says_so_and_still_leads_with_its_countdown() {
        // #27: newer builds no longer restart a hold, and the row shows it.
        let w = Waited { since: "Jul 29".into(), skipped: 9 };
        assert_eq!(
            held_note(20, &HeldReason::Waiting(w.clone())),
            "20 days remaining · waiting since Jul 29 · 9 newer versions skipped"
        );
        assert_eq!(
            held_note(3, &HeldReason::SafetyWait(Some(w.clone()))),
            "3 days · newest build too new · waiting since Jul 29 · 9 newer versions skipped"
        );
        assert_eq!(held_note(1, &HeldReason::SafetyWait(None)), "1 day · newest build too new");
        assert_eq!(
            ready_note(&ReadyReason::AfterWaiting(Waited { since: "Aug 4".into(), skipped: 1 })),
            "waiting since Aug 4 · 1 newer version skipped"
        );
    }

    // ---- v1.4.3: the run log tells the truth (#19, #20, #21, #22) -----------

    fn lrow(source: &str, bucket: &str, pkg: &str) -> runlog::RunRow {
        runlog::RunRow {
            source: source.into(),
            bucket: bucket.into(),
            package: pkg.into(),
            old_version: "1-1".into(),
            new_version: "2-1".into(),
            tier: "3".into(),
            note: String::new(),
            outcome: String::new(),
            detail: String::new(),
        }
    }

    fn outcomes(rows: &[runlog::RunRow]) -> Vec<(String, String, String)> {
        rows.iter()
            .map(|r| (r.package.clone(), r.outcome.clone(), r.detail.clone()))
            .collect()
    }

    #[test]
    fn held_rows_say_held_on_a_run_that_installed() {
        // The 2026-09-15 run: gimp held, 7zip ready, pacman step fine.
        let rows = vec![lrow("pacman", "ready", "7zip"), lrow("pacman", "held", "gimp")];
        let steps: HashMap<Source, StepState> = [(Source::Pacman, StepState::Done)].into();
        let out = outcomes(&settle_rows(&rows, &[], &RunEnd::Steps(steps)));
        assert_eq!(out[0].1, "installed");
        assert_eq!(out[1].1, "held", "a held package was logged as {:?}", out[1].1);
    }

    #[test]
    fn each_source_gets_its_own_steps_result() {
        // pacman fine, the AUR build failed and the user stopped there, so
        // flatpak never ran. Three sources, three different truths.
        let rows = vec![
            lrow("pacman", "ready", "7zip"),
            lrow("aur", "ready", "discord"),
            lrow("flatpak", "ready", "org.gimp.GIMP"),
        ];
        let steps: HashMap<Source, StepState> = [
            (Source::Pacman, StepState::Done),
            (Source::Aur, StepState::DidNotComplete {
                code: 1,
                reason: Some("error: failed to build 'discord'".into()),
            }),
        ].into();
        let out = outcomes(&settle_rows(&rows, &[], &RunEnd::Steps(steps)));
        assert_eq!(out[0], ("7zip".into(), "installed".into(), "".into()));
        assert_eq!(
            out[1],
            ("discord".into(), "did not complete (status 1)".into(), "error: failed to build 'discord'".into())
        );
        assert_eq!(out[2].1, "not run");
    }

    #[test]
    fn a_skipped_unknown_and_a_cancelled_run_say_so() {
        let rows = vec![
            lrow("pacman", "ready", "7zip"),
            lrow("aur", "unknown", "odd-pkg"),
            lrow("pacman", "held", "gimp"),
        ];
        let out = outcomes(&settle_rows(&rows, &["odd-pkg".into()], &RunEnd::Cancelled));
        assert_eq!(out[0].1, "cancelled");
        assert_eq!(out[1].1, "skipped");
        assert_eq!(out[2].1, "held");
    }

    #[test]
    fn source_words_round_trip() {
        for s in [Source::Pacman, Source::Aur, Source::Flatpak, Source::Snap] {
            assert_eq!(source_of(s.as_str()), s);
        }
    }

    #[test]
    fn reboot_advice_lines_keep_their_heading_level() {
        let lines: Vec<String> = [
            "IMPORTANT: the running kernel is no longer installed.",
            "  Reboot before loading any new module.",
            "",
            "NOTE: glibc was updated; a reboot is recommended.",
        ].iter().map(|s| s.to_string()).collect();
        let got = advice_levels(&lines);
        assert_eq!(got.len(), 3, "blank spacer lines are not advice");
        assert_eq!(got[0].0, "IMPORTANT");
        assert_eq!(got[1], ("IMPORTANT".into(), "Reboot before loading any new module.".into()));
        assert_eq!(got[2].0, "NOTE");
    }

    // ---- issue #17: nog install <file> ------------------------------------

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn names_stay_names() {
        assert_eq!(split_install_args(&args(&["gimp", "vlc"]), |_| false), Ok(InstallArgs::Names));
    }

    #[test]
    fn a_built_package_is_a_file_install() {
        let a = args(&["./grubforge-1.1.1-1-any.pkg.tar.zst"]);
        assert_eq!(split_install_args(&a, |_| true), Ok(InstallArgs::Files));
    }

    #[test]
    fn names_and_files_together_are_refused() {
        let a = args(&["gimp", "./grubforge-1.1.1-1-any.pkg.tar.zst"]);
        let e = split_install_args(&a, |_| true).unwrap_err();
        assert!(e.contains("cannot be mixed"), "{e}");
    }

    #[test]
    fn a_mistyped_path_is_a_missing_file_not_a_package_name() {
        let a = args(&["./grubforge-1.1.1-1-any.pkg.tar.zts"]);
        let e = split_install_args(&a, |_| false).unwrap_err();
        assert!(e.starts_with("no such package file"), "{e}");
    }

    /// The one row with no countdown (v1.3.1, issue #13): a soname coupling
    /// whose partner has no pending update never clears on a date, so it must
    /// not borrow the countdown vocabulary — and must not open with a digit,
    /// which is what the scan-the-column habit reads as a countdown.
    #[test]
    fn a_block_with_no_end_date_says_so_instead_of_counting_down() {
        let note = held_note(0, &HeldReason::CoupledTo("some-aur-pkg".into()));
        assert_eq!(note, "blocked by some-aur-pkg");
        assert!(!note.starts_with(|c: char| c.is_ascii_digit()));
        assert!(!note.contains("day"));
    }
}

pub fn pin(package: &str, tier: u8) {
    let cfg = load_config();
    let current = load_tiers().classify(package);
    println!("nog: pinning '{}' to tier {} (currently {})...", package, tier, current);

    match crate::tiers::pin_package(&cfg.paths.tier_pins, package, tier) {
        Ok(()) => println!(
            "nog: '{}' successfully pinned to tier {}. Change saved to {}.",
            package, tier, cfg.paths.tier_pins
        ),
        Err(e) => {
            eprintln!("nog: failed to pin '{}': {}", package, e);
            end(1);
        }
    }
}

fn load_tiers() -> TierManager {
    let cfg = NogConfig::load_default();
    let tm = TierManager::load(&cfg.paths.tier_pins).unwrap_or_else(|e| {
        // Use a clean user-facing error rather than a Rust panic — a panic
        // emits an unhelpful backtrace hint and a "fatal" line that reads
        // like an internal error. This path is reachable when the user has
        // a broken install (missing tier-pins.toml, permissions issue, etc.)
        // and they deserve a clear message plus the attempted path so they
        // can diagnose it themselves.
        eprintln!("nog: could not load tier-pins: {}", e);
        eprintln!("     (tried: {})", cfg.paths.tier_pins);
        end(1);
    });

    // v1.0.4: attach the pkgbase coupling index so classify() can resolve
    // split-PKGBUILD siblings to the highest tier present in their group.
    // Without this, e.g., `libpipewire` would default to Tier 3 even though
    // its sibling `pipewire` is Tier 2 — breaking Arch's lockstep contract
    // and surfacing the 2026-05-25 pacman dep-resolution failure.
    //
    // Walks the sync DB on first call (OnceLock-cached in sync_db.rs); same
    // data underlies load_build_dates so `nog update` only walks once total.
    // For commands that don't already touch the DB (install, search, pin,
    // unlock), this adds a one-time ~hundreds-of-ms cost per nog invocation
    // — accepted for the correctness gain.
    let pkgbase_index = crate::tiers::PkgbaseIndex::from_packages(crate::sync_db::load_packages());
    tm.with_pkgbase_index(pkgbase_index)
}

fn load_config() -> NogConfig {
    NogConfig::load_default()
}
