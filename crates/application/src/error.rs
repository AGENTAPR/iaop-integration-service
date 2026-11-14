//! Application layer error types.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("provider unavailable: {0}")]
    ProviderUnavailable(String),

    #[error("domain error: {0}")]
    Domain(#[from] iaop_integration_service_domain::DomainError),

    #[error("infrastructure error: {0}")]
    Infra(String),
}

impl AppError {
    pub fn infra(e: impl std::fmt::Display) -> Self {
        Self::Infra(e.to_string())
    }
}
