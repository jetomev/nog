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
    about = "Kognog OS package manager",
    version = env!("CARGO_PKG_VERSION"),
    long_about = "nog wraps pacman with tier-aware update management for Kognog OS."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install one or more packages
    Install {
        #[arg(required = true)]
        packages: Vec<String>,
    },
    /// Remove one or more packages
    Remove {
        #[arg(required = true)]
        packages: Vec<String>,
    },
    /// Update all packages (respects tier holds)
    Update,
    /// Search for a package
    Search {
        query: String,
    },
    /// Pin a package to a specific tier
    Pin {
        package: String,
        #[arg(long, default_value = "1")]
        tier: u8,
    },
    /// Unlock a tier-1 package for manual update
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