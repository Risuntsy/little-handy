use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone)]
pub struct AppState {
    pub upload_path: PathBuf,
    pub base_url: String,
    pub max_file_size: usize,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub listen_addr: String,
    pub base_url: String,
}

#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    pub upload_dir: String,
    pub max_file_size_mb: usize,
    pub cleanup_days: i64,
    pub cleanup_schedule: String,
}

#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub logging: LoggingConfig,
}

impl Config {
    pub fn new() -> anyhow::Result<Self> {
        let mut config_path = PathBuf::from("config/app_config.toml");
        if !config_path.exists() {
            config_path = PathBuf::from("config/sample.toml");
        }
        let config_str = std::fs::read_to_string(config_path)?;
        let mut config: Config = toml::from_str(&config_str)?;

        // Override with environment variables if they exist
        if let Ok(base_url) = std::env::var("BASE_URL") {
            config.server.base_url = base_url;
        }
        if let Ok(listen_addr) = std::env::var("LISTEN_ADDR") {
            config.server.listen_addr = listen_addr;
        }
        if let Ok(upload_dir) = std::env::var("UPLOAD_DIR") {
            config.storage.upload_dir = upload_dir;
        }
        if let Ok(max_file_size) = std::env::var("MAX_FILE_SIZE_MB") {
            config.storage.max_file_size_mb = max_file_size.parse()?;
        }
        if let Ok(cleanup_days) = std::env::var("CLEANUP_DAYS") {
            config.storage.cleanup_days = cleanup_days.parse()?;
        }
        if let Ok(cleanup_schedule) = std::env::var("CLEANUP_SCHEDULE") {
            config.storage.cleanup_schedule = cleanup_schedule;
        }
        if let Ok(log_level) = std::env::var("RUST_LOG") {
            config.logging.level = log_level;
        }

        Ok(config)
    }

    pub fn max_file_size_bytes(&self) -> usize {
        self.storage.max_file_size_mb * 1024 * 1024
    }
} 