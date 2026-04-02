-- Remove the historical built-in admin account so production no longer depends
-- on a fixed default credential from SQL seed data.
DELETE FROM sys_user
WHERE
    username = 'admin'
    AND role = 'ADMIN'
    AND password_hash = '$argon2id$v=19$m=19456,t=2,p=1$hmEX4K3tsRMf7/s1Fl36Ww$L+ltX2iKO0w9w9SS8pAkTYFmlYLe8j10ZyDZmVLcpms';