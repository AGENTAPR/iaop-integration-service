-- Integration Operations, Providers, Attempts, Callbacks, and Reconciliations

CREATE TABLE IF NOT EXISTS integration_providers (
    id VARCHAR(64) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    kind VARCHAR(64) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'ACTIVE',
    base_url TEXT NOT NULL,
    timeout_ms BIGINT NOT NULL DEFAULT 10000,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS integration_operations (
    id UUID PRIMARY KEY,
    tenant_id VARCHAR(64) NOT NULL,
    requesting_service VARCHAR(128) NOT NULL,
    provider VARCHAR(64) NOT NULL REFERENCES integration_providers(id),
    operation_type VARCHAR(128) NOT NULL,
    business_reference VARCHAR(255) NOT NULL,
    idempotency_key VARCHAR(255) NOT NULL,
    request_fingerprint VARCHAR(64) NOT NULL,
    correlation_id UUID,
    causation_id UUID,
    external_reference VARCHAR(255),
    status VARCHAR(64) NOT NULL,
    attempt_count INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 5,
    next_retry_at TIMESTAMPTZ,
    locked_until TIMESTAMPTZ,
    locked_by VARCHAR(128),
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    result JSONB,
    error_details TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_int_op_idempotency 
    ON integration_operations(tenant_id, requesting_service, operation_type, idempotency_key);

CREATE INDEX IF NOT EXISTS idx_int_op_dispatch_claim 
    ON integration_operations(status, next_retry_at, locked_until) 
    WHERE status IN ('ACCEPTED', 'PENDING', 'FAILED_RETRYABLE');

CREATE INDEX IF NOT EXISTS idx_int_op_tenant 
    ON integration_operations(tenant_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_int_op_bus_ref 
    ON integration_operations(business_reference);

CREATE TABLE IF NOT EXISTS integration_operation_attempts (
    id UUID PRIMARY KEY,
    operation_id UUID NOT NULL REFERENCES integration_operations(id) ON DELETE CASCADE,
    attempt_number INT NOT NULL,
    dispatched_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    status VARCHAR(64) NOT NULL,
    http_status INT,
    error_code VARCHAR(64),
    error_message TEXT,
    latency_ms BIGINT
);

CREATE INDEX IF NOT EXISTS idx_int_attempts_op 
    ON integration_operation_attempts(operation_id, attempt_number);

CREATE TABLE IF NOT EXISTS integration_callbacks (
    id UUID PRIMARY KEY,
    provider VARCHAR(64) NOT NULL REFERENCES integration_providers(id),
    callback_ref VARCHAR(255),
    signature TEXT,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    status VARCHAR(64) NOT NULL,
    operation_id UUID REFERENCES integration_operations(id),
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_int_cb_provider_ref 
    ON integration_callbacks(provider, callback_ref);

CREATE TABLE IF NOT EXISTS integration_reconciliations (
    id UUID PRIMARY KEY,
    operation_id UUID NOT NULL REFERENCES integration_operations(id) ON DELETE CASCADE,
    checked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    provider_status VARCHAR(64),
    outcome VARCHAR(64) NOT NULL,
    details TEXT
);

CREATE INDEX IF NOT EXISTS idx_int_rec_op 
    ON integration_reconciliations(operation_id, checked_at DESC);

-- Seed baseline standard providers
INSERT INTO integration_providers (id, name, kind, status, base_url, timeout_ms)
VALUES 
    ('dukcapil-id', 'Dukcapil National Identity Verification', 'IDENTITY_VERIFICATION', 'ACTIVE', 'https://api.dukcapil.gov.id/v1', 5000),
    ('ojk-license', 'OJK Agent License & Credential Registry', 'CREDENTIAL_VERIFICATION', 'ACTIVE', 'https://api.ojk.go.id/licensing/v1', 5000),
    ('core-insurance', 'IAOP Enterprise Core Policy System', 'CORE_INSURANCE', 'ACTIVE', 'http://iaop-core-insurance:8080/v1', 10000),
    ('mock-provider', 'Test Simulator Mock Provider', 'GENERIC', 'ACTIVE', 'http://127.0.0.1:0', 5000)
ON CONFLICT (id) DO NOTHING;
