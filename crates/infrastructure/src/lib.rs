#![forbid(unsafe_code)]
//! Infrastructure layer for Integration Platform.
//!
//! Adapters implementing application ports: configuration, PostgreSQL
//! persistence (pool, migrations, outbox/inbox), messaging helpers,
//! and security utilities. Depends on `application` and `domain`.

pub mod callback_verifier;
pub mod config;
pub mod connectors;
pub mod messaging;
pub mod outbox;
pub mod persistence;
pub mod security;
pub mod store;
pub mod transport;
pub mod worker;

pub use callback_verifier::CallbackVerifier;
pub use connectors::{ConnectorDispatcherImpl, HttpConnector, MockConnector};
pub use store::PgIntegrationStore;
pub use transport::{ConfiguredTransport, KafkaTransport, LogTransport};
