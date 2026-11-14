//! Smoke integration test: the router serves health and info endpoints
//! without a database.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use iaop_integration_service_api::{AppState, build_router};
use iaop_integration_service_infrastructure::callback_verifier::CallbackVerifier;
use iaop_integration_service_infrastructure::config::AppConfig;
use iaop_integration_service_infrastructure::connectors::ConnectorDispatcherImpl;
use std::sync::Arc;

fn test_state() -> AppState {
    AppState {
        config: Arc::new(AppConfig::from_env().expect("test config must load")),
        db: None,
        store: None,
        dispatcher: Arc::new(ConnectorDispatcherImpl::new("mock", false)),
        verifier: Arc::new(CallbackVerifier::new("test-secret".to_string())),
    }
}

#[tokio::test]
async fn health_live_returns_ok() {
    let app = build_router(test_state());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health/live")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn system_info_returns_metadata() {
    let app = build_router(test_state());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/system/info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn openapi_spec_is_valid_json() {
    let app = build_router(test_state());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/docs/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("must be valid JSON");
    println!("---BEGIN_OPENAPI---");
    println!("{}", serde_json::to_string_pretty(&json).unwrap());
    println!("---END_OPENAPI---");
}
