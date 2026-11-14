-- Outbox distributed leasing support
ALTER TABLE outbox_events
    ADD COLUMN IF NOT EXISTS locked_until TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS locked_by TEXT;

CREATE INDEX IF NOT EXISTS idx_outbox_lease_claim
    ON outbox_events (published, locked_until, created_at)
    WHERE published = FALSE;
