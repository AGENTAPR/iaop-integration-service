//! Reconciliation records for unknown or mismatched outcomes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReconciliationOutcome {
    MatchedSuccess,
    MatchedFailure,
    StateConflict,
    Inconclusive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationRecord {
    pub id: Uuid,
    pub operation_id: Uuid,
    pub checked_at: DateTime<Utc>,
    pub provider_status: Option<String>,
    pub outcome: ReconciliationOutcome,
    pub details: Option<String>,
}

impl ReconciliationRecord {
    pub fn new(
        operation_id: Uuid,
        provider_status: Option<String>,
        outcome: ReconciliationOutcome,
        details: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            operation_id,
            checked_at: Utc::now(),
            provider_status,
            outcome,
            details,
        }
    }
}
