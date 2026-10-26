use serde::Serialize;
use thiserror::Error;

/// Every command returns this. The window shows the message as it is, so each one is written
/// for a creator to read: what went wrong, and what was (or was not) changed.
#[derive(Error, Debug)]
pub enum AppError {
    #[error("{0}")]
    Message(String),

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("Could not reach the internet: {0}")]
    Network(String),

    #[error("{0}")]
    Archive(String),
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Network(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Message(format!("Could not read the data: {err}"))
    }
}

impl From<zip::result::ZipError> for AppError {
    fn from(err: zip::result::ZipError) -> Self {
        AppError::Archive(format!("The download could not be opened as a zip: {err}"))
    }
}

pub type AppResult<T> = Result<T, AppError>;

/// Shorthand for a plain message error.
pub fn fail<T>(msg: impl Into<String>) -> AppResult<T> {
    Err(AppError::Message(msg.into()))
}
