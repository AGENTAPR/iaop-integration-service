//! PostgreSQL implementation of IntegrationStore using dynamic sqlx queries.

use std::future::Future;
use std::pin::Pin;

use chrono::{Duration, Utc};
use iaop_integration_service_application::error::AppError;
use iaop_integration_service_application::ports::IntegrationStore;
use iaop_integration_service_domain::{
    AttemptStatus, CallbackReceipt, ExternalOperation, IntegrationProvider, OperationAttempt,
    OperationStatus, ProviderKind, ProviderStatus, ReconciliationRecord,
};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgIntegrationStore {
    pool: PgPool,
}

impl PgIntegrationStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn db_err(e: sqlx::Error) -> AppError {
    AppError::infra(e)
}

fn map_op_row(row: &sqlx::postgres::PgRow) -> Result<ExternalOperation, AppError> {
    let status_str: String = row.try_get("status").map_err(db_err)?;
    let status: OperationStatus = serde_json::from_value(serde_json::Value::String(status_str))
        .unwrap_or(OperationStatus::RequiresReview);

    Ok(ExternalOperation {
        id: row.try_get("id").map_err(db_err)?,
        tenant_id: row.try_get("tenant_id").map_err(db_err)?,
        requesting_service: row.try_get("requesting_service").map_err(db_err)?,
        provider: row.try_get("provider").map_err(db_err)?,
        operation_type: row.try_get("operation_type").map_err(db_err)?,
        business_reference: row.try_get("business_reference").map_err(db_err)?,
        idempotency_key: row.try_get("idempotency_key").map_err(db_err)?,
        request_fingerprint: row.try_get("request_fingerprint").map_err(db_err)?,
        correlation_id: row.try_get("correlation_id").map_err(db_err)?,
        causation_id: row.try_get("causation_id").map_err(db_err)?,
        external_reference: row.try_get("external_reference").map_err(db_err)?,
        status,
        attempt_count: row.try_get("attempt_count").map_err(db_err)?,
        max_attempts: row.try_get("max_attempts").map_err(db_err)?,
        next_retry_at: row.try_get("next_retry_at").map_err(db_err)?,
        locked_until: row.try_get("locked_until").map_err(db_err)?,
        locked_by: row.try_get("locked_by").map_err(db_err)?,
        payload: row.try_get("payload").map_err(db_err)?,
        result: row.try_get("result").map_err(db_err)?,
        error_details: row.try_get("error_details").map_err(db_err)?,
        created_at: row.try_get("created_at").map_err(db_err)?,
        updated_at: row.try_get("updated_at").map_err(db_err)?,
        completed_at: row.try_get("completed_at").map_err(db_err)?,
    })
}

impl IntegrationStore for PgIntegrationStore {
    fn save_operation<'a>(
        &'a self,
        op: &'a ExternalOperation,
    ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>> {
        Box::pin(async move {
            let status = serde_json::to_string(&op.status)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string();

            sqlx::query(
                r#"
                INSERT INTO integration_operations (
                    id, tenant_id, requesting_service, provider, operation_type,
                    business_reference, idempotency_key, request_fingerprint,
                    correlation_id, causation_id, external_reference, status,
                    attempt_count, max_attempts, next_retry_at, locked_until,
                    locked_by, payload, result, error_details, created_at,
                    updated_at, completed_at
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                    $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23
                )
                ON CONFLICT (id) DO UPDATE SET
                    status = EXCLUDED.status,
                    external_reference = EXCLUDED.external_reference,
                    attempt_count = EXCLUDED.attempt_count,
                    next_retry_at = EXCLUDED.next_retry_at,
                    locked_until = EXCLUDED.locked_until,
                    locked_by = EXCLUDED.locked_by,
                    result = EXCLUDED.result,
                    error_details = EXCLUDED.error_details,
                    updated_at = EXCLUDED.updated_at,
                    completed_at = EXCLUDED.completed_at
                "#,
            )
            .bind(op.id)
            .bind(&op.tenant_id)
            .bind(&op.requesting_service)
            .bind(&op.provider)
            .bind(&op.operation_type)
            .bind(&op.business_reference)
            .bind(&op.idempotency_key)
            .bind(&op.request_fingerprint)
            .bind(op.correlation_id)
            .bind(op.causation_id)
            .bind(&op.external_reference)
            .bind(&status)
            .bind(op.attempt_count)
            .bind(op.max_attempts)
            .bind(op.next_retry_at)
            .bind(op.locked_until)
            .bind(&op.locked_by)
            .bind(&op.payload)
            .bind(&op.result)
            .bind(&op.error_details)
            .bind(op.created_at)
            .bind(op.updated_at)
            .bind(op.completed_at)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;

            Ok(())
        })
    }

