-- 启用 UUID 主键
CREATE TABLE sys_user (
    user_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) NOT NULL UNIQUE, -- 登录账号
    nickname VARCHAR(50) NOT NULL,        -- 显示名称
    password_hash VARCHAR(200) NOT NULL,  -- 加密后的密码
    role VARCHAR(20) NOT NULL DEFAULT 'USER', -- 用户角色，如 ADMIN、USER

    -- 基础信息
    email VARCHAR(100),
    phone VARCHAR(20),
    avatar_url TEXT,

    -- 状态控制
    is_active BOOLEAN NOT NULL DEFAULT TRUE,

    -- 系统元数据
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sys_user_username ON sys_user(username);

-- 自动更新 updated_at 的触发器函数
CREATE OR REPLACE FUNCTION update_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_sys_user_modtime
    BEFORE UPDATE ON sys_user
    FOR EACH ROW
    EXECUTE PROCEDURE update_timestamp();

-- 插入初始管理员用户
-- username: admin
-- password: admin123
INSERT INTO sys_user (username, nickname, password_hash, role, is_active)
VALUES (
    'admin',
    '系统管理员',
    '$argon2id$v=19$m=19456,t=2,p=1$hmEX4K3tsRMf7/s1Fl36Ww$L+ltX2iKO0w9w9SS8pAkTYFmlYLe8j10ZyDZmVLcpms',
    'ADMIN',
    TRUE
) ON CONFLICT (username) DO NOTHING;