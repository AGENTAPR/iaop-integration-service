//! HMAC-SHA256 callback signature verification.

use std::future::Future;
use std::pin::Pin;

use hmac::{Hmac, Mac};
use iaop_integration_service_application::error::AppError;
use iaop_integration_service_application::ports::CallbackVerifierPort;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub struct CallbackVerifier {
    default_secret: String,
}

impl CallbackVerifier {
    pub fn new(default_secret: String) -> Self {
        Self { default_secret }
    }

    pub fn compute_signature(&self, secret: &str, payload: &[u8]) -> String {
        let mut mac =
            HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take any key size");
        mac.update(payload);
        format!("{:x}", mac.finalize().into_bytes())
    }
}

impl CallbackVerifierPort for CallbackVerifier {
    fn verify<'a>(
        &'a self,
        _provider: &'a str,
        signature: Option<&'a str>,
        payload: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>> {
        Box::pin(async move {
            let sig = signature.ok_or_else(|| {
                AppError::Unauthorized("missing callback signature header".into())
            })?;

            let expected = self.compute_signature(&self.default_secret, payload);

            // Constant-time compare or direct hex string compare
            if sig.trim() != expected {
                return Err(AppError::Unauthorized(
                    "invalid callback signature: HMAC-SHA256 verification failed".into(),
                ));
            }

            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn verifier_validates_correct_hmac() {
        let verifier = CallbackVerifier::new("secret-key-123".into());
        let payload = b"{\"event\":\"identity.verified\",\"status\":\"SUCCESS\"}";
        let sig = verifier.compute_signature("secret-key-123", payload);

        assert!(
            verifier
                .verify("dukcapil", Some(&sig), payload)
                .await
                .is_ok()
        );
        assert!(
            verifier
                .verify("dukcapil", Some("invalid_signature"), payload)
                .await
                .is_err()
        );
        assert!(verifier.verify("dukcapil", None, payload).await.is_err());
    }
}
