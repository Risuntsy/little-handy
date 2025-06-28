use anyhow::Result;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub listen_addr: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfig {
    pub temp_file_host_url: String,
    pub max_response_size_bytes: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CurlConfig {
    pub timeout_seconds: u64,
    pub follow_redirects: bool,
    pub include_headers: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub proxy: ProxyConfig,
    pub curl: CurlConfig,
}

impl Config {
    pub fn new() -> Result<Self> {
        let config_path = PathBuf::from("config/app_config.toml");
        if !config_path.exists() {
            return Err(anyhow::anyhow!("Config file not found at {:?}", config_path));
        }
        
        let config_str = std::fs::read_to_string(&config_path)
            .map_err(|e| anyhow::anyhow!("Failed to read config file {:?}: {}", config_path, e))?;
        
        let config: Config = toml::from_str(&config_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))?;
        
        Ok(config)
    }
} 