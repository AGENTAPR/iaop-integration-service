//! Transactional outbox publisher with durable distributed leasing.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use iaop_integration_service_application::ports::IntegrationEvent;
use sqlx::{PgPool, Row};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::transport::ConfiguredTransport;

pub async fn run_publisher(
    pool: PgPool,
    transport: Arc<ConfiguredTransport>,
    poll_interval: Duration,
    batch_size: i64,
    max_attempts: i32,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let worker_id = format!("worker-{}", Uuid::now_v7().simple());
    info!(worker_id = %worker_id, "starting outbox publisher with durable leasing");

    loop {
        tokio::select! {
            _ = tokio::time::sleep(poll_interval) => {
                if let Err(e) = publish_batch(&pool, &transport, batch_size, max_attempts, &worker_id).await {
                    error!(error = %e, "error in outbox publisher iteration");
                }
            }
            _ = shutdown.changed() => {
                info!("outbox publisher received shutdown signal");
                break;
            }
        }
    }
}

async fn publish_batch(
    pool: &PgPool,
    transport: &ConfiguredTransport,
    batch_size: i64,
    max_attempts: i32,
    worker_id: &str,
) -> Result<(), sqlx::Error> {
    let lease_until = Utc::now() + chrono::Duration::seconds(30);

    let rows = sqlx::query(
        r#"
        UPDATE outbox_events
        SET locked_until = $1,
            locked_by = $2,
            publish_attempts = publish_attempts + 1
        WHERE id IN (
            SELECT id FROM outbox_events
            WHERE published_at IS NULL
              AND publish_attempts < $3
              AND (locked_until IS NULL OR locked_until < NOW())
            ORDER BY created_at ASC
            LIMIT $4
            FOR UPDATE SKIP LOCKED
        )
        RETURNING id, event_id, event_type, event_version, payload, occurred_at
        "#,
    )
    .bind(lease_until)
    .bind(worker_id)
    .bind(max_attempts)
    .bind(batch_size)
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok(());
    }

    for row in rows {
        let id: Uuid = row.try_get("id")?;
        let event_id: Uuid = row.try_get("event_id")?;
        let event_type: String = row.try_get("event_type")?;
        let event_version: i32 = row.try_get("event_version")?;
        let payload: serde_json::Value = row.try_get("payload")?;
        let occurred_at: DateTime<Utc> = row.try_get("occurred_at")?;

        let event = IntegrationEvent {
            event_id,
            event_type: event_type.clone(),
            event_version: event_version as u32,
            source: "iaop-integration-service".into(),
            aggregate_id: None,
            aggregate_version: None,
            occurred_at,
            correlation_id: None,
            causation_id: None,
            payload,
        };

        match transport.deliver(&event).await {
            Ok(_) => {
                sqlx::query(
                    r#"
                    UPDATE outbox_events
                    SET published_at = NOW(),
                        locked_until = NULL,
                        locked_by = NULL
                    WHERE id = $1
                    "#,
                )
                .bind(id)
                .execute(pool)
                .await?;
            }
            Err(e) => {
                warn!(error = %e, event_id = %event_id, "failed to deliver outbox event");
                sqlx::query(
                    r#"
                    UPDATE outbox_events
                    SET last_error = $1,
                        locked_until = NULL,
                        locked_by = NULL
                    WHERE id = $2
                    "#,
                )
                .bind(&e)
                .bind(id)
                .execute(pool)
                .await?;
            }
        }
    }

    Ok(())
}
