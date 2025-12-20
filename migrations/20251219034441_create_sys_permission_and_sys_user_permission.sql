CREATE TABLE sys_access_rule (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    user_id UUID NOT NULL REFERENCES sys_user(user_id) ON DELETE CASCADE,
    -- API 匹配模式
    api_pattern VARCHAR(255) NOT NULL, 
    -- HTTP 方法，如 GET、POST、PUT、DELETE 等，ALL 表示所有方法
    http_method VARCHAR(10) NOT NULL DEFAULT 'ALL',
    -- 规则描述
    description TEXT,
    -- 过期时间
    expires_at TIMESTAMPTZ,
    -- 该权限由谁授予
    granted_by UUID REFERENCES sys_user(user_id) ON DELETE SET NULL,
    -- 创建时间(或被更新的时间)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (user_id, api_pattern, http_method)
);


CREATE INDEX idx_access_rule_match ON sys_access_rule (user_id, api_pattern varchar_pattern_ops);