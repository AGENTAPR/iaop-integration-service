#![forbid(unsafe_code)]
//! Domain layer for Integration Platform.
//!
//! This crate contains the pure domain model: entities, value objects,
//! aggregates, domain events, and domain errors. It MUST NOT depend on
//! Axum, SQLx, HTTP clients, Keycloak, message brokers, or any other
//! infrastructure concern.
//!
//! Dependency rule: `domain` has no dependencies on sibling crates.

pub mod attempt;
pub mod callback;
pub mod connector;
pub mod entity_id;
pub mod error;
pub mod operation;
pub mod provider;
pub mod reconciliation;

pub use attempt::{AttemptStatus, OperationAttempt};
pub use callback::{CallbackReceipt, CallbackStatus};
pub use connector::{ConnectorError, ConnectorOperation};
pub use entity_id::EntityId;
pub use error::DomainError;
pub use operation::{ExternalOperation, OperationStatus};
pub use provider::{IntegrationProvider, ProviderKind, ProviderStatus};
pub use reconciliation::{ReconciliationOutcome, ReconciliationRecord};
