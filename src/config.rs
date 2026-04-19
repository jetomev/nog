use serde::Deserialize;
use std::fs;
use std::sync::OnceLock;

#[derive(Debug, Deserialize, Clone)]
pub struct GeneralConfig {
    pub version: String,
    pub log_level: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PathsConfig {
    pub tier_pins: String,
    pub pacman_conf: String,
    pub log_file: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ReposConfig {
    pub staging: String,
    pub testing: String,
    pub stable: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HoldsConfig {
    pub tier1_days: u32,
    pub tier2_days: u32,
    pub tier3_days: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AurConfig {
    /// One of: "auto" | "yay" | "paru" | "none".
    ///   "auto" — prefer yay if present, else paru, else disable AUR support
    ///   "yay" / "paru" — require that specific helper; error if missing
    ///   "none" — skip all AUR-aware paths even if a helper is installed
    pub helper: String,
}

impl Default for AurConfig {
    fn default() -> Self {
        AurConfig { helper: "auto".to_string() }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct NogConfig {
    pub general: GeneralConfig,
    pub paths: PathsConfig,
    pub repos: ReposConfig,
    pub holds: HoldsConfig,
    // Phase 4 added the [aur] section. Existing installs without it should
    // keep working with the default ("auto" helper).
    #[serde(default)]
    pub aur: AurConfig,
}

impl NogConfig {
    pub fn load(path: &str) -> Result<Self, String> {
        let contents = fs::read_to_string(path)
            .map_err(|e| format!("Could not read {}: {}", path, e))?;
        toml::from_str(&contents)
            .map_err(|e| format!("Could not parse nog.conf: {}", e))
    }

    /// Load the config with the standard fallback chain, caching the result
    /// so repeat calls within a single process invocation don't re-read the
    /// file (or, when the file is missing, don't print the "no nog.conf
    /// found" warning multiple times). Callers get an owned clone.
    pub fn load_default() -> Self {
        static CACHED: OnceLock<NogConfig> = OnceLock::new();
        CACHED.get_or_init(Self::resolve_once).clone()
    }

    /// The actual resolution logic — called at most once per process through
    /// the `CACHED` OnceLock above. Any side effects (the "no nog.conf
    /// found" warning) fire here, exactly once.
    fn resolve_once() -> Self {
        // Try the system path first.
        if let Ok(cfg) = Self::load("/etc/nog/nog.conf") {
            return cfg;
        }

        // Dev fallback: resolved at compile time to the cargo manifest dir.
        // In release builds this branch would embed the maintainer's build
        // path into the final binary as a string literal — see the
        // debug_assertions gate below (F2) that removes the embedding in
        // release builds.
        #[cfg(debug_assertions)]
        {
            let dev_nog_conf = concat!(env!("CARGO_MANIFEST_DIR"), "/config/nog.conf");
            let dev_tier_pins = concat!(env!("CARGO_MANIFEST_DIR"), "/config/tier-pins.toml");
            if let Ok(mut cfg) = Self::load(dev_nog_conf) {
                cfg.paths.tier_pins = dev_tier_pins.to_string();
                return cfg;
            }
        }

        eprintln!("nog warning: no nog.conf found — using built-in defaults");
        NogConfig {
            general: GeneralConfig {
                version: "0.1.0".to_string(),
                log_level: "info".to_string(),
            },
            paths: PathsConfig {
                // Built-in-defaults path: point at the canonical system
                // location. If that file is also missing, tier loading will
                // fail with a clean error (F5 fix in commands::load_tiers).
                tier_pins: "/etc/nog/tier-pins.toml".to_string(),
                pacman_conf: "/etc/pacman.conf".to_string(),
                log_file: "/var/log/nog.log".to_string(),
            },
            repos: ReposConfig {
                staging: "https://repo.kognog.org/staging".to_string(),
                testing: "https://repo.kognog.org/testing".to_string(),
                stable:  "https://repo.kognog.org/stable".to_string(),
            },
            holds: HoldsConfig {
                tier1_days: 30,
                tier2_days: 15,
                tier3_days: 7,
            },
            aur: AurConfig::default(),
        }
    }
}
