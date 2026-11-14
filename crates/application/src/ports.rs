//! Ports (interfaces) that the application layer requires from the outside world.

use std::future::Future;
use std::pin::Pin;

use chrono::{DateTime, Duration, Utc};
use iaop_integration_service_domain::{
    CallbackReceipt, ConnectorError, EntityId, ExternalOperation, IntegrationProvider,
    OperationAttempt, ReconciliationRecord,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

/// Canonical integration-event envelope published through the outbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationEvent {
    pub event_id: Uuid,
    pub event_type: String,
    pub event_version: u32,
    pub source: String,
    pub aggregate_id: Option<EntityId>,
    pub aggregate_version: Option<u64>,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Option<Uuid>,
    pub causation_id: Option<Uuid>,
    pub payload: serde_json::Value,
}

#[derive(Debug, thiserror::Error)]
#[error("failed to publish event: {0}")]
pub struct PublishError(pub String);

/// Port: publish an integration event durably.
pub trait EventPublisher: Send + Sync {
    fn publish<'a>(
        &'a self,
        event: &'a IntegrationEvent,
    ) -> Pin<Box<dyn Future<Output = Result<(), PublishError>> + Send + 'a>>;
}

/// Port: transactional persistence for Integration aggregates.
pub trait IntegrationStore: Send + Sync {
    fn save_operation<'a>(
        &'a self,
        op: &'a ExternalOperation,
    ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>>;

    fn get_operation<'a>(
        &'a self,
        id: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<Option<ExternalOperation>, AppError>> + Send + 'a>>;

    fn get_by_idempotency<'a>(
        &'a self,
        tenant_id: &'a str,
        requesting_service: &'a str,
        operation_type: &'a str,
        idempotency_key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<ExternalOperation>, AppError>> + Send + 'a>>;

    fn claim_pending_operations<'a>(
        &'a self,
        limit: i64,
        lease_duration: Duration,
        worker_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ExternalOperation>, AppError>> + Send + 'a>>;

    fn record_attempt<'a>(
        &'a self,
        attempt: &'a OperationAttempt,
    ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>>;

    fn get_attempts<'a>(
        &'a self,
        operation_id: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<OperationAttempt>, AppError>> + Send + 'a>>;

    fn save_callback<'a>(
        &'a self,
        callback: &'a CallbackReceipt,
    ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>>;

    fn save_reconciliation<'a>(
        &'a self,
        record: &'a ReconciliationRecord,
    ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>>;

    fn list_providers<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<IntegrationProvider>, AppError>> + Send + 'a>>;

    fn get_provider<'a>(
        &'a self,
        id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<IntegrationProvider>, AppError>> + Send + 'a>>;

    fn list_operations<'a>(
        &'a self,
        tenant_id: &'a str,
        limit: i64,
        offset: i64,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ExternalOperation>, AppError>> + Send + 'a>>;
}

/// Port: connector dispatcher for executing provider operations.
pub trait ConnectorDispatcher: Send + Sync {
    fn dispatch<'a>(
        &'a self,
        provider: &'a IntegrationProvider,
        op: &'a ExternalOperation,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, ConnectorError>> + Send + 'a>>;
}

/// Port: callback verification for HMAC-SHA256 signatures.
pub trait CallbackVerifierPort: Send + Sync {
    fn verify<'a>(
        &'a self,
        provider: &'a str,
        signature: Option<&'a str>,
        payload: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>>;
}
