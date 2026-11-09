use std::fmt::Display;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

use irodoru_engine::AppError;

#[derive(Serialize, ToSchema)]
pub struct ErrorBody {
    pub error: String,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn from_json(rejection: axum::extract::rejection::JsonRejection) -> Self {
        Self::new(rejection.status(), rejection.body_text())
    }

    pub fn bad_request(msg: impl Display) -> Self {
        Self::new(StatusCode::BAD_REQUEST, msg)
    }

    pub fn internal(msg: impl Display) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, msg)
    }

    fn new(status: StatusCode, msg: impl Display) -> Self {
        let message = format!("{msg:#}");
        Self { status, message }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

impl From<AppError> for ApiError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::InvalidInput(message) => Self::bad_request(message),
            AppError::Internal(message) => Self::internal(message),
        }
    }
}
