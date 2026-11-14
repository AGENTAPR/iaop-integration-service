//! Connector traits and error classification.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorOperation {
    pub provider: String,
    pub operation_type: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum ConnectorError {
    #[error("network or transient error (retryable): {0}")]
    RetryableNetwork(String),

    #[error("rate limited by provider (retry after {retry_after_secs}s)")]
    RetryableRateLimited { retry_after_secs: u32 },

    #[error("terminal validation error from provider: {0}")]
    TerminalValidation(String),

    #[error("terminal authentication or authorization error: {0}")]
    TerminalAuthentication(String),

    #[error("unknown external outcome (requires reconciliation): {0}")]
    UnknownOutcome(String),
}

impl ConnectorError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RetryableNetwork(_) | Self::RetryableRateLimited { .. }
        )
    }
}
