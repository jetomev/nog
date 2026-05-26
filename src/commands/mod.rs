use crate::aur::{self, Helper};
use crate::tiers::{Tier, TierManager};
use crate::config::NogConfig;
use crate::holds::{self, HoldStatus};
use crate::pacman::{self, CheckUpdatesError, PendingUpdate};
use crate::sync_db;

// Catppuccin Mocha palette — true-color ANSI. Centralized so every tier-colored
// surface (currently `nog update`; eventually `nog search`) stays consistent.
const C_RED: &str     = "\x1b[38;2;243;139;168m"; // #F38BA8 — Tier 1
const C_YELLOW: &str  = "\x1b[38;2;249;226;175m"; // #F9E2AF — Tier 2
const C_GREEN: &str   = "\x1b[38;2;166;227;161m"; // #A6E3A1 — Tier 3
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
    match aur::detect_helper(&cfg.aur.helper) {
        Ok(opt) => opt,
        Err(e) => {
            eprintln!("nog: {}", e);
            std::process::exit(1);
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
    std::process::exit(1);
}

pub fn install(packages: &[String]) {
    // Explicit user action — never gate or block. Just report tier classification
    // for transparency, then hand off. Tier protection lives in the passive
    // `nog update` path, not at install time.
    let cfg = load_config();
    let helper = resolve_helper(&cfg);
    guard_not_sudo_with_helper(helper);

    let tm = load_tiers();
    for pkg in packages {
        let tier = tm.classify(pkg);
        match tier {
            Tier::One => println!(
                "nog: '{}' is {} — critical system package, will be protected by 30-day hold on future updates.",
                pkg, tier
            ),
            Tier::Two => println!(
                "nog: '{}' is {} — 15-day hold applies to future updates.",
                pkg, tier
            ),
            Tier::Three => println!("nog: '{}' is {} — proceeding.", pkg, tier),
        }
    }

    // When a helper is configured we always route through it — the helper
    // checks sync repos before AUR, so official packages still install via
    // pacman under the hood. This keeps the code simple and avoids a brittle
    // "is this package in a sync DB?" pre-check that would have to stay in
    // sync with pacman's own resolution order.
    let status = match helper {
        Some(h) => aur::install(h, packages),
        None    => pacman::install(packages),
    };
    if !status.success() {
        eprintln!("nog: install exited with status {}", status.code().unwrap_or(-1));
        std::process::exit(status.code().unwrap_or(1));
    }
}

pub fn remove(packages: &[String]) {
    let status = pacman::remove(packages);
    if !status.success() {
        eprintln!("nog: pacman exited with status {}", status.code().unwrap_or(-1));
        std::process::exit(status.code().unwrap_or(1));
    }
}

/// Why this package landed in the Ready bucket. Distinguishes the normal
/// "hold window passed" case from the `--realign` override that pulled a held
/// kernel into Ready to recover from a kernel/headers version mismatch.
#[derive(Clone)]
enum ReadyReason {
    Expired { days_past_window: u64 },
    Realigned,
}

pub fn update(realign: bool) {
    let cfg = load_config();
    let helper = resolve_helper(&cfg);
    guard_not_sudo_with_helper(helper);
    let tm = load_tiers();

    println!("nog: checking for pending updates...");
    let mut pending = match pacman::checkupdates_capture() {
        Ok(list) => list,
        Err(CheckUpdatesError::Missing) => {
            eprintln!("nog: `checkupdates` not found. Please install `pacman-contrib`:");
            eprintln!("       sudo pacman -S pacman-contrib");
            std::process::exit(1);
        }
        Err(CheckUpdatesError::Other(msg)) => {
            eprintln!("nog: checkupdates failed: {}", msg);
            std::process::exit(1);
        }
    };

    // Fold AUR pending upgrades into the same list when a helper is configured.
    // We track which names came from AUR so we can look up their build dates
    // via the helper's cached metadata below.
    let mut aur_names: Vec<String> = Vec::new();
    if let Some(h) = helper {
        match aur::pending_updates(h) {
            Ok(aur_list) => {
                if !aur_list.is_empty() {
                    println!("nog: {} AUR update(s) reported by {}.", aur_list.len(), h);
                }
                for u in &aur_list {
                    aur_names.push(u.name.clone());
                }
                pending.extend(aur_list);
            }
            Err(e) => {
                eprintln!("nog: warning — could not query AUR updates from {}: {}", h, e);
                eprintln!("     proceeding with official repo updates only.");
            }
        }
    }

    if pending.is_empty() {
        println!("nog: system is up to date — nothing to do.");
        return;
    }

    // Sync-DB build dates first (covers official repos and any binary AUR
    // mirrors like Chaotic-AUR).
    let mut build_dates = sync_db::load_build_dates();

    // Then extend with AUR build dates fetched via the helper's cached metadata
    // (`<helper> -Sai`). Only query for AUR names that weren't already resolved
    // by the sync-DB pass. If the helper is unreachable or the date is
    // unparseable, those packages fall back to the Unknown bucket — the
    // per-package y/N prompt still handles them cleanly.
    if let Some(h) = helper {
        let missing: Vec<String> = aur_names.iter()
            .filter(|name| !build_dates.contains_key(name.as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            let aur_dates = aur::build_dates_for(h, &missing);
            build_dates.extend(aur_dates);
        }
    }

    let now = std::time::SystemTime::now();

    // Evaluate every pending update and bucket it.
    let mut ready: Vec<(PendingUpdate, Tier, ReadyReason)> = Vec::new();
    let mut held: Vec<(PendingUpdate, Tier, u64, bool)> = Vec::new(); // (upd, tier, days_remaining, forced_by_signoff)
    let mut unknown: Vec<(PendingUpdate, Tier)> = Vec::new();

    for upd in &pending {
        let tier = tm.classify(&upd.name);
        let status = holds::evaluate(&upd.name, tier.clone(), &build_dates, &cfg.holds, now);

        // Expert-mode override: `manual_signoff = true` on Tier 1 forces every
        // Tier 1 package into the held bucket regardless of date. Escape hatch
        // is `nog unlock <pkg> --promote`.
        let signoff_hold = tm.is_manual_signoff(&upd.name);

        match status {
            _ if signoff_hold => {
                // Report 0 days remaining as a placeholder; the UI shows the
                // "manual sign-off" reason instead of a countdown.
                held.push((upd.clone(), tier, 0, true));
            }
            HoldStatus::Expired { days_past_window } => {
                ready.push((upd.clone(), tier, ReadyReason::Expired { days_past_window }));
            }
            HoldStatus::Holding { days_remaining } => {
                held.push((upd.clone(), tier, days_remaining, false));
            }
            HoldStatus::Unknown => {
                unknown.push((upd.clone(), tier));
            }
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
            let mut new_held: Vec<(PendingUpdate, Tier, u64, bool)> = Vec::new();
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

    print_buckets(&ready, &held, &unknown);

    // Interactive step: decide what to do with Unknowns. Each gets a y/N prompt.
    // EOF or non-TTY stdin → default all remaining to skip, with a warning.
    let mut extra_ignore: Vec<String> = Vec::new();
    if !unknown.is_empty() {
        println!();
        println!("{}nog: {} package(s) have no build date in any sync DB.{}",
            C_SUBTEXT, unknown.len(), C_RESET);
        println!("{}      This usually means an AUR-only, locally-built, or disabled-repo package.{}",
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
    let mut ignore: Vec<String> = held.iter().map(|(u, _, _, _)| u.name.clone()).collect();
    ignore.extend(extra_ignore);

    if ready.is_empty() && ignore.len() == pending.len() {
        println!();
        println!("nog: nothing to install right now — all pending updates are held.");
        return;
    }

    println!();
    let status = match helper {
        Some(h) => {
            println!("{}nog: handing off to {}...{}", C_BOLD, h, C_RESET);
            aur::upgrade_excluding(h, &ignore)
        }
        None => {
            println!("{}nog: handing off to pacman...{}", C_BOLD, C_RESET);
            pacman::update_excluding(&ignore)
        }
    };
    if !status.success() {
        eprintln!("nog: upgrade exited with status {}", status.code().unwrap_or(-1));
        std::process::exit(status.code().unwrap_or(1));
    }
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

fn print_buckets(
    ready: &[(PendingUpdate, Tier, ReadyReason)],
    held: &[(PendingUpdate, Tier, u64, bool)],
    unknown: &[(PendingUpdate, Tier)],
) {
    // Convention: each section opens with a leading blank line and never trails
    // one. Upstream sections (outer `update()` logic) follow the same rule so
    // spacing is uniform regardless of which buckets are populated.
    if !ready.is_empty() {
        println!();
        println!("{}Ready to install ({}):{}", C_BOLD, ready.len(), C_RESET);
        for (upd, tier, reason) in ready {
            let color = tier_color(tier);
            let reason_str = match reason {
                ReadyReason::Expired { days_past_window: 0 } => "hold just expired".to_string(),
                ReadyReason::Expired { days_past_window: 1 } => "1 day past window".to_string(),
                ReadyReason::Expired { days_past_window } => format!("{} days past window", days_past_window),
                ReadyReason::Realigned => "realigned to match installed headers".to_string(),
            };
            println!(
                "  {}{}{} {}{} -> {}{}  {}[{} · {}]{}",
                color, upd.name, C_RESET,
                C_SUBTEXT, upd.old_version, upd.new_version, C_RESET,
                C_SUBTEXT, tier, reason_str, C_RESET,
            );
        }
    }

    if !held.is_empty() {
        println!();
        println!("{}Held ({}):{}", C_BOLD, held.len(), C_RESET);
        for (upd, tier, remaining, signoff) in held {
            let color = tier_color(tier);
            let reason = if *signoff {
                "manual sign-off required — run `nog unlock` to release".to_string()
            } else if *remaining == 1 {
                "1 day remaining".to_string()
            } else {
                format!("{} days remaining", remaining)
            };
            println!(
                "  {}{}{} {}{} -> {}{}  {}[{} · {}]{}",
                color, upd.name, C_RESET,
                C_SUBTEXT, upd.old_version, upd.new_version, C_RESET,
                C_SUBTEXT, tier, reason, C_RESET,
            );
        }
    }

    if !unknown.is_empty() {
        println!();
        println!("{}Unknown ({}):{}", C_BOLD, unknown.len(), C_RESET);
        for (upd, tier) in unknown {
            let color = tier_color(tier);
            println!(
                "  {}{}{} {}{} -> {}{}  {}[{} · no build date in sync DB]{}",
                color, upd.name, C_RESET,
                C_SUBTEXT, upd.old_version, upd.new_version, C_RESET,
                C_SUBTEXT, tier, C_RESET,
            );
        }
    }
}

pub fn search(query: &str) {
    let cfg = load_config();
    let tm = load_tiers();
    let output = pacman::search_capture(query);

    if output.stdout.is_empty() {
        println!("nog: no results for '{}'", query);
        return;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if !line.starts_with(' ') && !line.starts_with('\t') {
            let pkg_name = line
                .split('/')
                .nth(1)
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("");

            let tier = tm.classify(pkg_name);
            // All three tier labels now read their day count from the holds
            // config, and Tier 1 flips to "manual sign-off" text only when
            // expert mode is enabled. This keeps the search annotation in
            // lockstep with the actual v1.0 behavior — the old hardcoded
            // "manual sign-off" for Tier 1 and bespoke "fast-track" for
            // Tier 3 both misrepresented the default experience.
            let tier_tag = match tier {
                Tier::One => {
                    let body = if tm.is_manual_signoff(pkg_name) {
                        "manual sign-off".to_string()
                    } else {
                        format!("{}d hold", cfg.holds.tier1_days)
                    };
                    format!(" \x1b[31m[Tier 1 — {}]\x1b[0m", body)
                }
                Tier::Two   => format!(" \x1b[33m[Tier 2 — {}d hold]\x1b[0m",
                                    cfg.holds.tier2_days),
                Tier::Three => format!(" \x1b[32m[Tier 3 — {}d hold]\x1b[0m",
                                    cfg.holds.tier3_days),
            };

            println!("{}{}", line, tier_tag);

            if i + 1 < lines.len() && (lines[i+1].starts_with(' ') || lines[i+1].starts_with('\t')) {
                println!("{}", lines[i + 1]);
                i += 2;
                continue;
            }
        }
        i += 1;
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
            std::process::exit(1);
        }
    }
}

pub fn unlock(package: &str, promote: bool) {
    // `unlock --promote` force-upgrades a package immediately, bypassing the
    // hold window regardless of tier.
    //
    // v1.0.4 relaxed the Tier 1 restriction. Pre-v1.0.4 unlock refused any
    // non-Tier-1 package ("no unlock needed (only Tier 1 is ever held by
    // policy)"), but Tier 2 packages CAN be held within their 15-day window —
    // and the 2026-05-25 pipewire split-PKGBUILD incident showed that users
    // need to release Tier 2 holds to break a tier-mismatched lockstep
    // deadlock. The new rule: any held package can be promoted.
    let tm = load_tiers();
    let tier = tm.classify(package);
    let signoff = tm.is_manual_signoff(package);

    if !promote {
        println!("nog: '{}' is {}.", package, tier);
        match tier {
            Tier::One if signoff => {
                println!("     Tier 1 with `manual_signoff = true` — wholesale held until promote.");
            }
            Tier::One => {
                println!("     Tier 1 (30-day hold by default).");
            }
            Tier::Two => {
                println!("     Tier 2 (15-day hold by default).");
            }
            Tier::Three => {
                println!("     Tier 3 (7-day hold by default).");
            }
        }
        println!("     `nog unlock` by itself does nothing — it has no per-session state to toggle.");
        println!("     To force-upgrade this package now, bypassing the hold, run:");
        println!("         nog unlock {} --promote", package);
        return;
    }

    let cfg = load_config();
    let helper = resolve_helper(&cfg);
    guard_not_sudo_with_helper(helper);

    println!("nog: promoting '{}' (currently {}) — forcing an upgrade now.", package, tier);
    let pkgs = vec![package.to_string()];
    let status = match helper {
        Some(h) => aur::install(h, &pkgs),
        None    => pacman::install(&pkgs),
    };
    if !status.success() {
        eprintln!("nog: upgrade exited with status {}", status.code().unwrap_or(-1));
        std::process::exit(status.code().unwrap_or(1));
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
        std::process::exit(1);
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