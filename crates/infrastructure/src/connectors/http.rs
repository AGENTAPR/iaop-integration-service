//! Real HTTP Connector executing external operations with timeout and SSRF guard.

use std::time::Duration;

use iaop_integration_service_domain::{ConnectorError, ExternalOperation, IntegrationProvider};
use reqwest::{Client, StatusCode};

use crate::connectors::ssrf::SsrfGuard;

pub struct HttpConnector {
    client: Client,
    is_production: bool,
}

impl HttpConnector {
    pub fn new(is_production: bool) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        Self {
            client,
            is_production,
        }
    }

    pub async fn execute(
        &self,
        provider: &IntegrationProvider,
        op: &ExternalOperation,
    ) -> Result<serde_json::Value, ConnectorError> {
        let target_url = format!(
            "{}/operations/{}",
            provider.base_url.trim_end_matches('/'),
            op.operation_type
        );

        SsrfGuard::validate_url(&target_url, self.is_production).map_err(|e| {
            ConnectorError::TerminalValidation(format!("SSRF guard rejection: {e}"))
        })?;

        let timeout = Duration::from_millis(provider.timeout_ms.max(1000));

        let res = self
            .client
            .post(&target_url)
            .timeout(timeout)
            .header("X-Operation-ID", op.id.to_string())
            .header("X-Idempotency-Key", &op.idempotency_key)
            .json(&op.payload)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ConnectorError::RetryableNetwork(format!("upstream request timeout: {e}"))
                } else if e.is_connect() {
                    ConnectorError::RetryableNetwork(format!("upstream connect error: {e}"))
                } else {
                    ConnectorError::UnknownOutcome(format!("upstream transport error: {e}"))
                }
            })?;

        let status = res.status();
        if status.is_success() {
            let body = res.json::<serde_json::Value>().await.map_err(|e| {
                ConnectorError::UnknownOutcome(format!("failed to parse provider JSON: {e}"))
            })?;
            Ok(body)
        } else if status == StatusCode::TOO_MANY_REQUESTS {
            let retry_after = res
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(30);
            Err(ConnectorError::RetryableRateLimited {
                retry_after_secs: retry_after,
            })
        } else if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            Err(ConnectorError::TerminalAuthentication(format!(
                "provider rejected authentication with HTTP {status}"
            )))
        } else if status.is_client_error() {
            let err_text = res.text().await.unwrap_or_default();
            Err(ConnectorError::TerminalValidation(format!(
                "provider validation error ({status}): {err_text}"
            )))
        } else {
            let err_text = res.text().await.unwrap_or_default();
            Err(ConnectorError::RetryableNetwork(format!(
                "provider server error ({status}): {err_text}"
            )))
        }
    }
}
