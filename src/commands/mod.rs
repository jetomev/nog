use crate::tiers::{Tier, TierManager};
use crate::config::NogConfig;
use crate::pacman;

pub fn install(packages: &[String]) {
    let tm = load_tiers();
    let mut blocked = false;

    for pkg in packages {
        let tier = tm.classify(pkg);
        match tier {
            Tier::One => {
                eprintln!(
                    "nog: '{}' is {} — blocked. Use 'nog unlock {}' first.",
                    pkg, tier, pkg
                );
                blocked = true;
            }
            Tier::Two => {
                println!(
                    "nog: '{}' is {} — hold period applies, proceeding with install.",
                    pkg, tier
                );
            }
            Tier::Three => {
                println!("nog: '{}' is {} — proceeding.", pkg, tier);
            }
        }
    }

    if blocked {
        eprintln!("nog: install aborted — one or more Tier 1 packages require manual unlock.");
        std::process::exit(1);
    }

    let status = pacman::install(packages);
    if !status.success() {
        eprintln!("nog: pacman exited with status {}", status);
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
    let tm = load_tiers();
    println!("nog: checking tier holds before update...\n");

    let tier1_pkgs: Vec<String> = tm.tier1_packages();
    if !tier1_pkgs.is_empty() {
        println!("  Tier 1 packages (held — manual sign-off required):");
        for pkg in &tier1_pkgs {
            println!("    [HELD] {}", pkg);
        }
        println!();
    }

    println!("nog: running update (Tier 1 packages excluded)...\n");
    let status = pacman::update_excluding(&tier1_pkgs);
    if !status.success() {
        eprintln!("nog: pacman exited with status {}", status);
        std::process::exit(status.code().unwrap_or(1));
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
    let tm = load_tiers();
    let tier = tm.classify(package);

    if tier != Tier::One {
        println!("nog: '{}' is {} — no unlock needed.", package, tier);
        return;
    }

    println!("nog: unlocking '{}'...", package);
    if promote {
        println!("nog: promoting '{}' — calling pacman to install/upgrade.", package);
        let pkgs = vec![package.to_string()];
        let status = pacman::install(&pkgs);
        if !status.success() {
            eprintln!("nog: pacman exited with status {}", status);
            std::process::exit(status.code().unwrap_or(1));
        }
    } else {
        println!(
            "nog: '{}' unlocked for this session. Run 'nog unlock {} --promote' to apply.",
            package, package
        );
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