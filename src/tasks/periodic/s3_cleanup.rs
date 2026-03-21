use std::collections::HashSet;
use std::time::Duration;

use sqlx::PgPool;

use crate::utils::s3_storage::S3Storage;

const S3_CLEANUP_ADVISORY_LOCK_KEY: i64 = 4_292_430_981;

pub async fn cleanup_orphan_persistent_objects(
    pool: &PgPool,
    s3_storage: &S3Storage,
    min_object_age: Duration,
) -> anyhow::Result<()> {
    let mut lock_conn = pool.acquire().await?;
    let lock_acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(S3_CLEANUP_ADVISORY_LOCK_KEY)
        .fetch_one(&mut *lock_conn)
        .await?;

    if !lock_acquired {
        tracing::debug!(
            "periodic s3 cleanup skipped because another instance is holding the advisory lock"
        );
        return Ok(());
    }

    let cleanup_result = async {
        let archive_keys_in_s3 = s3_storage
            .list_object_keys_with_prefix_older_than("archive/", min_object_age)
            .await?;
        let avatar_keys_in_s3 = s3_storage
            .list_object_keys_with_prefix_older_than("avatar/", min_object_age)
            .await?;

        let archive_keys_in_db: HashSet<String> = sqlx::query_scalar(
            r#"
		SELECT object_key
		FROM sys_file
		WHERE object_key LIKE 'archive/%'
		"#,
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect();

        let avatar_keys_in_db: HashSet<String> = sqlx::query_scalar(
            r#"
        SELECT CASE
                 WHEN avatar_key ~ '^avatar/.+\.[^./]+$'
                   THEN regexp_replace(avatar_key, '\.[^./]+$', '')
                 ELSE avatar_key
               END AS object_key
		FROM sys_user
		WHERE avatar_key IS NOT NULL
		  AND avatar_key LIKE 'avatar/%'
		"#,
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .filter(|v: &String| !v.is_empty())
        .collect();

        let mut orphan_keys = Vec::new();

        orphan_keys.extend(
            archive_keys_in_s3
                .into_iter()
                .filter(|key| !archive_keys_in_db.contains(key)),
        );

        orphan_keys.extend(
            avatar_keys_in_s3
                .into_iter()
                .filter(|key| !avatar_keys_in_db.contains(key)),
        );

        if orphan_keys.is_empty() {
            tracing::info!("periodic s3 cleanup finished, no orphan object found");
            return Ok(());
        }

        let orphan_count = orphan_keys.len();
        s3_storage.delete_objects(&orphan_keys).await?;
        tracing::info!(
            "periodic s3 cleanup finished, removed {} orphan object(s)",
            orphan_count
        );

        Ok::<(), anyhow::Error>(())
    }
    .await;

    if let Err(unlock_err) = sqlx::query_scalar::<_, bool>("SELECT pg_advisory_unlock($1)")
        .bind(S3_CLEANUP_ADVISORY_LOCK_KEY)
        .fetch_one(&mut *lock_conn)
        .await
    {
        tracing::warn!(
            "failed to release periodic s3 cleanup advisory lock: {:?}",
            unlock_err
        );
    }

    cleanup_result
}
