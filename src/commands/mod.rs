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
        eprintln!("nog: install exited with status {}", status);
        std::process::exit(status.code().unwrap_or(1));
    }
}

pub fn remove(packages: &[String]) {
    let status = pacman::remove(packages);
    if !status.success() {
        eprintln!("nog: pacman exited with status {}", status);
        std::process::exit(status.code().unwrap_or(1));
    }
}

pub fn update() {
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

    // Phase 4: fold AUR pending upgrades into the same list when a helper is
    // configured. They don't appear in any sync DB, so the hold evaluator will
    // bucket them as Unknown — the per-package prompt handles them cleanly.
    // Future: AUR RPC lookup can feed real build dates in here.
    if let Some(h) = helper {
        match aur::pending_updates(h) {
            Ok(aur_list) => {
                if !aur_list.is_empty() {
                    println!("nog: {} AUR update(s) reported by {}.", aur_list.len(), h);
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

    // Load build dates once; expensive enough to avoid reloading per package.
    let build_dates = sync_db::load_build_dates();
    let now = std::time::SystemTime::now();

    // Evaluate every pending update and bucket it.
    let mut ready: Vec<(PendingUpdate, Tier, u64)> = Vec::new();      // (upd, tier, days_past_window)
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
                ready.push((upd.clone(), tier, days_past_window));
            }
            HoldStatus::Holding { days_remaining } => {
                held.push((upd.clone(), tier, days_remaining, false));
            }
            HoldStatus::Unknown => {
                unknown.push((upd.clone(), tier));
            }
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
        eprintln!("nog: upgrade exited with status {}", status);
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
    ready: &[(PendingUpdate, Tier, u64)],
    held: &[(PendingUpdate, Tier, u64, bool)],
    unknown: &[(PendingUpdate, Tier)],
) {
    // Convention: each section opens with a leading blank line and never trails
    // one. Upstream sections (outer `update()` logic) follow the same rule so
    // spacing is uniform regardless of which buckets are populated.
    if !ready.is_empty() {
        println!();
        println!("{}Ready to install ({}):{}", C_BOLD, ready.len(), C_RESET);
        for (upd, tier, past) in ready {
            let color = tier_color(tier);
            let past_str = if *past == 0 {
                "hold just expired".to_string()
            } else if *past == 1 {
                "1 day past window".to_string()
            } else {
                format!("{} days past window", past)
            };
            println!(
                "  {}{}{} {}{} -> {}{}  {}[{} · {}]{}",
                color, upd.name, C_RESET,
                C_SUBTEXT, upd.old_version, upd.new_version, C_RESET,
                C_SUBTEXT, tier, past_str, C_RESET,
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
            let tier_tag = match tier {
                Tier::One   => format!(" \x1b[31m[Tier 1 — manual sign-off]\x1b[0m"),
                Tier::Two   => format!(" \x1b[33m[Tier 2 — {}d hold]\x1b[0m",
                                    load_config().holds.tier2_days),
                Tier::Three => format!(" \x1b[32m[Tier 3 — fast-track]\x1b[0m"),
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
    // `unlock` only makes sense for Tier 1 packages held by `manual_signoff = true`.
    // With the default `manual_signoff = false`, Tier 1 auto-updates after 30
    // days and this command is a no-op for most users. Its one real action is
    // `--promote`: force-upgrade a held Tier 1 package right now, bypassing the
    // hold regardless of the date window or sign-off policy.
    let tm = load_tiers();
    let tier = tm.classify(package);

    if tier != Tier::One {
        println!("nog: '{}' is {} — no unlock needed (only Tier 1 is ever held by policy).", package, tier);
        return;
    }

    if !promote {
        println!(
            "nog: '{}' is Tier 1. `nog unlock` by itself does nothing — it has no session state to toggle.",
            package,
        );
        println!(
            "     To force-upgrade this package now, bypassing its hold, run:"
        );
        println!("         sudo nog unlock {} --promote", package);
        return;
    }

    let cfg = load_config();
    let helper = resolve_helper(&cfg);
    guard_not_sudo_with_helper(helper);

    println!("nog: promoting '{}' — forcing an upgrade now.", package);
    let pkgs = vec![package.to_string()];
    let status = match helper {
        Some(h) => aur::install(h, &pkgs),
        None    => pacman::install(&pkgs),
    };
    if !status.success() {
        eprintln!("nog: upgrade exited with status {}", status);
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn load_tiers() -> TierManager {
    let cfg = NogConfig::load_default();
    TierManager::load(&cfg.paths.tier_pins).unwrap_or_else(|e| {
        eprintln!("nog warning: could not load tier-pins: {}", e);
        panic!("nog: fatal — could not initialize tier manager");
    })
}

fn load_config() -> NogConfig {
    NogConfig::load_default()
}