use anyhow::Result;
use dashmap::DashMap;
use moka::future::Cache;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::proxy::JobStatus;

#[derive(Clone)]
pub struct AppState {
    pub upload_path: PathBuf,
    pub base_url: String,
    pub max_file_size: usize,
    pub http_client: reqwest::Client,
    pub proxy_config: Arc<ProxyConfig>,
    pub job_cache: Cache<String, Arc<JobStatus>>,
    pub job_status_map: Arc<DashMap<Uuid, Arc<JobStatus>>>,
    pub job_semaphore: Arc<Semaphore>,
    pub auth_config: Arc<AuthConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub listen_addr: String,
    pub base_url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StorageConfig {
    pub upload_dir: String,
    pub max_file_size_mb: usize,
    pub cleanup_days: i64,
    pub cleanup_schedule: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProxyConfig {
    pub cache_ttl_seconds: u64,
    pub cache_max_capacity: u64,
    pub remote_request_timeout_seconds: u64,
    pub max_concurrent_jobs: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthConfig {
    pub allowed_tokens: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub logging: LoggingConfig,
    pub proxy: ProxyConfig,
    pub auth: AuthConfig,
}

impl Config {
    pub fn new() -> Result<Self> {
        let config_path = std::env::var("APP_CONFIG_PATH").unwrap_or("config/sample.toml".to_string());
        
        let config_str = std::fs::read_to_string(&config_path)
            .map_err(|e| anyhow::anyhow!("Failed to read config file at '{}': {}", config_path, e))?;

        let config: Config = toml::from_str(&config_str)?;

        Ok(config)
    }

    pub fn max_file_size_bytes(&self) -> usize {
        self.storage.max_file_size_mb * 1024 * 1024
    }
} 