use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::Serialize;
use anyhow;

#[derive(Serialize)]
pub struct UploadResponse {
    pub download_url: String,
    pub filename: String,
    pub sha256_hash: String,
}

#[derive(Debug, Serialize)]
pub struct FileMeta {
    pub original_filename: String,
    pub sha256_hash: String,
    pub short_hash: String,
}

#[derive(Debug)]
pub enum AppError {
    UserError(String),
    InternalError(anyhow::Error),
    NotFound(String),
    ServiceUnavailable(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::UserError(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::InternalError(err) => {
                tracing::error!("Internal error: {:?}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg),
        };
        let body = Json(serde_json::json!({ "error": message }));
        (status, body).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::InternalError(err)
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::InternalError(err.into())
    }
}

impl From<axum::extract::multipart::MultipartError> for AppError {
    fn from(err: axum::extract::multipart::MultipartError) -> Self {
        AppError::UserError(format!("Multipart error: {}", err))
    }
} 