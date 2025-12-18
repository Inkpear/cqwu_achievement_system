-- 添加角色字段
ALTER TABLE sys_user
ADD COLUMN role VARCHAR(20) NOT NULL DEFAULT 'USER';

-- 修改密码哈希字段长度
ALTER TABLE sys_user
ALTER COLUMN password_hash TYPE VARCHAR(200);

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
