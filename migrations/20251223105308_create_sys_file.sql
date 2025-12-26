CREATE TABLE
    sys_file (
        file_id UUID PRIMARY KEY,
        record_id UUID NOT NULL REFERENCES archive_record (record_id) ON DELETE CASCADE,
        filename VARCHAR(255) NOT NULL,
        object_key VARCHAR(255) NOT NULL,
        file_size BIGINT NOT NULL,
        mime_type VARCHAR(100) NOT NULL,
        uploaded_by UUID REFERENCES sys_user (user_id) ON DELETE SET NULL,
        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW ()
    );

CREATE INDEX idx_file_record ON sys_file (record_id);