//! Background dispatcher worker claiming and executing durable integration operations.

use std::sync::Arc;
use std::time::Duration;

use iaop_integration_service_application::commands::dispatch_operation;
use iaop_integration_service_application::ports::{ConnectorDispatcher, IntegrationStore};
use tracing::{error, info};
use uuid::Uuid;

pub async fn run_operation_worker<S, D>(
    store: Arc<S>,
    dispatcher: Arc<D>,
    poll_interval: Duration,
    batch_size: i64,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) where
    S: IntegrationStore + 'static,
    D: ConnectorDispatcher + 'static,
{
    let worker_id = format!("op-worker-{}", Uuid::now_v7().simple());
    info!(worker_id = %worker_id, "starting integration operation worker");

    loop {
        tokio::select! {
            _ = tokio::time::sleep(poll_interval) => {
                let lease_duration = chrono::Duration::seconds(60);
                match store.claim_pending_operations(batch_size, lease_duration, &worker_id).await {
                    Ok(ops) => {
                        for op in ops {
                            let s = store.clone();
                            let d = dispatcher.clone();
                            tokio::spawn(async move {
                                if let Err(e) = dispatch_operation(&*s, &*d, op).await {
                                    error!(error = %e, "error dispatching integration operation");
                                }
                            });
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "error claiming pending integration operations");
                    }
                }
            }
            _ = shutdown.changed() => {
                info!("operation worker received shutdown signal");
                break;
            }
        }
    }
}
