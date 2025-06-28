use axum::{
    extract::{Query, Request, State},
    http::HeaderMap,
    response::Response,
};
use std::{collections::HashMap, sync::Arc};

use crate::{
    errors::AppError,
    models::AppState,
    services::execute_curl_command,
};

pub async fn curl_proxy(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    req: Request,
) -> Result<Response, AppError> {
    // 获取目标URL
    let target_url = params
        .get("url")
        .ok_or_else(|| AppError::BadRequest("Missing 'url' parameter".to_string()))?;

    // 获取HTTP方法
    let method = req.method().as_str();

    // 执行curl命令
    execute_curl_command(&state, target_url, &headers, method).await
}

pub async fn health_check() -> &'static str {
    "curl2url service is running"
} 