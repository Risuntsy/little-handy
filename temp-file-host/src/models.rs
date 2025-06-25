use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::Serialize;
use std::io;

#[derive(Serialize)]
pub struct UploadResponse {
    pub download_url: String,
    pub filename: String,
    pub sha256_hash: String,
}

pub enum AppError {
    IoError(io::Error),
    MultipartError(axum::extract::multipart::MultipartError),
    PayloadTooLarge(String),
    NotFound(String),
    UserError(String),   // 客户端请求问题
    ServerError(String), // 服务器内部问题
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::IoError(e) => {
                tracing::error!("I/O error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Internal Server Error: {}", e),
                )
            }
            AppError::MultipartError(e) => {
                tracing::error!("Multipart error: {}", e);
                (
                    StatusCode::BAD_REQUEST,
                    format!("Bad Request: Invalid multipart data: {}", e),
                )
            }
            AppError::PayloadTooLarge(msg) => (StatusCode::PAYLOAD_TOO_LARGE, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::UserError(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::ServerError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(serde_json::json!({ "error": error_message }));
        (status, body).into_response()
    }
}

impl From<io::Error> for AppError {
    fn from(err: io::Error) -> Self {
        AppError::IoError(err)
    }
}

impl From<axum::extract::multipart::MultipartError> for AppError {
    fn from(err: axum::extract::multipart::MultipartError) -> Self {
        AppError::MultipartError(err)
    }
} 