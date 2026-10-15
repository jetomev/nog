mod snap;
mod flatpak;
mod handoff;
mod aur;
mod cache;
mod commands;
mod events;
mod config;
mod holds;
mod locks;
mod maint;
mod history;
mod elf;
mod local_db;
mod machine;
mod pacman;
mod reboot;
mod record;
mod removal;
mod runlog;
mod sightings;
mod sources;
mod sync_db;
mod tiers;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "nog",
    about = "Tier-aware package manager for Arch Linux",
    version = env!("CARGO_PKG_VERSION"),
    long_about = "\
nog wraps pacman (and optionally yay or paru) with a three-tier update \
management system. Every package is classified into Tier 1 (kernel, bootloader, \
glibc, systemd — 30-day hold), Tier 2 (desktop environment and key apps — \
15-day hold), or Tier 3 (everything else — 7-day hold). `nog update` computes \
a plan showing Ready / Held / Unknown buckets before any transaction runs.\n\
\n\
Run `nog` as your regular user; it prompts for sudo internally only when root \
is actually needed. See `man nog` for the full reference."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install packages: the repositories, then the AUR, then Flatpak, then Snap
    ///
    /// Shows where each package comes from (and its tier) before anything is
    /// downloaded or built, and says who takes over. `aur/x`, `extra/x`,
    /// `flatpak/x` and `snap/x` name that source only. A package file
    /// (`*.pkg.tar.*`) is installed as it is. Tier protection applies to
    /// future updates, not to installs — explicit user commands execute
    /// user intent.
    Install {
        #[arg(required = true)]
        packages: Vec<String>,
        /// Answer yes to the install's question for you — repository packages, package
        /// files, Flatpak and Snap. Never the AUR: its build review needs a person (v1.9.0)
        #[arg(long)]
        yes: bool,
    },
    /// Remove packages, Flatpak apps and snaps; says what else goes and who still uses it
    ///
    /// Before pacman removes anything, nog lists each package that would go
    /// along but another installed app still uses optionally, and offers to
    /// keep it (F-13). A Flatpak or a snap nog installed is removed by its own
    /// tool.
    Remove {
        #[arg(required = true)]
        packages: Vec<String>,
        /// Answer yes to the removal's question for you; what another app still uses is kept (v1.9.0)
        #[arg(long)]
        yes: bool,
    },
    /// Mark packages as installed on purpose, so removing something else never takes them along
    ///
    /// pacman's own "install reason" (`pacman -D --asexplicit`), without
    /// leaving nog for it. `nog remove` offers the same for each package it
    /// would take along that another app still uses (v1.9.0, F-13).
    Keep {
        #[arg(required = true)]
        packages: Vec<String>,
    },
    /// Compute a tier-aware upgrade plan and hand off to pacman or the helper
    ///
    /// Lists pending upgrades via `checkupdates` (and `<helper> -Qua` if
    /// configured), evaluates each against its tier's hold window using
    /// sync-DB build dates (plus `<helper> -Sai` for AUR packages), and
    /// groups them into Ready / Held / Unknown. Unknown packages prompt
    /// per-package. The final transaction runs `pacman -Syu --ignore=…`
    /// or `<helper> -Syu --ignore=…`.
    ///
    /// `--realign` recovers from a kernel/headers version mismatch by pulling
    /// any held kernel whose pending upgrade matches the installed headers
    /// version out of the Held bucket and into the upgrade transaction. Useful
    /// after a partial upgrade left DKMS in a broken state.
    Update {
        /// Update only these (v1.6.1): nothing else moves, and only they are shown. A held one
        /// must be promoted first (`--promote`), or nog stops without changing anything
        packages: Vec<String>,
        #[arg(long, help = "Pull held kernels into the upgrade to match installed headers")]
        realign: bool,
        /// Write the plan as JSON and stop before any question or change (v1.6.0)
        #[arg(long)]
        json: bool,
        /// Keep these back this time; what must stay with them follows (comma-separated)
        #[arg(long, value_delimiter = ',')]
        keep: Vec<String>,
        /// Promote these: ready now, installed with the rest; partners they need come too
        #[arg(long, value_delimiter = ',')]
        promote: Vec<String>,
    },
    /// Search every source: the repositories, the AUR, Flatpak and Snap
    ///
    /// v2.0.0 (C4): results per source, each with its tier; a Flatpak row says
    /// how to install exactly it (`flatpak/<app-id>`), a snap row `snap/<name>`.
    /// A source that is off or not installed is skipped; one that cannot be
    /// reached is said. `--json` for programs (nogForge's Install page).
    Search {
        query: String,
        #[arg(long)]
        json: bool,
    },
    /// List installed packages, Flatpak apps and snaps with their tier and source
    ///
    /// `--json` adds what needs each one and whether it is protected from
    /// removal (Tier 1, the base set, or needed by another package) — what
    /// nogForge's Home shows (v1.6.0). Flatpak apps and snaps since v2.0.0.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Everything nog knows about one package: source, tier and why, hold, details
    ///
    /// v2.0.0 (C4). Installed first (pacman's database, an installed Flatpak or
    /// snap), then the repositories, the AUR, Flatpak, Snap. `--json` for programs.
    Info {
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// Put back the files of installed packages, each through its own tool
    ///
    /// v2.0.0 (C4): pacman reinstalls the same version, the AUR helper rebuilds,
    /// `flatpak install --reinstall` for a Flatpak app. A snap cannot be
    /// reinstalled in place (snapd has no such step), and a name that is not
    /// installed stops before anything.
    Reinstall {
        #[arg(required = true)]
        packages: Vec<String>,
        /// Answer the tools' questions for you (never the AUR's build review)
        #[arg(long)]
        yes: bool,
    },
    /// A package's tier and where it comes from; --set moves it (what `nog pin` does)
    Tier {
        package: String,
        /// Move the package to this tier (1, 2 or 3); written to tier-pins.toml
        #[arg(long)]
        set: Option<u8>,
        #[arg(long)]
        json: bool,
    },
    /// Lock packages at their installed version: never updated until unlocked
    ///
    /// v2.0.0 (C4). `nog update` lists a locked package as held, "locked by
    /// you", and pacman is told to ignore it. Alone, lists the locks.
    /// Written to /etc/nog/locks.toml, so every user and nogForge see them.
    Lock {
        packages: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// The dashboard: what is installed from where, what is ready or held, locks, last runs
    Status {
        #[arg(long)]
        json: bool,
    },
    /// What nog has done: every run, every update decision, every reboot advice
    ///
    /// v2.0.0 (C4). Read from nog's own logs (~/.local/share/nog/logs, kept 90
    /// days), newest first. `--days N` (default 30).
    History {
        #[arg(long, default_value = "30")]
        days: u32,
        #[arg(long)]
        json: bool,
    },
    /// Pin a package to a specific tier (persists to /etc/nog/tier-pins.toml)
    ///
    /// Writes via `sudo tee` — no shell-level sudo required. Pinning to
    /// Tier 3 removes any existing Tier 1 or Tier 2 entry (Tier 3 is the
    /// implicit default).
    Pin {
        package: String,
        #[arg(long, default_value = "1")]
        tier: u8,
    },
    /// Remove a lock; with --promote, also upgrade the package now, past its hold
    ///
    /// v2.0.0: `nog unlock <pkg>` undoes `nog lock`. `--promote` keeps its
    /// v1.0.4 meaning: bypass the hold window and any sign-off policy and
    /// upgrade the package immediately via pacman or the configured helper.
    Unlock {
        #[arg(required = true)]
        packages: Vec<String>,
        #[arg(long)]
        promote: bool,
    },
    /// Deactivate a package source (kill switch; persists to /etc/nog/sources.toml)
    ///
    /// `nog deactivate aur` — every AUR-aware path refuses until reactivated:
    /// update detection skips the AUR, installs route through pacman only, and
    /// the upgrade handoff runs via pacman (foreign packages stay fenced).
    /// Intended for incident response — e.g. an active AUR supply-chain attack.
    Deactivate {
        /// Source to deactivate: "aur", "chaotic-aur", "flatpak", or "snap"
        source: String,
    },
    /// Clean up what every source leaves behind: old downloads, orphans, build trees, unused runtimes, old snap revisions
    ///
    /// v2.0.0 (C5). One report, each category picked on its own, each removed
    /// by the tool that owns it (pacman asks, flatpak asks, nog says). pacman's
    /// cache keeps 3/2/1 versions per tier (v1.5.5; `[clean]` in nog.conf) and
    /// never the installed one. `--json` is the report alone; `--only a,b`
    /// picks categories (cache, orphans, aur, flatpak, snap); `--yes` answers
    /// the questions.
    Clean {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
    },
    /// Reactivate a package source previously disabled with `nog deactivate`
    Activate {
        /// Source to activate: "aur", "chaotic-aur", "flatpak", or "snap"
        source: String,
    },
    /// Internal: dump the build date for a package from the sync DB
    #[command(name = "_debug-dates", hide = true)]
    DebugDates {
        package: String,
    },
    /// Internal: evaluate the hold status for a package
    #[command(name = "_debug-hold", hide = true)]
    DebugHold {
        package: String,
    },
}

fn main() {
    let cli = Cli::parse();
    // v1.5.8: every user-facing run starts with nog's version and the run
    // requested, and ends with where it was logged and a thank-you (Javier,
    // 2 Oct 2026). The hidden _debug commands are internal and stay bare.
    let internal = matches!(cli.command, Commands::DebugDates { .. } | Commands::DebugHold { .. });
    let json = matches!(&cli.command,
        Commands::Update { json: true, .. } | Commands::Search { json: true, .. } | Commands::List { json: true }
        | Commands::Info { json: true, .. } | Commands::Tier { json: true, .. } | Commands::Lock { json: true, .. }
        | Commands::Status { json: true } | Commands::History { json: true, .. } | Commands::Clean { json: true, .. });
    if json {
        machine::begin();
    }
    // v1.6.0: a JSON run is a program asking, not a person's run: no banner,
    // no closing note, no line in the run history.
    let internal = internal || json;
    if !internal {
        // v1.6.0: a person's run is kept whole, as on screen (record.rs)
        record::rerun_recorded(std::path::Path::new(&runlog::expand_home(&config::NogConfig::load_default().paths.run_logs)));
    }
    if !internal {
        let label: Vec<String> = std::env::args().skip(1).collect();
        commands::begin(&label.join(" "));
    }
    match cli.command {
        Commands::Install { packages, yes } => commands::install(&packages, yes),
        Commands::Remove { packages, yes } => commands::remove(&packages, yes),
        Commands::Keep { packages } => commands::keep(&packages),
        Commands::Update { packages, realign, json, keep, promote } => {
            machine::set_only(packages);
            machine::set_promote(promote);
            if json {
                machine::set_plan(keep);
            } else {
                machine::set_keep(keep);
            }
            commands::update(realign)
        }
        Commands::Search { query, json } => commands::search(&query, json),
        Commands::List { json } => commands::list(json),
        Commands::Pin { package, tier } => commands::pin(&package, tier),
        Commands::Unlock { packages, promote } => commands::unlock(&packages, promote),
        Commands::Info { name, json } => commands::info(&name, json),
        Commands::Reinstall { packages, yes } => commands::reinstall(&packages, yes),
        Commands::Tier { package, set, json } => commands::tier(&package, set, json),
        Commands::Lock { packages, json } => commands::lock(&packages, json),
        Commands::Status { json } => commands::status(json),
        Commands::History { days, json } => commands::history(days, json),
        Commands::Deactivate { source } => commands::deactivate(&source),
        Commands::Activate { source } => commands::activate(&source),
        Commands::Clean { json, yes, only } => commands::clean(json, yes, &only),
        Commands::DebugDates { package } => debug_dates(&package),
        Commands::DebugHold { package } => debug_hold(&package),
    }
    if !internal {
        commands::end(0);
    }
}

fn debug_dates(package: &str) {
    let dates = sync_db::load_build_dates();
    match dates.get(package) {
        Some(ts) => {
            println!("package:    {}", package);
            println!("build_date: {} (Unix timestamp)", ts);
            let readable = std::process::Command::new("date")
                .arg("-d")
                .arg(format!("@{}", ts))
                .output();
            match readable {
                Ok(out) if out.status.success() => {
                    print!("readable:   {}", String::from_utf8_lossy(&out.stdout));
                }
                _ => {
                    println!("readable:   (could not invoke `date`)");
                }
            }
            println!("total packages indexed: {}", dates.len());
        }
        None => {
            eprintln!("nog: no sync-DB entry for '{}'", package);
            eprintln!("total packages indexed: {}", dates.len());
            std::process::exit(1);
        }
    }
}

fn debug_hold(package: &str) {
    let cfg = config::NogConfig::load_default();
    let tier_manager = match tiers::TierManager::load(&cfg.paths.tier_pins) {
        Ok(tm) => tm,
        Err(e) => {
            eprintln!("nog: could not load tier pins: {}", e);
            std::process::exit(1);
        }
    };
    // v1.0.4: attach pkgbase coupling index so debug_hold reports the same
    // tier nog update would. Without this, _debug-hold libpipewire reports
    // Tier 3 while nog update buckets it Tier 2 — confusing.
    let tier_manager = tier_manager.with_pkgbase_index(
        tiers::PkgbaseIndex::from_packages(sync_db::load_packages())
    );

    let tier = tier_manager.classify(package);
    let dates = sync_db::load_build_dates();
    let status = holds::evaluate(
        package,
        tier.clone(),
        &dates,
        &cfg.holds,
        std::time::SystemTime::now(),
    );

    println!("package:   {}", package);
    println!("tier:      {}", tier);
    println!("window:    {} days", match tier {
        tiers::Tier::One => cfg.holds.tier1_days,
        tiers::Tier::Two => cfg.holds.tier2_days,
        tiers::Tier::Three => cfg.holds.tier3_days,
    });

    match dates.get(package) {
        Some(ts) => println!("built:     {} (Unix timestamp)", ts),
        None => println!("built:     (unknown — not in any sync DB)"),
    }

    match status {
        holds::HoldStatus::Expired { days_past_window } => {
            println!("status:    READY TO INSTALL (hold expired {} day{} ago)",
                days_past_window,
                if days_past_window == 1 { "" } else { "s" },
            );
        }
        holds::HoldStatus::Holding { days_remaining } => {
            println!("status:    HELD ({} day{} remaining)",
                days_remaining,
                if days_remaining == 1 { "" } else { "s" },
            );
        }
        holds::HoldStatus::SafetyWait { days_remaining } => {
            println!("status:    HELD (safety wait, {} day{} remaining)",
                days_remaining,
                if days_remaining == 1 { "" } else { "s" },
            );
        }
        holds::HoldStatus::Unknown => {
            println!("status:    UNKNOWN (no build date available)");
        }
    }
}