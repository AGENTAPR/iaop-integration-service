//! Callback receipts and validation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CallbackStatus {
    Received,
    Verified,
    Processed,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallbackReceipt {
    pub id: Uuid,
    pub provider: String,
    pub callback_ref: Option<String>,
    pub signature: Option<String>,
    pub payload: serde_json::Value,
    pub status: CallbackStatus,
    pub operation_id: Option<Uuid>,
    pub received_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

impl CallbackReceipt {
    pub fn new(
        provider: String,
        callback_ref: Option<String>,
        signature: Option<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            provider,
            callback_ref,
            signature,
            payload,
            status: CallbackStatus::Received,
            operation_id: None,
            received_at: Utc::now(),
            processed_at: None,
        }
    }
}
