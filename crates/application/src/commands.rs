//! Application command handlers for integration operations.

use chrono::Utc;
use iaop_integration_service_domain::{
    AttemptStatus, CallbackReceipt, CallbackStatus, ExternalOperation, OperationAttempt,
    OperationStatus, ReconciliationOutcome, ReconciliationRecord,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::ports::{CallbackVerifierPort, ConnectorDispatcher, IntegrationStore};

#[derive(Debug, Clone)]
pub struct SubmitOperationRequest {
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

pub async fn submit_operation<S: IntegrationStore>(
    store: &S,
    req: SubmitOperationRequest,
) -> Result<ExternalOperation, AppError> {
    // 1. Check idempotency
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&req.payload).unwrap_or_default());
    let fingerprint = format!("{:x}", hasher.finalize());
    if let Some(existing) = store
        .get_by_idempotency(
            &req.tenant_id,
            &req.requesting_service,
            &req.operation_type,
            &req.idempotency_key,
        )
        .await?
    {
        if existing.request_fingerprint != fingerprint {
            return Err(AppError::Conflict(
                "idempotency key already used with different payload".into(),
            ));
        }
        return Ok(existing);
    }

    // 2. Validate provider exists and is active
    let provider = store
        .get_provider(&req.provider)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("provider '{}' not found", req.provider)))?;

    if !provider.is_active() {
        return Err(AppError::ProviderUnavailable(format!(
            "provider '{}' is not active",
            req.provider
        )));
    }

    // 3. Create aggregate
    let op = ExternalOperation::new(
        Uuid::now_v7(),
        req.tenant_id,
        req.requesting_service,
        req.provider,
        req.operation_type,
        req.business_reference,
        req.idempotency_key,
        fingerprint,
        req.payload,
        req.correlation_id,
        req.causation_id,
        req.max_attempts.unwrap_or(5),
    );

    store.save_operation(&op).await?;
    Ok(op)
}

pub async fn dispatch_operation<S: IntegrationStore, D: ConnectorDispatcher>(
    store: &S,
    dispatcher: &D,
    mut op: ExternalOperation,
) -> Result<ExternalOperation, AppError> {
    let provider = store
        .get_provider(&op.provider)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("provider '{}' not found", op.provider)))?;

    op.transition_to(OperationStatus::Running)?;
    store.save_operation(&op).await?;

    let attempt_num = op.attempt_count + 1;
    let mut attempt = OperationAttempt::new(op.id, attempt_num);
    let start = std::time::Instant::now();

    match dispatcher.dispatch(&provider, &op).await {
        Ok(res) => {
            let latency = start.elapsed().as_millis() as u64;
            attempt.complete_success(200, latency);
            op.record_success(res, None)?;
        }
        Err(err) => {
            let latency = start.elapsed().as_millis() as u64;
            let retryable = err.is_retryable();
            let status = if retryable {
                AttemptStatus::RetryableError
            } else {
                AttemptStatus::TerminalError
            };
            attempt.complete_error(
                status,
                None,
                Some("CONNECTOR_ERROR".into()),
                Some(err.to_string()),
                latency,
            );
            op.record_failure(err.to_string(), retryable)?;
        }
    }

    store.record_attempt(&attempt).await?;
    store.save_operation(&op).await?;
    Ok(op)
}

pub async fn process_callback<S: IntegrationStore, V: CallbackVerifierPort>(
    store: &S,
    verifier: &V,
    provider: String,
    signature: Option<String>,
    payload: serde_json::Value,
    raw_bytes: &[u8],
) -> Result<CallbackReceipt, AppError> {
    verifier
        .verify(&provider, signature.as_deref(), raw_bytes)
        .await?;

    let cb_ref = payload
        .get("callback_ref")
        .and_then(|v| v.as_str())
        .map(String::from);
    let mut receipt = CallbackReceipt::new(provider, cb_ref, signature, payload.clone());
    receipt.status = CallbackStatus::Verified;

    // Check if matching operation ID is in payload
    if let Some(op_id_str) = payload.get("operation_id").and_then(|v| v.as_str()) {
        if let Ok(op_id) = Uuid::parse_str(op_id_str) {
            if let Some(mut op) = store.get_operation(op_id).await? {
                receipt.operation_id = Some(op.id);
                let outcome = payload
                    .get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("SUCCESS");
                if outcome.eq_ignore_ascii_case("SUCCESS") {
                    let _ = op.record_success(payload.clone(), None);
                } else {
                    let _ = op
                        .record_failure(format!("callback indicated failure: {}", outcome), false);
                }
                store.save_operation(&op).await?;
                receipt.status = CallbackStatus::Processed;
                receipt.processed_at = Some(Utc::now());
            }
        }
    }

    store.save_callback(&receipt).await?;
    Ok(receipt)
}

pub async fn manual_retry<S: IntegrationStore>(
    store: &S,
    operation_id: Uuid,
) -> Result<ExternalOperation, AppError> {
    let mut op = store
        .get_operation(operation_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("operation '{}' not found", operation_id)))?;

    if !op.status.is_retryable() && op.status != OperationStatus::FailedFinal {
        return Err(AppError::Conflict(format!(
            "operation in status '{:?}' cannot be manually retried",
            op.status
        )));
    }

    op.transition_to(OperationStatus::Pending)?;
    op.next_retry_at = Some(Utc::now());
    op.locked_until = None;
    op.locked_by = None;
    op.max_attempts += 3; // Grant additional attempts for manual retry

    store.save_operation(&op).await?;
    Ok(op)
}

pub async fn reconcile_operation<S: IntegrationStore>(
    store: &S,
    operation_id: Uuid,
    provider_status: Option<String>,
    outcome: ReconciliationOutcome,
    details: Option<String>,
) -> Result<ReconciliationRecord, AppError> {
    let mut op = store
        .get_operation(operation_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("operation '{}' not found", operation_id)))?;

    let rec = ReconciliationRecord::new(op.id, provider_status.clone(), outcome, details);
    store.save_reconciliation(&rec).await?;

    match outcome {
        ReconciliationOutcome::MatchedSuccess => {
            let _ = op.record_success(
                serde_json::json!({"reconciled": true, "provider_status": provider_status}),
                None,
            );
            store.save_operation(&op).await?;
        }
        ReconciliationOutcome::MatchedFailure => {
            let _ = op.record_failure("reconciliation confirmed failure".into(), false);
            store.save_operation(&op).await?;
        }
        _ => {
            let _ = op.transition_to(OperationStatus::RequiresReview);
            store.save_operation(&op).await?;
        }
    }

    Ok(rec)
}
