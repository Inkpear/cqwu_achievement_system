-- 归档模板表
CREATE TABLE
    sys_template (
        template_id UUID PRIMARY KEY DEFAULT gen_random_uuid (),
        name VARCHAR(100) NOT NULL, -- 模板名称
        category VARCHAR(50) NOT NULL, -- 分类
        description TEXT,
        schema_def JSONB NOT NULL,
        is_active BOOLEAN NOT NULL DEFAULT TRUE,
        created_by UUID REFERENCES sys_user (user_id) ON DELETE SET NULL,
        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
        updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW ()
    );

-- 归档记录表
CREATE TABLE
    archive_record (
        record_id UUID PRIMARY KEY DEFAULT gen_random_uuid (),
        template_id UUID NOT NULL REFERENCES sys_template (template_id) ON DELETE RESTRICT,
        data JSONB NOT NULL,
        created_by UUID REFERENCES sys_user (user_id) ON DELETE SET NULL,
        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
        updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW ()
    );

-- 索引优化
CREATE INDEX idx_archive_template ON archive_record (template_id);

CREATE INDEX idx_archive_created_by ON archive_record (created_by);

-- GIN 索引：允许对 JSON 内部字段进行查询
CREATE INDEX idx_archive_data ON archive_record USING GIN (data);

CREATE TRIGGER update_sys_template_modtime BEFORE
UPDATE ON sys_template FOR EACH ROW EXECUTE PROCEDURE update_timestamp ();

CREATE TRIGGER update_archive_record_modtime BEFORE
UPDATE ON archive_record FOR EACH ROW EXECUTE PROCEDURE update_timestamp ();