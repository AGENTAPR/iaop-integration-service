use std::sync::Arc;

use iaop_integration_service_infrastructure::callback_verifier::CallbackVerifier;
use iaop_integration_service_infrastructure::config::AppConfig;
use iaop_integration_service_infrastructure::connectors::ConnectorDispatcherImpl;
use iaop_integration_service_infrastructure::store::PgIntegrationStore;
use sqlx::PgPool;

/// Shared application state injected into handlers via `State`.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    /// `None` when DATABASE_URL is not configured; `/health/ready`
    /// reports accordingly instead of failing.
    pub db: Option<PgPool>,
    pub store: Option<Arc<PgIntegrationStore>>,
    pub dispatcher: Arc<ConnectorDispatcherImpl>,
    pub verifier: Arc<CallbackVerifier>,
}
