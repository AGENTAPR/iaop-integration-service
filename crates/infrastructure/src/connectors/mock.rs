//! Mock connector for local testing and unconfigured providers.

use iaop_integration_service_domain::{ConnectorError, ExternalOperation, IntegrationProvider};

pub struct MockConnector;

impl MockConnector {
    pub async fn execute(
        &self,
        provider: &IntegrationProvider,
        op: &ExternalOperation,
    ) -> Result<serde_json::Value, ConnectorError> {
        // Check for test simulation triggers in payload
        if let Some(err_kind) = op.payload.get("__simulate_error").and_then(|v| v.as_str()) {
            match err_kind {
                "timeout" => {
                    return Err(ConnectorError::RetryableNetwork("simulated timeout".into()));
                }
                "rate_limit" => {
                    return Err(ConnectorError::RetryableRateLimited {
                        retry_after_secs: 15,
                    });
                }
                "validation" => {
                    return Err(ConnectorError::TerminalValidation(
                        "simulated validation failure".into(),
                    ));
                }
                "auth" => {
                    return Err(ConnectorError::TerminalAuthentication(
                        "simulated auth rejection".into(),
                    ));
                }
                _ => {
                    return Err(ConnectorError::UnknownOutcome(
                        "simulated unknown outcome".into(),
                    ));
                }
            }
        }

        Ok(serde_json::json!({
            "provider": provider.id,
            "operation_id": op.id,
            "status": "COMPLETED",
            "mock": true,
            "echo": op.payload
        }))
    }
}
