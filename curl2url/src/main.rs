use anyhow::Result;
use axum::{routing::get, Router};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::info;

use curl2url::{
    config::Config,
    handlers::{curl_proxy, health_check},
    models::AppState,
};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    // 加载配置
    let config = Config::new()?;
    info!("Loaded configuration: temp_file_host_url={}, max_response_size={}MB", 
          config.proxy.temp_file_host_url, 
          config.proxy.max_response_size_bytes / (1024 * 1024));

    // 创建HTTP客户端
    let http_client = reqwest::Client::new();

    // 创建应用状态
    let app_state = Arc::new(AppState {
        config: config.clone(),
        http_client,
    });

    // 构建路由
    let app = Router::new()
        .route("/", get(health_check))
        .route("/curl", get(curl_proxy))
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    // 启动服务器
    let listener = tokio::net::TcpListener::bind(&config.server.listen_addr).await?;
    info!("curl2url service listening on http://{}", config.server.listen_addr);
    info!("Usage: GET /curl?url=<target_url>");
    info!("Large responses (>{}MB) will be uploaded to: {}", 
          config.proxy.max_response_size_bytes / (1024 * 1024),
          config.proxy.temp_file_host_url);
    
    axum::serve(listener, app).await?;

    Ok(())
}
