//! Connector Dispatcher implementation.

use std::future::Future;
use std::pin::Pin;

use iaop_integration_service_application::ports::ConnectorDispatcher;
use iaop_integration_service_domain::{ConnectorError, ExternalOperation, IntegrationProvider};

use super::http::HttpConnector;
use super::mock::MockConnector;

pub enum ConnectorDispatcherImpl {
    Http(HttpConnector),
    Mock(MockConnector),
    Hybrid {
        http: HttpConnector,
        mock: MockConnector,
    },
}

impl ConnectorDispatcherImpl {
    pub fn new(mode: &str, is_production: bool) -> Self {
        match mode {
            "mock" => Self::Mock(MockConnector),
            "http" => Self::Http(HttpConnector::new(is_production)),
            _ => Self::Hybrid {
                http: HttpConnector::new(is_production),
                mock: MockConnector,
            },
        }
    }
}

impl ConnectorDispatcher for ConnectorDispatcherImpl {
    fn dispatch<'a>(
        &'a self,
        provider: &'a IntegrationProvider,
        op: &'a ExternalOperation,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, ConnectorError>> + Send + 'a>> {
        Box::pin(async move {
            match self {
                Self::Mock(m) => m.execute(provider, op).await,
                Self::Http(h) => h.execute(provider, op).await,
                Self::Hybrid { http, mock } => {
                    if provider.id == "mock-provider"
                        || provider.base_url.starts_with("http://127.0.0.1:0")
                    {
                        mock.execute(provider, op).await
                    } else {
                        http.execute(provider, op).await
                    }
                }
            }
        })
    }
}
