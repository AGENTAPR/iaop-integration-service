//! Data Transfer Objects (DTOs) for Integration Service API.

use chrono::{DateTime, Utc};
use iaop_integration_service_domain::{
    AttemptStatus, CallbackStatus, ExternalOperation, IntegrationProvider, OperationAttempt,
    OperationStatus, ProviderKind, ProviderStatus, ReconciliationOutcome, ReconciliationRecord,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Wire format for `GET /health/live` and `/health/ready`.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    pub status: &'static str,
}

/// Wire format for `GET /api/v1/system/info`.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SystemInfoResponse {
    pub service: String,
    pub version: String,
    pub environment: String,
    pub status: &'static str,
}

/// Request to submit an asynchronous integration operation.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SubmitOperationDto {
    pub tenant_id: String,
    pub requesting_service: String,
    pub provider: String,
    pub operation_type: String,
    pub business_reference: String,
    pub idempotency_key: String,
    pub payload: serde_json::Value,
    pub correlation_id: Option<Uuid>,
    pub causation_id: Option<Uuid>,
    pub max_attempts: Option<i32>,
}

/// Full view of an external integration operation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OperationResponseDto {
    pub id: Uuid,
    pub tenant_id: String,
    pub requesting_service: String,
    pub provider: String,
    pub operation_type: String,
    pub business_reference: String,
    pub idempotency_key: String,
    pub external_reference: Option<String>,
    pub status: OperationStatus,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub payload: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub error_details: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl From<ExternalOperation> for OperationResponseDto {
    fn from(op: ExternalOperation) -> Self {
        Self {
            id: op.id,
            tenant_id: op.tenant_id,
            requesting_service: op.requesting_service,
            provider: op.provider,
            operation_type: op.operation_type,
            business_reference: op.business_reference,
            idempotency_key: op.idempotency_key,
            external_reference: op.external_reference,
            status: op.status,
            attempt_count: op.attempt_count,
            max_attempts: op.max_attempts,
            next_retry_at: op.next_retry_at,
            payload: op.payload,
            result: op.result,
            error_details: op.error_details,
            created_at: op.created_at,
            updated_at: op.updated_at,
            completed_at: op.completed_at,
        }
    }
}

/// Attempt history record.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AttemptResponseDto {
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

impl From<OperationAttempt> for AttemptResponseDto {
    fn from(a: OperationAttempt) -> Self {
        Self {
            id: a.id,
            operation_id: a.operation_id,
            attempt_number: a.attempt_number,
            dispatched_at: a.dispatched_at,
            completed_at: a.completed_at,
            status: a.status,
            http_status: a.http_status,
            error_code: a.error_code,
            error_message: a.error_message,
            latency_ms: a.latency_ms,
        }
    }
}

/// Provider details and availability.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProviderResponseDto {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub status: ProviderStatus,
    pub base_url: String,
    pub timeout_ms: u64,
}

impl From<IntegrationProvider> for ProviderResponseDto {
    fn from(p: IntegrationProvider) -> Self {
        Self {
            id: p.id,
            name: p.name,
            kind: p.kind,
            status: p.status,
            base_url: p.base_url,
            timeout_ms: p.timeout_ms,
        }
    }
}

/// Callback ingestion response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CallbackResponseDto {
    pub callback_id: Uuid,
    pub provider: String,
    pub status: CallbackStatus,
    pub operation_id: Option<Uuid>,
    pub received_at: DateTime<Utc>,
}

/// Request to reconcile an operation.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ReconcileRequestDto {
    pub provider_status: Option<String>,
    pub outcome: ReconciliationOutcome,
    pub details: Option<String>,
}

/// Reconciliation outcome response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReconcileResponseDto {
    pub reconciliation_id: Uuid,
    pub operation_id: Uuid,
    pub checked_at: DateTime<Utc>,
    pub provider_status: Option<String>,
    pub outcome: ReconciliationOutcome,
    pub details: Option<String>,
}

impl From<ReconciliationRecord> for ReconcileResponseDto {
    fn from(r: ReconciliationRecord) -> Self {
        Self {
            reconciliation_id: r.id,
            operation_id: r.operation_id,
            checked_at: r.checked_at,
            provider_status: r.provider_status,
            outcome: r.outcome,
            details: r.details,
        }
    }
}
