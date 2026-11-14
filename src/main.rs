//! Composition root for Integration Platform.
//!
//! Wires configuration, persistence, connectors, the background outbox publisher,
//! the background operation dispatcher worker, and the HTTP router, then serves
//! with graceful shutdown on SIGINT/SIGTERM.

use std::sync::Arc;

use anyhow::Context;
use iaop_integration_service_api::{AppState, build_router};
use iaop_integration_service_infrastructure::callback_verifier::CallbackVerifier;
use iaop_integration_service_infrastructure::config::AppConfig;
use iaop_integration_service_infrastructure::connectors::ConnectorDispatcherImpl;
use iaop_integration_service_infrastructure::outbox;
use iaop_integration_service_infrastructure::persistence;
use iaop_integration_service_infrastructure::store::PgIntegrationStore;
use iaop_integration_service_infrastructure::transport::{
    ConfiguredTransport, KafkaTransport, LogTransport,
};
use iaop_integration_service_infrastructure::worker;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::from_env().context("failed to load configuration")?;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .json()
        .init();

    tracing::info!(config = %config.redacted_summary(), "starting service");

    let db = match &config.database_url {
        Some(url) => {
            let pool = persistence::connect_pool(url, config.database_max_connections)
                .await
                .context("failed to connect to database")?;
            persistence::run_migrations(&pool)
                .await
                .context("failed to run migrations")?;
            tracing::info!("database migrations applied");
            Some(pool)
        }
        None => {
            tracing::warn!("DATABASE_URL not set; running without persistence");
            None
        }
    };

    let store = db
        .as_ref()
        .map(|pool| Arc::new(PgIntegrationStore::new(pool.clone())));
    let dispatcher = Arc::new(ConnectorDispatcherImpl::new(
        &config.connector_mode,
        config.is_production,
    ));
    let verifier = Arc::new(CallbackVerifier::new(config.callback_secret.clone()));

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // ---- outbox publisher (only with DB) --------------------------------
    if let Some(pool) = &db {
        let publisher_pool = pool.clone();
        let transport = Arc::new(match config.event_transport.as_str() {
            "kafka" => {
                let brokers = config
                    .kafka_brokers
                    .clone()
                    .unwrap_or_else(|| "127.0.0.1:9092".to_string());
                ConfiguredTransport::Kafka(KafkaTransport::new(
                    brokers,
                    config.kafka_topic_prefix.clone(),
                ))
            }
            _ => ConfiguredTransport::Log(LogTransport),
        });
        let poll = config.outbox_poll_interval;
        let batch = config.outbox_batch_size;
        let max_attempts = config.outbox_max_attempts;
        let rx = shutdown_rx.clone();

        tokio::spawn(async move {
            outbox::run_publisher(publisher_pool, transport, poll, batch, max_attempts, rx).await;
        });
        tracing::info!("outbox publisher started");
    }

    // ---- operation worker (only with DB) --------------------------------
    if let Some(s) = &store {
        let op_store = s.clone();
        let op_dispatcher = dispatcher.clone();
        let poll = std::time::Duration::from_millis(1000);
        let batch = 25;
        let rx = shutdown_rx.clone();

        tokio::spawn(async move {
            worker::run_operation_worker(op_store, op_dispatcher, poll, batch, rx).await;
        });
        tracing::info!("operation worker started");
    }

    let state = AppState {
        config: Arc::new(config.clone()),
        db,
        store,
        dispatcher,
        verifier,
    };
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr)
        .await
        .with_context(|| format!("failed to bind {}", config.bind_addr))?;
    tracing::info!(addr = %config.bind_addr, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            let _ = shutdown_tx.send(true);
        })
        .await
        .context("server error")?;

    tracing::info!("shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
