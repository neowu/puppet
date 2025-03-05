use std::fmt::Display;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;

pub type HttpResult<T> = Result<T, HttpError>;

#[derive(Debug)]
pub enum HttpError {
    NotFound(String),
    InternalError(anyhow::Error),
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        match self {
            HttpError::InternalError(error) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Internal Error: {}", error)).into_response()
            }
            HttpError::NotFound(error) => (StatusCode::NOT_FOUND, format!("Not Found: {}", error)).into_response(),
        }
    }
}

impl<E> From<E> for HttpError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self::InternalError(err.into())
    }
}

impl Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}
