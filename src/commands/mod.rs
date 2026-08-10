use crate::aur::{self, Helper};
use crate::tiers::{Tier, TierManager};
use crate::config::NogConfig;
use crate::holds::{self, HoldStatus};
use crate::pacman::{self, CheckUpdatesError, PendingUpdate};
use crate::runlog;
use crate::flatpak;
use crate::snap;
use crate::sources;
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

pub fn deactivate(source: &str) {
    set_source(source, false);
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
            std::process::exit(1);
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
        std::process::exit(1);
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
        std::process::exit(1);
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
        std::process::exit(1);
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
            std::process::exit(1);
        }
    };

    match sources::toggle_repo_section(&text, "chaotic-aur", enable) {
        sources::RepoToggle::NotFound => {
            eprintln!("nog: no [chaotic-aur] section found in {} (neither active nor nog-disabled).", conf_path);
            if enable {
                eprintln!("     Nothing to restore — if you want chaotic-aur, add the repo per its docs first.");
            }
            std::process::exit(1);
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
            let cp = std::process::Command::new("sudo")
                .args(["cp", "--preserve=all", conf_path, &backup])
                .status();
            match cp {
                Ok(s) if s.success() => {}
                _ => {
                    eprintln!("nog: could not back up {} — aborting without changes.", conf_path);
                    std::process::exit(1);
                }
            }

            // 2. Write the toggled pacman.conf.
            if let Err(e) = crate::tiers::write_as_root(conf_path, &new_text) {
                eprintln!("nog: could not write {}: {}", conf_path, e);
                eprintln!("     your original is safe at {}", backup);
                std::process::exit(1);
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
}

/// Why this package landed in the Held bucket. Drives the reason string shown in
/// the Held listing.
#[derive(Clone)]
enum HeldReason {
    /// Normal hold — the tier's window is still open (`days_remaining` left).
    Window,
    /// Expert-mode `manual_signoff = true` on a Tier 1 package. Released with
    /// `nog unlock`.
    ManualSignoff,
    /// v1.0.6 (issue #1): held only because coupling this `lib32-<X>`/base `<X>`
    /// pair keeps a version-locked multilib package from splitting across
    /// buckets. Carries the partner it is waiting on. Its own window may already
    /// have expired; the countdown shown is the partner's.
    CoupledTo(String),
}

pub fn update(realign: bool) {
    let cfg = load_config();
    let helper = resolve_helper(&cfg);
    guard_not_sudo_with_helper(helper);
    let tm = load_tiers();

    let (run_date, run_time, run_user) = print_update_header();
    println!("nog: Checking for pending updates ...");
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
    let official_count = pending.len();

    // Fold AUR pending upgrades into the same list when a helper is configured.
    // We track which names came from AUR so we can look up their build dates
    // via the helper's cached metadata below.
    let mut aur_names: Vec<String> = Vec::new();
    let mut aur_count = 0usize;
    if let Some(h) = helper {
        match aur::pending_updates(h) {
            Ok(aur_list) => {
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

    // Per-source counts.
    println!();
    println!("nog: {} official repository update(s) reported by pacman.", official_count);
    if let Some(h) = helper {
        println!("nog: {} AUR update(s) reported by {}.", aur_count, h);
    }
    if flatpak_present {
        println!("nog: {} flatpak update(s) reported by flatpak.", flatpak_count);
    }
    if snap_present {
        println!("nog: {} snap update(s) reported by snapd.", snap_count);
    }

    if pending.is_empty() {
        println!();
        println!("nog: System is up to date — nothing to do.");
        write_run_log(&cfg, &run_date, &run_time, &run_user, Vec::new(), "up to date");
        return;
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
        });
    }

    // Snap publish dates → the same builddate map.
    for (name, ts) in &snap_dates {
        packages.entry(name.clone()).or_insert(sync_db::PackageDesc {
            builddate: *ts,
            pkgbase: None,
            version: None,
        });
    }

    let now = std::time::SystemTime::now();

    // Evaluate every pending update and bucket it.
    let mut ready: Vec<(PendingUpdate, Tier, ReadyReason)> = Vec::new();
    let mut held: Vec<(PendingUpdate, Tier, u64, HeldReason)> = Vec::new(); // (upd, tier, days_remaining, reason)
    let mut unknown: Vec<(PendingUpdate, Tier)> = Vec::new();

    for upd in &pending {
        let tier = tm.classify(&upd.name);
        let status = holds::evaluate_candidate(
            &upd.name,
            tier.clone(),
            &upd.new_version,
            &packages,
            &cfg.holds,
            now,
        );

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
            HoldStatus::Expired { days_past_window } => {
                ready.push((upd.clone(), tier, ReadyReason::Expired { days_past_window }));
            }
            HoldStatus::Holding { days_remaining } => {
                held.push((upd.clone(), tier, days_remaining, HeldReason::Window));
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

    // v1.0.6 (issue #1): couple split lib32/base pairs. A lib32-<X> and its base
    // <X> are version-locked, but their hold windows are dated independently, so
    // one can land in Ready while the other is Held. Releasing half the pair
    // makes pacman's exact-version dependency unsatisfiable and aborts the whole
    // transaction. Demote the Ready member of any split pair into Held (inheriting
    // the held partner's countdown) so the pair releases together. Runs last, so
    // it sees the post-realign buckets and feeds the ignore list below.
    {
        let ready_names: Vec<String> = ready.iter().map(|(u, _, _)| u.name.clone()).collect();
        let held_names: Vec<String> = held.iter().map(|(u, _, _, _)| u.name.clone()).collect();
        let demotions = holds::lib32_coupling_demotions(&ready_names, &held_names);
        if !demotions.is_empty() {
            let mut kept: Vec<(PendingUpdate, Tier, ReadyReason)> = Vec::new();
            for entry in ready.drain(..) {
                let (upd, tier, _) = &entry;
                match demotions.iter().find(|(name, _)| name == &upd.name) {
                    Some((_, partner)) => {
                        // Inherit the partner's remaining days so both rows show
                        // the same countdown and clear together.
                        let remaining = held.iter()
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
        }
    }

    // v1.0.9 (A4, issue #6): Held reads soonest-to-release first. Ties break
    // alphabetically so the order is stable run-to-run. ManualSignoff rows
    // carry the placeholder 0 and surface at the top — they need the user's
    // attention anyway. The CSV snapshot below mirrors this order.
    held.sort_by(|(a, _, ar, _), (b, _, br, _)| ar.cmp(br).then_with(|| a.name.cmp(&b.name)));

    print_buckets(&ready, &held, &unknown, &flatpak_names, &snap_names);

    // v1.0.8: snapshot the final buckets for the run log. Taken after the
    // realign/coupling passes so the CSV mirrors the printed tables exactly.
    let log_rows = runlog_rows(&ready, &held, &unknown);

    // Interactive step: decide what to do with Unknowns. Each gets a y/N prompt.
    // EOF or non-TTY stdin → default all remaining to skip, with a warning.
    let mut extra_ignore: Vec<String> = Vec::new();
    if !unknown.is_empty() {
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
    let mut ignore: Vec<String> = held.iter().map(|(u, _, _, _)| u.name.clone()).collect();
    ignore.extend(extra_ignore);

    if ready.is_empty() && ignore.len() == pending.len() {
        println!();
        println!("nog: Nothing to install — every pending update is held.");
        write_run_log(&cfg, &run_date, &run_time, &run_user, log_rows, "all held");
        return;
    }

    // v1.0.9 (Ironhold): the foreign fence. The helper's -Syu resolves AUR
    // updates on its own at handoff time, so it can upgrade a held AUR package
    // whenever our earlier query failed to name it — the 2026-08-01 bypass.
    // Fence rule: every foreign package is ignored unless nog cleared it THIS
    // run (Ready, or an Unknown the user answered yes to). See holds::foreign_fence.
    if helper.is_some() {
        let mut cleared: Vec<String> = ready.iter().map(|(u, _, _)| u.name.clone()).collect();
        cleared.extend(
            unknown.iter()
                .map(|(u, _)| u.name.clone())
                .filter(|n| !ignore.contains(n)),
        );
        let fence = holds::foreign_fence(&pacman::foreign_package_names(), &cleared, &ignore);
        if !fence.is_empty() {
            println!();
            println!(
                "{}nog: foreign fence — {} AUR/local package(s) shielded from the handoff{}",
                C_SUBTEXT, fence.len(), C_RESET
            );
            println!(
                "{}     (only AUR updates cleared by nog can install, even if the AUR query failed).{}",
                C_SUBTEXT, C_RESET
            );
            ignore.extend(fence);
        }
    }

    // First review gate. yay/pacman will present its own transaction detail and
    // ask again — two deliberate layers so an expert can still catch and cancel.
    println!();
    if !prompt_proceed() {
        println!("nog: Cancelled — nothing was installed.");
        write_run_log(&cfg, &run_date, &run_time, &run_user, log_rows, "cancelled");
        return;
    }

    println!();
    let status = match helper {
        Some(h) => {
            println!("{}nog: Handing off to {} ...{}", C_BOLD, h, C_RESET);
            aur::upgrade_excluding(h, &ignore)
        }
        None => {
            println!("{}nog: Handing off to pacman ...{}", C_BOLD, C_RESET);
            pacman::update_excluding(&ignore)
        }
    };
    if !status.success() {
        let code = status.code().unwrap_or(-1);
        eprintln!("nog: upgrade exited with status {}", code);
        write_run_log(&cfg, &run_date, &run_time, &run_user, log_rows,
            &format!("handoff failed (status {})", code));
        std::process::exit(status.code().unwrap_or(1));
    }

    // v1.1.0 (C1): flatpak apply — only the refs nog cleared THIS run
    // (Ready, or an Unknown the user approved). flatpak has no --ignore, so
    // listing exactly the cleared app IDs IS the hold mechanism: held
    // flatpaks are simply never named.
    if !flatpak_names.is_empty() {
        let ready_names: Vec<String> = ready.iter().map(|(u, _, _)| u.name.clone()).collect();
        let unknown_names: Vec<String> = unknown.iter().map(|(u, _)| u.name.clone()).collect();
        let fp_apply = flatpak::apply_list(&flatpak_names, &ready_names, &unknown_names, &ignore);
        if !fp_apply.is_empty() {
            println!();
            println!("{}nog: Handing off {} app(s) to flatpak ...{}", C_BOLD, fp_apply.len(), C_RESET);
            println!("{}     (flatpak shows its own transaction below){}", C_SUBTEXT, C_RESET);
            let fp_status = flatpak::update(&fp_apply);
            if !fp_status.success() {
                let code = fp_status.code().unwrap_or(-1);
                eprintln!("nog: flatpak update exited with status {}", code);
                write_run_log(&cfg, &run_date, &run_time, &run_user, log_rows,
                    &format!("flatpak apply failed (status {})", code));
                std::process::exit(fp_status.code().unwrap_or(1));
            }
        }
    }

    // v1.2.0 (C2): snap apply — same naming rule as flatpak. `snap refresh`
    // needs root, so nog escalates through sudo for this step only.
    if !snap_names.is_empty() {
        let ready_names: Vec<String> = ready.iter().map(|(u, _, _)| u.name.clone()).collect();
        let unknown_names: Vec<String> = unknown.iter().map(|(u, _)| u.name.clone()).collect();
        let sn_apply = snap::apply_list(&snap_names, &ready_names, &unknown_names, &ignore);
        if !sn_apply.is_empty() {
            println!();
            println!("{}nog: Handing off {} snap(s) to snapd ...{}", C_BOLD, sn_apply.len(), C_RESET);
            println!("{}     (snap refresh needs root — sudo may prompt; snap shows its own progress){}",
                C_SUBTEXT, C_RESET);
            let sn_status = snap::refresh(&sn_apply);
            if !sn_status.success() {
                let code = sn_status.code().unwrap_or(-1);
                eprintln!("nog: snap refresh exited with status {}", code);
                write_run_log(&cfg, &run_date, &run_time, &run_user, log_rows,
                    &format!("snap apply failed (status {})", code));
                std::process::exit(sn_status.code().unwrap_or(1));
            }
        }
    }

    println!();
    println!("nog: Update finished!");
    write_run_log(&cfg, &run_date, &run_time, &run_user, log_rows, "installed");
    println!();
    println!("Thank you for using nog!");
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
fn print_update_header() -> (String, String, String) {
    let (date, time) = now_date_time();
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "unknown".to_string());
    println!("{}nog - Update!{}", C_BOLD, C_RESET);
    println!("=============");
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
    print!("nog: Proceed with installation? [Y/n] ");
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
    old: String,
    new: String,
    tier: u8,
    note: String,
}

impl TableRow {
    fn from(upd: &PendingUpdate, tier: &Tier, note: String) -> TableRow {
        TableRow {
            pkg: upd.name.clone(),
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
    let mut out = format!("{}\n{}\n\n", title_line, "-".repeat(title_line.len()));

    if rows.is_empty() {
        out.push_str("(none)\n");
        return out;
    }

    let pkg_hdr = format!("Package ({})", rows.len());
    let w_pkg = std::iter::once(pkg_hdr.len())
        .chain(rows.iter().map(|r| r.pkg.len()))
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
    let g = "  ";

    out.push_str(&format!(
        "{:<wp$}{g}{:<wo$}{g}{:<wn$}{g}{:<wt$}{g}{}\n",
        pkg_hdr, "Old Version", "New Version", "Tier", "Note",
        wp = w_pkg, wo = w_old, wn = w_new, wt = w_tier, g = g,
    ));
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
        out.push_str(&format!(
            "{:<wp$}{g}{:<wo$}{g}{:<wn$}{g}{}{g}{}\n",
            r.pkg, r.old, r.new, tier_cell, r.note,
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
    }
}

/// Map a Held bucket entry to its `Note` text.
fn held_note(remaining: u64, reason: &HeldReason) -> String {
    match reason {
        HeldReason::ManualSignoff =>
            "manual sign-off required — run `nog unlock` to release".to_string(),
        HeldReason::CoupledTo(partner) => match remaining {
            1 => format!("coupled to {} · 1 day", partner),
            n => format!("coupled to {} · {} days", partner, n),
        },
        HeldReason::Window => match remaining {
            1 => "1 day remaining".to_string(),
            n => format!("{} days remaining", n),
        },
    }
}

fn print_buckets(
    ready: &[(PendingUpdate, Tier, ReadyReason)],
    held: &[(PendingUpdate, Tier, u64, HeldReason)],
    unknown: &[(PendingUpdate, Tier)],
    flatpak_names: &[String],
    snap_names: &[String],
) {
    // Ruling #2 of the v2 arc: the user always sees WHERE a package comes
    // from. Non-pacman rows carry their source in the Note column.
    let tag = |name: &str, note: String| -> String {
        let src = if flatpak_names.iter().any(|n| n == name) {
            Some("flatpak")
        } else if snap_names.iter().any(|n| n == name) {
            Some("snap")
        } else {
            None
        };
        match src {
            None => note,
            Some(s) if note.is_empty() => s.to_string(),
            Some(s) => format!("{} · {}", note, s),
        }
    };
    let ready_rows: Vec<TableRow> = ready.iter()
        .map(|(upd, tier, reason)| TableRow::from(upd, tier, tag(&upd.name, ready_note(reason))))
        .collect();
    let held_rows: Vec<TableRow> = held.iter()
        .map(|(upd, tier, remaining, reason)| TableRow::from(upd, tier, tag(&upd.name, held_note(*remaining, reason))))
        .collect();
    let unknown_rows: Vec<TableRow> = unknown.iter()
        .map(|(upd, tier)| TableRow::from(upd, tier, tag(&upd.name, "no build date in sync DB".to_string())))
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
        bucket: bucket.to_string(),
        package: upd.name.clone(),
        old_version: upd.old_version.clone(),
        new_version: upd.new_version.clone(),
        tier: tier_num(tier).to_string(),
        note,
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

/// Write the run record and prune expired logs (v1.0.8). Every failure path
/// is a warning, never an abort — the update itself has already succeeded or
/// failed on its own terms by the time this runs.
fn write_run_log(
    cfg: &NogConfig,
    date: &str,
    time: &str,
    user: &str,
    rows: Vec<runlog::RunRow>,
    outcome: &str,
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
        outcome: outcome.to_string(),
    };
    match runlog::append_run(&cfg.paths.run_logs, &today, &record) {
        Ok(path) => {
            println!("{}nog: run logged to {}{}", C_SUBTEXT, path.display(), C_RESET);
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

    #[test]
    fn table_aligns_and_counts() {
        let rows = vec![
            TableRow { pkg: "libnm".into(), old: "1.56.1-1".into(), new: "1.56.1-2".into(), tier: 2, note: "9 days past window".into() },
            TableRow { pkg: "wine-staging".into(), old: "11.12-1".into(), new: "11.13-1".into(), tier: 3, note: "hold just expired".into() },
        ];
        let t = format_table("READY TO INSTALL", &rows, false);
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(lines[0], "READY TO INSTALL:");
        assert_eq!(lines[1], "-".repeat("READY TO INSTALL:".len()));
        assert_eq!(lines[2], "");
        let hdr = lines[3];
        assert!(hdr.starts_with("Package (2)"));
        for label in ["Old Version", "New Version", "Tier", "Note"] {
            assert!(hdr.contains(label), "header missing {label}");
        }
        assert_eq!(lines[4], "");
        // Alignment guarantee: every column's value begins exactly under its header.
        let (r0, r1) = (lines[5], lines[6]);
        assert!(r0.starts_with("libnm"));
        assert!(r1.starts_with("wine-staging"));
        for (label, v0, v1) in [
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

    #[test]
    fn empty_table_renders_none() {
        let t = format_table("UNKNOWN", &[], false);
        assert!(t.starts_with("UNKNOWN:\n"));
        assert!(t.contains("\n\n(none)\n"));
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