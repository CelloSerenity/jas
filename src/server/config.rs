use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub scheduler: SchedulerConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub discovery: DiscoveryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind: String,
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub database_path: String,
    pub ipa_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub interval_hours: u64,
    pub refresh_window_days: i64,
    pub worker_threads: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecurityConfig {
    pub secret_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    pub mdns_enabled: bool,
    pub mdns_interface: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:3000".into(),
            log_level: "info".into(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            database_path: "jas.db".into(),
            ipa_dir: "ipas".into(),
        }
    }
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            interval_hours: 2,
            refresh_window_days: 3,
            worker_threads: 2,
        }
    }
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            mdns_enabled: true,
            mdns_interface: String::new(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = std::env::var("JAS_CONFIG").unwrap_or_else(|_| "jas.toml".into());
        let path = PathBuf::from(&path);

        if !path.exists() {
            tracing::warn!("Config file not found at {path:?}, using defaults");
            return Self::default();
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(cfg) => cfg,
                Err(e) => {
                    tracing::error!("Failed to parse config: {e}");
                    Self::default()
                }
            },
            Err(e) => {
                tracing::error!("Failed to read config: {e}");
                Self::default()
            }
        }
    }

    pub fn secret_key_bytes(&self) -> [u8; 32] {
        let key_str = if let Ok(env_key) = std::env::var("JAS_SECRET_KEY") {
            env_key
        } else {
            self.security.secret_key.clone()
        };

        if key_str.is_empty() {
            tracing::warn!("No secret key configured, using an insecure default. Set JAS_SECRET_KEY in production.");
            return [0u8; 32];
        }

        let bytes = hex::decode(&key_str).unwrap_or_else(|e| {
            tracing::error!("Failed to decode secret key hex: {e}");
            vec![0u8; 32]
        });

        let mut out = [0u8; 32];
        let len = bytes.len().min(32);
        out[..len].copy_from_slice(&bytes[..len]);
        out
    }
}
