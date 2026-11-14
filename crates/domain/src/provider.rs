//! Integration Provider aggregate and capabilities.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderKind {
    IdentityVerification,
    CredentialVerification,
    CoreInsurance,
    DocumentVerification,
    ESignature,
    Notification,
    Payment,
    Generic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderStatus {
    Active,
    Suspended,
    Deprecated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationProvider {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub status: ProviderStatus,
    pub base_url: String,
    pub timeout_ms: u64,
}

impl IntegrationProvider {
    pub fn is_active(&self) -> bool {
        self.status == ProviderStatus::Active
    }
}
