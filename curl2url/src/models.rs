use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use std::collections::HashMap;

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Config,
    pub http_client: reqwest::Client,
}

#[derive(Debug, Serialize)]
pub struct CurlResponse {
    pub curl_command: String,
    pub response_body: Option<String>,
    pub response_headers: Option<HashMap<String, String>>,
    pub status_code: Option<u16>,
    pub error: Option<String>,
    pub redirected: bool,
    pub redirect_url: Option<String>,
    pub uploaded_to_temp_host: bool,
    pub temp_file_url: Option<String>,
}

impl IntoResponse for CurlResponse {
    fn into_response(self) -> Response {
        let json = serde_json::to_string(&self).unwrap_or_else(|_| "{}".to_string());
        (StatusCode::OK, json).into_response()
    }
} 