    fn get_operation<'a>(
        &'a self,
        id: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<Option<ExternalOperation>, AppError>> + Send + 'a>>
    {
        Box::pin(async move {
            let row = sqlx::query(
                r#"
                SELECT id, tenant_id, requesting_service, provider, operation_type,
                       business_reference, idempotency_key, request_fingerprint,
                       correlation_id, causation_id, external_reference, status,
                       attempt_count, max_attempts, next_retry_at, locked_until,
                       locked_by, payload, result, error_details, created_at,
                       updated_at, completed_at
                FROM integration_operations
                WHERE id = $1
                "#,
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;

            match row {
                Some(r) => Ok(Some(map_op_row(&r)?)),
                None => Ok(None),
            }
        })
    }

    fn get_by_idempotency<'a>(
        &'a self,
        tenant_id: &'a str,
        requesting_service: &'a str,
        operation_type: &'a str,
        idempotency_key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<ExternalOperation>, AppError>> + Send + 'a>>
    {
        Box::pin(async move {
            let row = sqlx::query(
                r#"
                SELECT id, tenant_id, requesting_service, provider, operation_type,
                       business_reference, idempotency_key, request_fingerprint,
                       correlation_id, causation_id, external_reference, status,
                       attempt_count, max_attempts, next_retry_at, locked_until,
                       locked_by, payload, result, error_details, created_at,
                       updated_at, completed_at
                FROM integration_operations
                WHERE tenant_id = $1 AND requesting_service = $2 AND operation_type = $3 AND idempotency_key = $4
                "#,
            )
            .bind(tenant_id)
            .bind(requesting_service)
            .bind(operation_type)
            .bind(idempotency_key)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;

            match row {
                Some(r) => Ok(Some(map_op_row(&r)?)),
                None => Ok(None),
            }
        })
    }

    fn claim_pending_operations<'a>(
        &'a self,
        limit: i64,
        lease_duration: Duration,
        worker_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ExternalOperation>, AppError>> + Send + 'a>> {
        Box::pin(async move {
            let lease_until = Utc::now() + lease_duration;

            let rows = sqlx::query(
                r#"
                UPDATE integration_operations
                SET locked_until = $1,
                    locked_by = $2,
                    updated_at = NOW()
                WHERE id IN (
                    SELECT id FROM integration_operations
                    WHERE (
                        status = 'ACCEPTED' 
                        OR (status = 'FAILED_RETRYABLE' AND next_retry_at <= NOW())
                        OR status = 'PENDING'
                    )
                    AND (locked_until IS NULL OR locked_until < NOW())
                    ORDER BY created_at ASC
                    LIMIT $3
                    FOR UPDATE SKIP LOCKED
                )
                RETURNING id, tenant_id, requesting_service, provider, operation_type,
                          business_reference, idempotency_key, request_fingerprint,
                          correlation_id, causation_id, external_reference, status,
                          attempt_count, max_attempts, next_retry_at, locked_until,
                          locked_by, payload, result, error_details, created_at,
                          updated_at, completed_at
                "#,
            )
            .bind(lease_until)
            .bind(worker_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;

            let mut ops = Vec::with_capacity(rows.len());
            for r in &rows {
                ops.push(map_op_row(r)?);
            }
            Ok(ops)
        })
    }

    fn record_attempt<'a>(
        &'a self,
        attempt: &'a OperationAttempt,
    ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>> {
        Box::pin(async move {
            let status = serde_json::to_string(&attempt.status)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string();
            let http_status = attempt.http_status.map(|s| s as i32);
            let latency_ms = attempt.latency_ms.map(|l| l as i64);

            sqlx::query(
                r#"
                INSERT INTO integration_operation_attempts (
                    id, operation_id, attempt_number, dispatched_at, completed_at,
                    status, http_status, error_code, error_message, latency_ms
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
            )
            .bind(attempt.id)
            .bind(attempt.operation_id)
            .bind(attempt.attempt_number)
            .bind(attempt.dispatched_at)
            .bind(attempt.completed_at)
            .bind(&status)
            .bind(http_status)
            .bind(&attempt.error_code)
            .bind(&attempt.error_message)
            .bind(latency_ms)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;

            Ok(())
        })
    }

    fn get_attempts<'a>(
        &'a self,
        operation_id: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<OperationAttempt>, AppError>> + Send + 'a>> {
        Box::pin(async move {
            let rows = sqlx::query(
                r#"
                SELECT id, operation_id, attempt_number, dispatched_at, completed_at,
                       status, http_status, error_code, error_message, latency_ms
                FROM integration_operation_attempts
                WHERE operation_id = $1
                ORDER BY attempt_number ASC
                "#,
            )
            .bind(operation_id)
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;

            let mut attempts = Vec::with_capacity(rows.len());
            for r in &rows {
                let status_str: String = r.try_get("status").map_err(db_err)?;
                let status: AttemptStatus =
                    serde_json::from_value(serde_json::Value::String(status_str))
                        .unwrap_or(AttemptStatus::TerminalError);
                let http_status: Option<i32> = r.try_get("http_status").map_err(db_err)?;
                let latency_ms: Option<i64> = r.try_get("latency_ms").map_err(db_err)?;

                attempts.push(OperationAttempt {
                    id: r.try_get("id").map_err(db_err)?,
                    operation_id: r.try_get("operation_id").map_err(db_err)?,
                    attempt_number: r.try_get("attempt_number").map_err(db_err)?,
                    dispatched_at: r.try_get("dispatched_at").map_err(db_err)?,
                    completed_at: r.try_get("completed_at").map_err(db_err)?,
                    status,
                    http_status: http_status.map(|s| s as u16),
                    error_code: r.try_get("error_code").map_err(db_err)?,
                    error_message: r.try_get("error_message").map_err(db_err)?,
                    latency_ms: latency_ms.map(|l| l as u64),
                });
            }
            Ok(attempts)
        })
    }

    fn save_callback<'a>(
        &'a self,
        cb: &'a CallbackReceipt,
    ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>> {
        Box::pin(async move {
            let status = serde_json::to_string(&cb.status)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string();

            sqlx::query(
                r#"
                INSERT INTO integration_callbacks (
                    id, provider, callback_ref, signature, payload, status,
                    operation_id, received_at, processed_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
            )
            .bind(cb.id)
            .bind(&cb.provider)
            .bind(&cb.callback_ref)
            .bind(&cb.signature)
            .bind(&cb.payload)
            .bind(&status)
            .bind(cb.operation_id)
            .bind(cb.received_at)
            .bind(cb.processed_at)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;

            Ok(())
        })
    }

    fn save_reconciliation<'a>(
        &'a self,
        rec: &'a ReconciliationRecord,
    ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>> {
        Box::pin(async move {
            let outcome = serde_json::to_string(&rec.outcome)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string();

            sqlx::query(
                r#"
                INSERT INTO integration_reconciliations (
                    id, operation_id, checked_at, provider_status, outcome, details
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(rec.id)
            .bind(rec.operation_id)
            .bind(rec.checked_at)
            .bind(&rec.provider_status)
            .bind(&outcome)
            .bind(&rec.details)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;

            Ok(())
        })
    }

    fn list_providers<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<IntegrationProvider>, AppError>> + Send + 'a>> {
        Box::pin(async move {
            let rows = sqlx::query(
                r#"
                SELECT id, name, kind, status, base_url, timeout_ms
                FROM integration_providers
                ORDER BY name ASC
                "#,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;

            let mut providers = Vec::with_capacity(rows.len());
            for r in &rows {
                let kind_str: String = r.try_get("kind").map_err(db_err)?;
                let status_str: String = r.try_get("status").map_err(db_err)?;
                let timeout_ms: i64 = r.try_get("timeout_ms").map_err(db_err)?;

                let kind: ProviderKind =
                    serde_json::from_value(serde_json::Value::String(kind_str))
                        .unwrap_or(ProviderKind::Generic);
                let status: ProviderStatus =
                    serde_json::from_value(serde_json::Value::String(status_str))
                        .unwrap_or(ProviderStatus::Suspended);

                providers.push(IntegrationProvider {
                    id: r.try_get("id").map_err(db_err)?,
                    name: r.try_get("name").map_err(db_err)?,
                    kind,
                    status,
                    base_url: r.try_get("base_url").map_err(db_err)?,
                    timeout_ms: timeout_ms as u64,
                });
            }
            Ok(providers)
        })
    }

    fn get_provider<'a>(
        &'a self,
        id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<IntegrationProvider>, AppError>> + Send + 'a>>
    {
        Box::pin(async move {
            let row = sqlx::query(
                r#"
                SELECT id, name, kind, status, base_url, timeout_ms
                FROM integration_providers
                WHERE id = $1
                "#,
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;

            match row {
                Some(r) => {
                    let kind_str: String = r.try_get("kind").map_err(db_err)?;
                    let status_str: String = r.try_get("status").map_err(db_err)?;
                    let timeout_ms: i64 = r.try_get("timeout_ms").map_err(db_err)?;

                    let kind: ProviderKind =
                        serde_json::from_value(serde_json::Value::String(kind_str))
                            .unwrap_or(ProviderKind::Generic);
                    let status: ProviderStatus =
                        serde_json::from_value(serde_json::Value::String(status_str))
                            .unwrap_or(ProviderStatus::Suspended);

                    Ok(Some(IntegrationProvider {
                        id: r.try_get("id").map_err(db_err)?,
                        name: r.try_get("name").map_err(db_err)?,
                        kind,
                        status,
                        base_url: r.try_get("base_url").map_err(db_err)?,
                        timeout_ms: timeout_ms as u64,
                    }))
                }
                None => Ok(None),
            }
        })
    }

    fn list_operations<'a>(
        &'a self,
        tenant_id: &'a str,
        limit: i64,
        offset: i64,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ExternalOperation>, AppError>> + Send + 'a>> {
        Box::pin(async move {
            let rows = sqlx::query(
                r#"
                SELECT id, tenant_id, requesting_service, provider, operation_type,
                       business_reference, idempotency_key, request_fingerprint,
                       correlation_id, causation_id, external_reference, status,
                       attempt_count, max_attempts, next_retry_at, locked_until,
                       locked_by, payload, result, error_details, created_at,
                       updated_at, completed_at
                FROM integration_operations
                WHERE tenant_id = $1
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(tenant_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;

            let mut ops = Vec::with_capacity(rows.len());
            for r in &rows {
                ops.push(map_op_row(r)?);
            }
            Ok(ops)
        })
    }
}
