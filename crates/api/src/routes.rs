//! HTTP route handlers for Integration Service.

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use iaop_integration_service_application::commands;
use iaop_integration_service_application::error::AppError;
use iaop_integration_service_application::queries;
use iaop_integration_service_application::service_info::GetServiceInfoQuery;
use iaop_integration_service_infrastructure::persistence;
use serde::Deserialize;
use utoipa::OpenApi;
use uuid::Uuid;

use crate::dto::*;
use crate::middleware;
use crate::openapi::ApiDoc;
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/api/v1/system/info", get(system_info))
        .route("/api/docs/openapi.json", get(openapi_json))
        .route(
            "/api/v1/integration/operations",
            post(submit_operation).get(list_operations),
        )
        .route("/api/v1/integration/operations/{id}", get(get_operation))
        .route(
            "/api/v1/integration/operations/{id}/attempts",
            get(get_attempts),
        )
        .route(
            "/api/v1/integration/operations/{id}/retry",
            post(retry_operation),
        )
        .route(
            "/api/v1/integration/operations/{id}/reconcile",
            post(reconcile_operation),
        )
        .route(
            "/api/v1/integration/callbacks/{provider}",
            post(receive_callback),
        )
        .route("/api/v1/integration/providers", get(list_providers))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::request_context,
        ))
        .with_state(state)
}

fn error_to_response(err: AppError) -> Response {
    let (status, msg) = match err {
        AppError::NotFound(m) => (StatusCode::NOT_FOUND, m),
        AppError::Validation(m) => (StatusCode::BAD_REQUEST, m),
        AppError::Conflict(m) => (StatusCode::CONFLICT, m),
        AppError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m),
        AppError::ProviderUnavailable(m) => (StatusCode::SERVICE_UNAVAILABLE, m),
        AppError::Domain(d) => (StatusCode::UNPROCESSABLE_ENTITY, d.to_string()),
        AppError::Infra(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
    };
    (status, Json(serde_json::json!({"error": msg}))).into_response()
}

// ---- Operations ----

async fn submit_operation(
    State(state): State<AppState>,
    Json(body): Json<SubmitOperationDto>,
) -> Response {
    let store = match &state.store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "persistence not configured",
            )
                .into_response();
        }
    };

    let req = commands::SubmitOperationRequest {
        tenant_id: body.tenant_id,
        requesting_service: body.requesting_service,
        provider: body.provider,
        operation_type: body.operation_type,
        business_reference: body.business_reference,
        idempotency_key: body.idempotency_key,
        payload: body.payload,
        correlation_id: body.correlation_id,
        causation_id: body.causation_id,
        max_attempts: body.max_attempts,
    };

    match commands::submit_operation(&**store, req).await {
        Ok(op) => (StatusCode::ACCEPTED, Json(OperationResponseDto::from(op))).into_response(),
        Err(e) => error_to_response(e),
    }
}

async fn get_operation(State(state): State<AppState>, Path(id): Path<Uuid>) -> Response {
    let store = match &state.store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "persistence not configured",
            )
                .into_response();
        }
    };

    match queries::get_operation(&**store, id).await {
        Ok(Some(op)) => (StatusCode::OK, Json(OperationResponseDto::from(op))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "operation not found"})),
        )
            .into_response(),
        Err(e) => error_to_response(e),
    }
}

async fn get_attempts(State(state): State<AppState>, Path(id): Path<Uuid>) -> Response {
    let store = match &state.store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "persistence not configured",
            )
                .into_response();
        }
    };

    match queries::get_operation_attempts(&**store, id).await {
        Ok(attempts) => {
            let dtos: Vec<AttemptResponseDto> =
                attempts.into_iter().map(AttemptResponseDto::from).collect();
            (StatusCode::OK, Json(dtos)).into_response()
        }
        Err(e) => error_to_response(e),
    }
}

