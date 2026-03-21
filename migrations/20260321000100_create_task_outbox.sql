CREATE TABLE
    task_outbox (
        id BIGSERIAL PRIMARY KEY,
        command_type TEXT NOT NULL,
        payload JSONB NOT NULL,
        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW ()
    );

CREATE INDEX idx_task_outbox_created_at ON task_outbox (created_at);