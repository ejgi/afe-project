use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use crate::types::Blueprint;

/// A complete, reusable analysis profile.
/// Stores everything the user would normally type on the command line.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct AnalysisProfile {
    /// Human-readable description of this profile
    pub description: Option<String>,
    /// The schema / blueprint for type overrides
    pub blueprint: Option<Blueprint>,
    /// Column separator (default: ',')
    pub delimiter: Option<char>,
    /// File format override: "csv", "json", "logs"
    pub format: Option<String>,
    /// Hardware profile: "auto", "hdd", "ssd"
    pub hardware_mode: Option<String>,
    /// Number of preamble rows to skip
    pub skip_rows: Option<usize>,
    /// Whether the file has a header row
    pub has_header: Option<bool>,
    /// Enable RFC 4180 strict CSV parsing
    pub rfc_4180: Option<bool>,
    /// Enable network/IP specialized analytics
    pub enable_network: Option<bool>,
    /// Enable GPU acceleration
    pub gpu: Option<bool>,
    /// Column extraction regex pattern
    pub regex_pattern: Option<String>,
    /// Chunk size in MB for segmented scanning
    pub chunk_size_mb: Option<usize>,
    /// Analysis accuracy level
    pub level: Option<String>,
    /// CPU Thread Throttling limit
    pub threads: Option<usize>,
    /// Bypass all CPU limits (Nitro Mode)
    pub no_limit: Option<bool>,
    /// Strip quotes from CSV fields
    pub strip_quotes: Option<bool>,
}

/// The top-level config file that lives at ~/.zen-engine-config.json
#[derive(Serialize, Deserialize, Default)]
pub struct EngineConfig {
    /// Global default hardware mode applied when not specified per-profile
    pub default_hardware_mode: String,
    /// Named analysis profiles (full settings)
    pub profiles: HashMap<String, AnalysisProfile>,
}

pub struct ConfigManager {
    path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Self {
        #[allow(deprecated)]
        let mut path = std::env::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(".zen-engine-config.json");
        Self { path }
    }

    pub fn load(&self) -> EngineConfig {
        if let Ok(content) = fs::read_to_string(&self.path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            EngineConfig::default()
        }
    }

    pub fn save(&self, config: &EngineConfig) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(config)?;
        fs::write(&self.path, content)?;
        Ok(())
    }

    pub fn save_profile(&self, name: String, profile: AnalysisProfile) -> anyhow::Result<()> {
        let mut config = self.load();
        config.profiles.insert(name, profile);
        self.save(&config)
    }

    pub fn get_profile(&self, name: &str) -> Option<AnalysisProfile> {
        let config = self.load();
        config.profiles.get(name).cloned()
    }

    pub fn list_profiles(&self) -> Vec<(String, Option<String>)> {
        let config = self.load();
        config.profiles
            .iter()
            .map(|(k, v)| (k.clone(), v.description.clone()))
            .collect()
    }

    pub fn delete_profile(&self, name: &str) -> anyhow::Result<()> {
        let mut config = self.load();
        if config.profiles.remove(name).is_some() {
            self.save(&config)?;
        }
        Ok(())
    }
}