async fn retry_operation(State(state): State<AppState>, Path(id): Path<Uuid>) -> Response {
    let store = match &state.store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "persistence not configured",
            )
                .into_response();
        }
    };

    match commands::manual_retry(&**store, id).await {
        Ok(op) => (StatusCode::OK, Json(OperationResponseDto::from(op))).into_response(),
        Err(e) => error_to_response(e),
    }
}

async fn reconcile_operation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ReconcileRequestDto>,
) -> Response {
    let store = match &state.store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "persistence not configured",
            )
                .into_response();
        }
    };

    match commands::reconcile_operation(
        &**store,
        id,
        body.provider_status,
        body.outcome,
        body.details,
    )
    .await
    {
        Ok(rec) => (StatusCode::OK, Json(ReconcileResponseDto::from(rec))).into_response(),
        Err(e) => error_to_response(e),
    }
}

async fn receive_callback(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let store = match &state.store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "persistence not configured",
            )
                .into_response();
        }
    };

    let sig = headers
        .get("x-callback-signature")
        .or_else(|| headers.get("x-signature-sha256"))
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("invalid JSON: {e}")})),
            )
                .into_response();
        }
    };

    match commands::process_callback(
        &**store,
        &*state.verifier,
        provider.clone(),
        sig,
        payload,
        &body,
    )
    .await
    {
        Ok(cb) => (
            StatusCode::OK,
            Json(CallbackResponseDto {
                callback_id: cb.id,
                provider: cb.provider,
                status: cb.status,
                operation_id: cb.operation_id,
                received_at: cb.received_at,
            }),
        )
            .into_response(),
        Err(e) => error_to_response(e),
    }
}

async fn list_providers(State(state): State<AppState>) -> Response {
    let store = match &state.store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "persistence not configured",
            )
                .into_response();
        }
    };

    match queries::list_providers(&**store).await {
        Ok(providers) => {
            let dtos: Vec<ProviderResponseDto> = providers
                .into_iter()
                .map(ProviderResponseDto::from)
                .collect();
            (StatusCode::OK, Json(dtos)).into_response()
        }
        Err(e) => error_to_response(e),
    }
}

#[derive(Deserialize)]
struct ListQuery {
    tenant_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn list_operations(State(state): State<AppState>, Query(q): Query<ListQuery>) -> Response {
    let store = match &state.store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "persistence not configured",
            )
                .into_response();
        }
    };

    let tenant = q.tenant_id.unwrap_or_else(|| "default-tenant".into());
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);

    match queries::list_operations(&**store, &tenant, limit, offset).await {
        Ok(ops) => {
            let dtos: Vec<OperationResponseDto> =
                ops.into_iter().map(OperationResponseDto::from).collect();
            (StatusCode::OK, Json(dtos)).into_response()
        }
        Err(e) => error_to_response(e),
    }
}

// ---- Health & System Info ----

async fn live() -> impl IntoResponse {
    (StatusCode::OK, Json(HealthResponse { status: "ok" }))
}

async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    match &state.db {
        Some(pool) => {
            if persistence::ping(pool).await {
                (StatusCode::OK, Json(HealthResponse { status: "ready" }))
            } else {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(HealthResponse { status: "degraded" }),
                )
            }
        }
        None => (StatusCode::OK, Json(HealthResponse { status: "ready" })),
    }
}

async fn system_info(State(state): State<AppState>) -> impl IntoResponse {
    let info = GetServiceInfoQuery {
        service_name: state.config.service_name.clone(),
        service_version: state.config.service_version.clone(),
        environment: state.config.environment.clone(),
    }
    .execute();
    Json(SystemInfoResponse {
        service: info.service,
        version: info.version,
        environment: info.environment,
        status: info.status,
    })
}

async fn openapi_json() -> impl IntoResponse {
    (
        StatusCode::OK,
        [("content-type", "application/json")],
        ApiDoc::openapi().to_json().unwrap_or_default(),
    )
}
