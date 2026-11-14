//! Operation attempt tracking.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttemptStatus {
    Dispatched,
    Success,
    RetryableError,
    TerminalError,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationAttempt {
    pub id: Uuid,
    pub operation_id: Uuid,
    pub attempt_number: i32,
    pub dispatched_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: AttemptStatus,
    pub http_status: Option<u16>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub latency_ms: Option<u64>,
}

impl OperationAttempt {
    pub fn new(operation_id: Uuid, attempt_number: i32) -> Self {
        Self {
            id: Uuid::now_v7(),
            operation_id,
            attempt_number,
            dispatched_at: Utc::now(),
            completed_at: None,
            status: AttemptStatus::Dispatched,
            http_status: None,
            error_code: None,
            error_message: None,
            latency_ms: None,
        }
    }

    pub fn complete_success(&mut self, http_status: u16, latency_ms: u64) {
        self.completed_at = Some(Utc::now());
        self.status = AttemptStatus::Success;
        self.http_status = Some(http_status);
        self.latency_ms = Some(latency_ms);
    }

    pub fn complete_error(
        &mut self,
        status: AttemptStatus,
        http_status: Option<u16>,
        code: Option<String>,
        msg: Option<String>,
        latency_ms: u64,
    ) {
        self.completed_at = Some(Utc::now());
        self.status = status;
        self.http_status = http_status;
        self.error_code = code;
        self.error_message = msg;
        self.latency_ms = Some(latency_ms);
    }
}
