use std::fmt::Display;

#[derive(Debug)]
pub enum AppError {
    InvalidInput(String),
    Internal(String),
}

impl AppError {
    pub fn invalid(message: impl Display) -> Self {
        Self::InvalidInput(format!("{message:#}"))
    }

    pub fn internal(error: impl Display) -> Self {
        Self::Internal(format!("{error:#}"))
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) | Self::Internal(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for AppError {}
