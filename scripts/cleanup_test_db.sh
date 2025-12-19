#!/usr/bin/env bash
set -e

DB_URL="postgres://postgres:password@192.168.31.199:5432/postgres"

TEST_DBS=$(psql "$DB_URL" -t -A -c "
SELECT datname FROM pg_database 
WHERE datname ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
")

if [ -z "$TEST_DBS" ]; then
    echo "没有找到测试数据库"
    exit 0
fi

COUNT=0

while IFS= read -r db; do
    [ -z "$db" ] && continue
    
    echo "删除数据库: $db"
    
    psql "$DB_URL" -c "
    SELECT pg_terminate_backend(pid)
    FROM pg_stat_activity
    WHERE datname = '$db' AND pid <> pg_backend_pid()
    " > /dev/null 2>&1

    psql "$DB_URL" -c "DROP DATABASE \"$db\"" && COUNT=$((COUNT + 1))
    
done <<< "$TEST_DBS"

echo "清理完成，共删除 $COUNT 个测试数据库"
