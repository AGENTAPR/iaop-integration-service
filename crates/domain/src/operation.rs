//! External Operation aggregate and state machine.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OperationStatus {
    Accepted,
    Pending,
    Running,
    WaitingExternalConfirmation,
    Succeeded,
    FailedRetryable,
    FailedFinal,
    RequiresReview,
    Cancelled,
}

impl OperationStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Succeeded | Self::FailedFinal | Self::Cancelled)
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::FailedRetryable | Self::Pending)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalOperation {
    pub id: Uuid,
    pub tenant_id: String,
    pub requesting_service: String,
    pub provider: String,
    pub operation_type: String,
    pub business_reference: String,
    pub idempotency_key: String,
    pub request_fingerprint: String,
    pub correlation_id: Option<Uuid>,
    pub causation_id: Option<Uuid>,
    pub external_reference: Option<String>,
    pub status: OperationStatus,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub locked_until: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
    pub payload: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub error_details: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl ExternalOperation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Uuid,
        tenant_id: String,
        requesting_service: String,
        provider: String,
        operation_type: String,
        business_reference: String,
        idempotency_key: String,
        request_fingerprint: String,
        payload: serde_json::Value,
        correlation_id: Option<Uuid>,
        causation_id: Option<Uuid>,
        max_attempts: i32,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            tenant_id,
            requesting_service,
            provider,
            operation_type,
            business_reference,
            idempotency_key,
            request_fingerprint,
            correlation_id,
            causation_id,
            external_reference: None,
            status: OperationStatus::Accepted,
            attempt_count: 0,
            max_attempts,
            next_retry_at: Some(now),
            locked_until: None,
            locked_by: None,
            payload,
            result: None,
            error_details: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }

    pub fn transition_to(&mut self, next: OperationStatus) -> Result<(), DomainError> {
        let valid = match (self.status, next) {
            // Self transitions are no-op rejected
            (s1, s2) if s1 == s2 => false,
            // From Accepted
            (OperationStatus::Accepted, OperationStatus::Running) => true,
            (OperationStatus::Accepted, OperationStatus::Pending) => true,
            (OperationStatus::Accepted, OperationStatus::Cancelled) => true,
            // From Running
            (OperationStatus::Running, OperationStatus::Succeeded) => true,
            (OperationStatus::Running, OperationStatus::WaitingExternalConfirmation) => true,
            (OperationStatus::Running, OperationStatus::FailedRetryable) => true,
            (OperationStatus::Running, OperationStatus::FailedFinal) => true,
            (OperationStatus::Running, OperationStatus::RequiresReview) => true,
            // From WaitingExternalConfirmation (async callback or polling)
            (OperationStatus::WaitingExternalConfirmation, OperationStatus::Succeeded) => true,
            (OperationStatus::WaitingExternalConfirmation, OperationStatus::FailedFinal) => true,
            (OperationStatus::WaitingExternalConfirmation, OperationStatus::RequiresReview) => true,
            // From FailedRetryable
            (OperationStatus::FailedRetryable, OperationStatus::Pending) => true,
            (OperationStatus::FailedRetryable, OperationStatus::Running) => true,
            (OperationStatus::FailedRetryable, OperationStatus::FailedFinal) => true,
            (OperationStatus::FailedRetryable, OperationStatus::RequiresReview) => true,
            // From Pending
            (OperationStatus::Pending, OperationStatus::Running) => true,
            (OperationStatus::Pending, OperationStatus::Cancelled) => true,
            // From RequiresReview (manual resolution)
            (OperationStatus::RequiresReview, OperationStatus::Succeeded) => true,
            (OperationStatus::RequiresReview, OperationStatus::FailedFinal) => true,
            (OperationStatus::RequiresReview, OperationStatus::Pending) => true,
            // Terminal states cannot transition
            (OperationStatus::Succeeded, _) => false,
            (OperationStatus::FailedFinal, _) => false,
            (OperationStatus::Cancelled, _) => false,
            _ => false,
        };

        if !valid {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: format!("{:?}", next),
            });
        }

        self.status = next;
        self.updated_at = Utc::now();
        if next.is_terminal() {
            self.completed_at = Some(self.updated_at);
        }
        Ok(())
    }

    pub fn record_success(
        &mut self,
        result: serde_json::Value,
        external_ref: Option<String>,
    ) -> Result<(), DomainError> {
        self.transition_to(OperationStatus::Succeeded)?;
        self.result = Some(result);
        if let Some(r) = external_ref {
            self.external_reference = Some(r);
        }
        self.locked_until = None;
        self.locked_by = None;
        Ok(())
    }

    pub fn record_failure(&mut self, error: String, retryable: bool) -> Result<(), DomainError> {
        self.error_details = Some(error);
        self.attempt_count += 1;
        self.locked_until = None;
        self.locked_by = None;

        if retryable && self.attempt_count < self.max_attempts {
            self.transition_to(OperationStatus::FailedRetryable)?;
            // Exponential backoff: base 2s * 2^(attempts-1) up to 1 hour
            let backoff_secs = 2u64.saturating_pow(self.attempt_count.min(10) as u32);
            self.next_retry_at = Some(Utc::now() + chrono::Duration::seconds(backoff_secs as i64));
        } else {
            self.transition_to(OperationStatus::FailedFinal)?;
            self.next_retry_at = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_transitions_to_running_and_succeeded() {
        let mut op = ExternalOperation::new(
            Uuid::now_v7(),
            "tenant-1".into(),
            "iaop-agent-service".into(),
            "dukun-id".into(),
            "identity.verify".into(),
            "agt-123".into(),
            "idem-key-1".into(),
            "fingerprint-1".into(),
            serde_json::json!({}),
            None,
            None,
            3,
        );

        assert_eq!(op.status, OperationStatus::Accepted);
        assert!(op.transition_to(OperationStatus::Running).is_ok());
        assert!(
            op.record_success(
                serde_json::json!({"verified": true}),
                Some("ext-ref-1".into())
            )
            .is_ok()
        );
        assert_eq!(op.status, OperationStatus::Succeeded);
        assert!(op.completed_at.is_some());
        // Terminal state cannot transition
        assert!(op.transition_to(OperationStatus::Running).is_err());
    }

    #[test]
    fn retryable_failure_schedules_next_retry() {
        let mut op = ExternalOperation::new(
            Uuid::now_v7(),
            "tenant-1".into(),
            "iaop-agent-service".into(),
            "dukun-id".into(),
            "identity.verify".into(),
            "agt-123".into(),
            "idem-key-1".into(),
            "fingerprint-1".into(),
            serde_json::json!({}),
            None,
            None,
            3,
        );
        op.transition_to(OperationStatus::Running).unwrap();
        op.record_failure("timeout".into(), true).unwrap();

        assert_eq!(op.status, OperationStatus::FailedRetryable);
        assert_eq!(op.attempt_count, 1);
        assert!(op.next_retry_at.is_some());
    }
}
