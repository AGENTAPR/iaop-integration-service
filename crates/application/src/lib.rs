#![forbid(unsafe_code)]
//! Application layer for Integration Platform.
//!
//! Contains use cases (commands and queries) and the ports they depend on.
//! Depends only on the `domain` crate — never on Axum, SQLx, or other
//! infrastructure. Adapters in `infrastructure` implement the ports.

pub mod commands;
pub mod error;
pub mod ports;
pub mod queries;
pub mod service_info;

pub use error::AppError;
