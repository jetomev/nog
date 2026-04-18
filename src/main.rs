mod commands;
mod config;
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
    }
}

fn debug_dates(package: &str) {
    let dates = sync_db::load_build_dates();
    match dates.get(package) {
        Some(ts) => {
            println!("package:    {}", package);
            println!("build_date: {} (Unix timestamp)", ts);
            // Use the system `date` command for a human-readable rendering —
            // this mirrors what `date -d @<ts>` would show, and avoids pulling
            // in a chrono/time dependency just for debug output.
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