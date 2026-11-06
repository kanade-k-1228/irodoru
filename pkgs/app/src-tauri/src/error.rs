use std::fmt::Display;

#[derive(Debug)]
pub enum AppError {
    InvalidInput(String),
    NotFound(String),
    Internal(String),
}

impl AppError {
    pub fn invalid(message: impl Display) -> Self {
        Self::InvalidInput(format!("{message:#}"))
    }

    pub fn not_found(message: impl Display) -> Self {
        Self::NotFound(format!("{message:#}"))
    }

    pub fn internal(error: impl Display) -> Self {
        Self::Internal(format!("{error:#}"))
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(s) | Self::NotFound(s) | Self::Internal(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for AppError {}
