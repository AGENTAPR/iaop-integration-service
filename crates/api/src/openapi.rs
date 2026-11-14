//! OpenAPI 3.1 document generation for Integration Platform.

use utoipa::OpenApi;

use crate::dto::*;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Integration Platform API",
        version = "0.1.0",
        description = "IAOP Integration Platform service API. Manages durable external operations, connectors, callbacks, and reconciliations."
    ),
    servers((url = "/")),
    components(schemas(
        HealthResponse,
        SystemInfoResponse,
        SubmitOperationDto,
        OperationResponseDto,
        AttemptResponseDto,
        ProviderResponseDto,
        CallbackResponseDto,
        ReconcileRequestDto,
        ReconcileResponseDto
    ))
)]
pub struct ApiDoc;
