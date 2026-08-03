use std::fmt;

use rusqlite::{Error as SqliteError, ErrorCode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    NotFound,
    Invalid(String),
    Conflict(String),
    Internal(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    pub fn ipc_message(&self) -> String {
        match self {
            Self::NotFound => "The requested item was not found".to_string(),
            Self::Invalid(message) | Self::Conflict(message) => message.clone(),
            Self::Internal(_) => "An internal error occurred".to_string(),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.ipc_message())
    }
}

impl std::error::Error for AppError {}

impl From<SqliteError> for AppError {
    fn from(error: SqliteError) -> Self {
        match error {
            SqliteError::QueryReturnedNoRows => Self::NotFound,
            SqliteError::SqliteFailure(ref sqlite_error, _)
                if sqlite_error.code == ErrorCode::ConstraintViolation =>
            {
                Self::Conflict("The requested change conflicts with existing data".to_string())
            }
            other => Self::Internal(other.to_string()),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        Self::Invalid(error.to_string())
    }
}
