mod commands;
mod config;
mod pacman;
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
    }
}