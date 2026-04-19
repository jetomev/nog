use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct GeneralConfig {
    pub version: String,
    pub log_level: String,
}

#[derive(Debug, Deserialize)]
pub struct PathsConfig {
    pub tier_pins: String,
    pub pacman_conf: String,
    pub log_file: String,
}

#[derive(Debug, Deserialize)]
pub struct ReposConfig {
    pub staging: String,
    pub testing: String,
    pub stable: String,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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

    pub fn load_default() -> Self {
        // Dev fallback paths — resolved at compile time to the absolute path of
        // the project root, so they work regardless of the current working directory.
        let dev_nog_conf = concat!(env!("CARGO_MANIFEST_DIR"), "/config/nog.conf");
        let dev_tier_pins = concat!(env!("CARGO_MANIFEST_DIR"), "/config/tier-pins.toml");

        // Try the system path first; fall back to the dev path during development.
        // If the dev config loads, override tier_pins to the dev-absolute path so
        // the user doesn't need /etc/nog/tier-pins.toml installed to test locally.
        if let Ok(cfg) = Self::load("/etc/nog/nog.conf") {
            return cfg;
        }

        if let Ok(mut cfg) = Self::load(dev_nog_conf) {
            cfg.paths.tier_pins = dev_tier_pins.to_string();
            return cfg;
        }

        eprintln!("nog warning: no nog.conf found — using built-in defaults");
        NogConfig {
            general: GeneralConfig {
                version: "0.1.0".to_string(),
                log_level: "info".to_string(),
            },
            paths: PathsConfig {
                tier_pins: dev_tier_pins.to_string(),
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
