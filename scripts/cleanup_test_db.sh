#!/usr/bin/env bash
set -euo pipefail

DB_URL="${TEST_DB_ADMIN_URL:-postgres://postgres:password@192.168.31.199:5432/postgres}"

# S3 test bucket cleanup settings (matches local integration-test defaults).
S3_ENDPOINT="${TEST_S3_ENDPOINT:-http://10.41.94.104:9000}"
S3_REGION="${TEST_S3_REGION:-cn-north-1}"
S3_ACCESS_KEY="${TEST_S3_ACCESS_KEY:-root}"
S3_SECRET_KEY="${TEST_S3_SECRET_KEY:-admin123}"
TEST_BUCKET_PREFIX="${TEST_S3_BUCKET_PREFIX:-itest-archive-}"
ENABLE_S3_CLEANUP="${ENABLE_S3_CLEANUP:-1}"

cleanup_test_databases() {
    local test_dbs
    test_dbs=$(psql "$DB_URL" -t -A -c "
    SELECT datname FROM pg_database
    WHERE datname ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
    ")

    if [ -z "$test_dbs" ]; then
        echo "没有找到测试数据库"
        return
    fi

    local count=0
    while IFS= read -r db; do
        [ -z "$db" ] && continue

        echo "删除数据库: $db"

        psql "$DB_URL" -c "
        SELECT pg_terminate_backend(pid)
        FROM pg_stat_activity
        WHERE datname = '$db' AND pid <> pg_backend_pid()
        " >/dev/null 2>&1

        psql "$DB_URL" -c "DROP DATABASE \"$db\""
        count=$((count + 1))
    done <<< "$test_dbs"

    echo "测试数据库清理完成，共删除 $count 个"
}

cleanup_test_buckets() {
    if [ "$ENABLE_S3_CLEANUP" != "1" ]; then
        echo "跳过 S3 测试 bucket 清理 (ENABLE_S3_CLEANUP=$ENABLE_S3_CLEANUP)"
        return
    fi

    if ! command -v mc >/dev/null 2>&1; then
        echo "未检测到 MinIO 客户端 mc，跳过 S3 测试 bucket 清理"
        return
    fi

    local mc_alias="cleanup-minio-$PPID"
    mc alias set "$mc_alias" "$S3_ENDPOINT" "$S3_ACCESS_KEY" "$S3_SECRET_KEY" --api S3v4 >/dev/null 2>&1 || {
        echo "警告: MinIO alias 初始化失败，跳过 S3 测试 bucket 清理"
        return
    }

    local all_buckets
    all_buckets=$(mc ls "$mc_alias/" 2>/dev/null | awk '{print $NF}' | sed 's:/$::' || true)
    if [ -z "$all_buckets" ]; then
        echo "没有找到可清理的 S3 bucket"
        mc alias rm "$mc_alias" >/dev/null 2>&1 || true
        return
    fi

    local count=0
    local bucket
    for bucket in $all_buckets; do
        case "$bucket" in
            "$TEST_BUCKET_PREFIX"*)
                echo "删除 S3 测试 bucket: $bucket"
                mc rb --force "$mc_alias/$bucket" >/dev/null 2>&1 || true
                if ! mc ls "$mc_alias/$bucket" >/dev/null 2>&1; then
                    count=$((count + 1))
                else
                    echo "警告: 删除 bucket 失败: $bucket"
                fi
                ;;
            *) ;;
        esac
    done

    mc alias rm "$mc_alias" >/dev/null 2>&1 || true

    echo "S3 测试 bucket 清理完成，共删除 $count 个 (prefix=$TEST_BUCKET_PREFIX)"
}

cleanup_test_databases
cleanup_test_buckets
