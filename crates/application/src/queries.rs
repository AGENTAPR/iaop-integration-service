//! Application query handlers.

use iaop_integration_service_domain::{ExternalOperation, IntegrationProvider, OperationAttempt};
use uuid::Uuid;

use crate::error::AppError;
use crate::ports::IntegrationStore;

pub async fn get_operation<S: IntegrationStore>(
    store: &S,
    id: Uuid,
) -> Result<Option<ExternalOperation>, AppError> {
    store.get_operation(id).await
}

pub async fn get_operation_attempts<S: IntegrationStore>(
    store: &S,
    operation_id: Uuid,
) -> Result<Vec<OperationAttempt>, AppError> {
    store.get_attempts(operation_id).await
}

pub async fn list_providers<S: IntegrationStore>(
    store: &S,
) -> Result<Vec<IntegrationProvider>, AppError> {
    store.list_providers().await
}

pub async fn list_operations<S: IntegrationStore>(
    store: &S,
    tenant_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<ExternalOperation>, AppError> {
    store.list_operations(tenant_id, limit, offset).await
}
