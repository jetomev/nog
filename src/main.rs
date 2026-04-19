mod aur;
mod commands;
mod config;
mod holds;
mod pacman;
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
    /// Install packages (tier classification shown; installs are never gated)
    ///
    /// Routes through the configured AUR helper if one is detected, so
    /// AUR-only packages work transparently. Tier protection applies to
    /// future updates, not to installs — explicit user commands execute
    /// user intent.
    Install {
        #[arg(required = true)]
        packages: Vec<String>,
    },
    /// Remove packages (works for both official and AUR-installed)
    Remove {
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
    Update,
    /// Search pacman repos; results annotated by tier (red/yellow/green)
    Search {
        query: String,
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
    /// Force-upgrade a held Tier 1 package via --promote; otherwise informational
    ///
    /// With `--promote`: bypass the hold window and any sign-off policy,
    /// upgrading the package immediately via pacman or the configured helper.
    /// Without `--promote`: just prints the held status (there is no per-session
    /// unlock state to toggle).
    Unlock {
        package: String,
        #[arg(long)]
        promote: bool,
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
    match cli.command {
        Commands::Install { packages } => commands::install(&packages),
        Commands::Remove { packages } => commands::remove(&packages),
        Commands::Update => commands::update(),
        Commands::Search { query } => commands::search(&query),
        Commands::Pin { package, tier } => commands::pin(&package, tier),
        Commands::Unlock { package, promote } => commands::unlock(&package, promote),
        Commands::DebugDates { package } => debug_dates(&package),
        Commands::DebugHold { package } => debug_hold(&package),
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
        holds::HoldStatus::Unknown => {
            println!("status:    UNKNOWN (no build date available)");
        }
    }
}