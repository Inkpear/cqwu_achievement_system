use std::time::Duration;

use anyhow::anyhow;
use cqwu_achievement_system::tasks::{
    dispatcher::TaskDispatcher,
    models::{OutboxStatus, TaskCommand},
};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::{
    helper::{TestApp, TestUser, check_response_code_and_message, generate_a_dummy_file_content},
    s3_test_support::{
        BucketFixture, count_outbox_rows, count_outbox_rows_by_status,
        create_archive_record_with_uploaded_file, list_outbox_commands, list_outbox_statuses,
        unique_bucket_name, wait_until,
    },
};

#[tokio::test]
async fn delete_archive_persists_compensation_when_worker_disabled() {
    let bucket = unique_bucket_name();
    let mut app = TestApp::spawn_with_overrides(|settings| {
        settings.storage.bucket_name = bucket.clone();
        settings.tasks.command_worker_concurrency = 1;
        settings.tasks.orphan_cleanup_interval_seconds = 3600;
        settings.tasks.command_worker_enabled = false;
    })
    .await;

    let fixture = BucketFixture::create(&app.settings, bucket.clone())
        .await
        .expect("failed to create isolated test bucket");

    let (template_id, record_id, _object_key) = create_archive_record_with_uploaded_file(&mut app)
        .await
        .expect("failed to create archive record with uploaded file");

    fixture
        .delete_bucket_and_contents()
        .await
        .expect("failed to delete bucket for S3-failure simulation");

    let delete_res = app
        .delete_archive_record(&template_id, &record_id)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("failed to parse delete response");
    check_response_code_and_message(&delete_res, 200, "删除归档记录成功");

    wait_until(Duration::from_secs(10), Duration::from_millis(200), || {
        let pool = app.db_pool.clone();
        async move {
            matches!(
                count_outbox_rows_by_status(&pool, OutboxStatus::Pending.as_i16()).await,
                Ok(v) if v >= 1
            )
        }
    })
    .await
    .expect("expected compensation task to remain pending in outbox when worker is disabled");

    let commands = list_outbox_commands(&app.db_pool)
        .await
        .expect("failed to load outbox commands");
    assert!(
        !commands.is_empty(),
        "expected compensation payload to persist in outbox"
    );

    let statuses = list_outbox_statuses(&app.db_pool)
        .await
        .expect("failed to load outbox statuses");
    assert!(
        statuses
            .iter()
            .any(|status| *status == OutboxStatus::Pending.as_i16()),
        "expected at least one pending outbox row"
    );

    fixture
        .recreate_bucket()
        .await
        .expect("failed to recreate bucket for cleanup");

    app.shutdown().await;
    fixture.cleanup().await.expect("failed to cleanup bucket");
}

#[tokio::test]
async fn startup_recovery_consumes_existing_outbox_task() {
    let bucket = unique_bucket_name();
    let app1 = TestApp::spawn_with_overrides(|settings| {
        settings.storage.bucket_name = bucket.clone();
        settings.tasks.command_worker_concurrency = 1;
        settings.tasks.orphan_cleanup_interval_seconds = 3600;
    })
    .await;

    let fixture = BucketFixture::create(&app1.settings, bucket.clone())
        .await
        .expect("failed to create isolated test bucket");

    let object_key = format!("archive/{}/{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4());

    fixture
        .put_object_bytes(&object_key, b"outbox-recovery-test".to_vec(), "text/plain")
        .await
        .expect("failed to put object for recovery test");

    let payload = serde_json::to_value(TaskCommand::DeleteArchiveObjects {
        object_keys: vec![object_key.clone()],
    })
    .expect("failed to serialize task payload");

    sqlx::query(
        r#"
        INSERT INTO task_outbox (command_type, payload)
        VALUES ($1, $2)
        "#,
    )
    .bind("DeleteArchiveObjects")
    .bind(payload)
    .execute(&app1.db_pool)
    .await
    .expect("failed to seed task_outbox row");

    let restart_settings = app1.settings.clone();
    let restart_pool = app1.db_pool.clone();
    app1.shutdown().await;

    let app2 = TestApp::spawn_from_settings(restart_settings, false).await;

    wait_until(Duration::from_secs(20), Duration::from_millis(300), || {
        let pool = restart_pool.clone();
        async move { matches!(count_outbox_rows(&pool).await, Ok(0)) }
    })
    .await
    .map_err(|e| anyhow!("outbox row was not consumed after restart: {}", e))
    .expect("outbox row should be consumed after restart recovery");

    wait_until(Duration::from_secs(20), Duration::from_millis(300), || {
        let key = object_key.clone();
        let fixture_ref = &fixture;
        async move { matches!(fixture_ref.object_exists(&key).await, Ok(false)) }
    })
    .await
    .expect("object key should be deleted by recovered task");

    app2.shutdown().await;
    fixture.cleanup().await.expect("failed to cleanup bucket");
}

#[tokio::test]
async fn startup_recovery_reclaims_stale_running_outbox_task() {
    let bucket = unique_bucket_name();
    let app1 = TestApp::spawn_with_overrides(|settings| {
        settings.storage.bucket_name = bucket.clone();
        settings.tasks.command_worker_concurrency = 1;
        settings.tasks.orphan_cleanup_interval_seconds = 3600;
        settings.tasks.outbox_running_timeout_seconds = 1;
    })
    .await;

    let fixture = BucketFixture::create(&app1.settings, bucket.clone())
        .await
        .expect("failed to create isolated test bucket");

    let object_key = format!("archive/{}/{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4());

    fixture
        .put_object_bytes(
            &object_key,
            b"stale-running-recovery-test".to_vec(),
            "text/plain",
        )
        .await
        .expect("failed to put object for stale running recovery test");

    let payload = serde_json::to_value(TaskCommand::DeleteArchiveObjects {
        object_keys: vec![object_key.clone()],
    })
    .expect("failed to serialize stale running task payload");

    sqlx::query(
        r#"
        INSERT INTO task_outbox (command_type, payload, status, next_retry_at, updated_at)
        VALUES ($1, $2, $3, NOW(), NOW() - INTERVAL '10 minutes')
        "#,
    )
    .bind("DeleteArchiveObjects")
    .bind(payload)
    .bind(OutboxStatus::Running.as_i16())
    .execute(&app1.db_pool)
    .await
    .expect("failed to seed stale running outbox row");

    let restart_settings = app1.settings.clone();
    let restart_pool = app1.db_pool.clone();
    app1.shutdown().await;

    let app2 = TestApp::spawn_from_settings(restart_settings, false).await;

    wait_until(Duration::from_secs(20), Duration::from_millis(300), || {
        let pool = restart_pool.clone();
        async move { matches!(count_outbox_rows(&pool).await, Ok(0)) }
    })
    .await
    .expect("stale running task should be reclaimed and consumed after restart");

    wait_until(Duration::from_secs(20), Duration::from_millis(300), || {
        let key = object_key.clone();
        let fixture_ref = &fixture;
        async move { matches!(fixture_ref.object_exists(&key).await, Ok(false)) }
    })
    .await
    .expect("object key should be deleted by reclaimed stale running task");

    app2.shutdown().await;
    fixture.cleanup().await.expect("failed to cleanup bucket");
}

#[tokio::test]
async fn startup_recovery_does_not_reclaim_fresh_running_outbox_task() {
    let bucket = unique_bucket_name();
    let app1 = TestApp::spawn_with_overrides(|settings| {
        settings.storage.bucket_name = bucket.clone();
        settings.tasks.command_worker_concurrency = 1;
        settings.tasks.orphan_cleanup_interval_seconds = 3600;
        settings.tasks.outbox_running_timeout_seconds = 600;
    })
    .await;

    let fixture = BucketFixture::create(&app1.settings, bucket.clone())
        .await
        .expect("failed to create isolated test bucket");

    let object_key = format!("archive/{}/{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4());

    fixture
        .put_object_bytes(
            &object_key,
            b"fresh-running-recovery-test".to_vec(),
            "text/plain",
        )
        .await
        .expect("failed to put object for fresh running recovery test");

    let payload = serde_json::to_value(TaskCommand::DeleteArchiveObjects {
        object_keys: vec![object_key.clone()],
    })
    .expect("failed to serialize fresh running task payload");

    sqlx::query(
        r#"
        INSERT INTO task_outbox (command_type, payload, status, next_retry_at, updated_at)
        VALUES ($1, $2, $3, NOW(), NOW())
        "#,
    )
    .bind("DeleteArchiveObjects")
    .bind(payload)
    .bind(OutboxStatus::Running.as_i16())
    .execute(&app1.db_pool)
    .await
    .expect("failed to seed fresh running outbox row");

    let restart_settings = app1.settings.clone();
    let restart_pool = app1.db_pool.clone();
    app1.shutdown().await;

    let app2 = TestApp::spawn_from_settings(restart_settings, false).await;

    wait_until(Duration::from_secs(10), Duration::from_millis(200), || {
        let pool = restart_pool.clone();
        async move {
            matches!(
                count_outbox_rows_by_status(&pool, OutboxStatus::Running.as_i16()).await,
                Ok(v) if v >= 1
            )
        }
    })
    .await
    .expect("fresh running task should remain running when timeout is not exceeded");

    let object_still_exists = fixture
        .object_exists(&object_key)
        .await
        .expect("failed to check object existence for fresh running task");
    assert!(
        object_still_exists,
        "fresh running task should not be reclaimed and object should remain"
    );

    app2.shutdown().await;
    fixture.cleanup().await.expect("failed to cleanup bucket");
}

#[tokio::test]
async fn startup_recovery_marks_non_retryable_outbox_task_dead() {
    let app1 = TestApp::spawn_with_overrides(|settings| {
        settings.tasks.command_worker_concurrency = 1;
        settings.tasks.orphan_cleanup_interval_seconds = 3600;
    })
    .await;

    let restart_settings = app1.settings.clone();
    let restart_pool = app1.db_pool.clone();
    app1.shutdown().await;

    let payload = serde_json::to_value(TaskCommand::DeleteArchiveObjects {
        object_keys: vec!["".to_string()],
    })
    .expect("failed to serialize non-retryable task payload");

    sqlx::query(
        r#"
        INSERT INTO task_outbox (command_type, payload)
        VALUES ($1, $2)
        "#,
    )
    .bind("DeleteArchiveObjects")
    .bind(payload)
    .execute(&restart_pool)
    .await
    .expect("failed to seed non-retryable task_outbox row");

    let app2 = TestApp::spawn_from_settings(restart_settings, false).await;

    wait_until(Duration::from_secs(10), Duration::from_millis(200), || {
        let pool = restart_pool.clone();
        async move {
            matches!(
                count_outbox_rows_by_status(&pool, OutboxStatus::Dead.as_i16()).await,
                Ok(v) if v >= 1
            )
        }
    })
    .await
    .expect("non-retryable task should be marked dead in outbox");

    let total = count_outbox_rows(&restart_pool)
        .await
        .expect("failed to count outbox rows after non-retryable task");
    assert!(
        total >= 1,
        "dead task should remain in outbox for inspection"
    );

    app2.shutdown().await;
}

#[tokio::test]
async fn startup_recovery_consumes_pending_outbox_task_after_worker_reenabled() {
    let bucket = unique_bucket_name();
    let app1 = TestApp::spawn_with_overrides(|settings| {
        settings.storage.bucket_name = bucket.clone();
        settings.tasks.command_worker_enabled = false;
        settings.tasks.command_worker_concurrency = 1;
        settings.tasks.orphan_cleanup_interval_seconds = 3600;
        settings.tasks.outbox_pull_interval_millis = 100;
        settings.tasks.outbox_pull_batch_size = 16;
    })
    .await;

    let fixture = BucketFixture::create(&app1.settings, bucket.clone())
        .await
        .expect("failed to create isolated test bucket");

    let object_key = format!("archive/{}/{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4());

    fixture
        .put_object_bytes(&object_key, b"pending-outbox-test".to_vec(), "text/plain")
        .await
        .expect("failed to put object for pending outbox test");

    let payload = serde_json::to_value(TaskCommand::DeleteArchiveObjects {
        object_keys: vec![object_key.clone()],
    })
    .expect("failed to serialize pending task payload");

    sqlx::query(
        r#"
        INSERT INTO task_outbox (command_type, payload)
        VALUES ($1, $2)
        "#,
    )
    .bind("DeleteArchiveObjects")
    .bind(payload)
    .execute(&app1.db_pool)
    .await
    .expect("failed to seed pending outbox row");

    let mut restart_settings = app1.settings.clone();
    restart_settings.tasks.command_worker_enabled = true;
    let restart_pool = app1.db_pool.clone();
    app1.shutdown().await;

    let app2 = TestApp::spawn_from_settings(restart_settings, false).await;

    wait_until(Duration::from_secs(20), Duration::from_millis(300), || {
        let pool = restart_pool.clone();
        async move { matches!(count_outbox_rows(&pool).await, Ok(0)) }
    })
    .await
    .expect("pending outbox task should be consumed after worker is re-enabled");

    wait_until(Duration::from_secs(20), Duration::from_millis(300), || {
        let key = object_key.clone();
        let fixture_ref = &fixture;
        async move { matches!(fixture_ref.object_exists(&key).await, Ok(false)) }
    })
    .await
    .expect("object key should be deleted after pending outbox task is consumed");

    app2.shutdown().await;
    fixture.cleanup().await.expect("failed to cleanup bucket");
}

#[tokio::test]
async fn startup_sanitizes_zero_task_settings() {
    let app = TestApp::spawn_with_overrides(|settings| {
        settings.tasks.orphan_cleanup_interval_seconds = 0;
        settings.tasks.orphan_cleanup_min_age_seconds = 0;
        settings.tasks.command_queue_size = 0;
        settings.tasks.command_worker_concurrency = 0;
        settings.tasks.outbox_running_timeout_seconds = 0;
    })
    .await;

    let health_res = app
        .api_client
        .get(format!("{}/health_check", app.address))
        .send()
        .await
        .expect("failed to query health_check");
    assert!(health_res.status().is_success());

    app.shutdown().await;
}

#[tokio::test]
async fn concurrent_pump_outbox_once_claims_row_only_once() {
    let app = TestApp::spawn_with_overrides(|settings| {
        settings.tasks.command_worker_enabled = false;
        settings.tasks.orphan_cleanup_interval_seconds = 3600;
    })
    .await;

    let payload = serde_json::to_value(TaskCommand::DeleteArchiveObjects {
        object_keys: vec![format!(
            "archive/{}/{}",
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4()
        )],
    })
    .expect("failed to serialize task payload");

    let outbox_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO task_outbox (command_type, payload)
        VALUES ($1, $2)
        RETURNING id
        "#,
    )
    .bind("DeleteArchiveObjects")
    .bind(payload)
    .fetch_one(&app.db_pool)
    .await
    .expect("failed to seed outbox row");

    let (tx1, mut rx1) = mpsc::channel(8);
    let (tx2, mut rx2) = mpsc::channel(8);
    let pool = Arc::new(app.db_pool.clone());
    let dispatcher1 = TaskDispatcher::new(tx1, pool.clone());
    let dispatcher2 = TaskDispatcher::new(tx2, pool);

    let (r1, r2) = tokio::join!(
        dispatcher1.pump_outbox_once(1),
        dispatcher2.pump_outbox_once(1)
    );
    let claimed_1 = r1.expect("pump 1 failed");
    let claimed_2 = r2.expect("pump 2 failed");

    assert_eq!(
        claimed_1 + claimed_2,
        1,
        "one outbox row should be claimed exactly once under concurrent pumping"
    );

    let mut queued = Vec::new();
    if let Ok(task) = rx1.try_recv() {
        queued.push(task);
    }
    if let Ok(task) = rx2.try_recv() {
        queued.push(task);
    }

    assert_eq!(
        queued.len(),
        1,
        "only one queue should receive the claimed task"
    );
    assert_eq!(
        queued[0].outbox_id,
        Some(outbox_id),
        "claimed task should carry the seeded outbox id"
    );

    app.shutdown().await;
}

#[tokio::test]
async fn periodic_cleanup_only_removes_persistent_prefix_orphans() {
    let bucket = unique_bucket_name();
    let app = TestApp::spawn_with_overrides(|settings| {
        settings.storage.bucket_name = bucket.clone();
        settings.tasks.command_worker_concurrency = 1;
        settings.tasks.orphan_cleanup_interval_seconds = 1;
        settings.tasks.orphan_cleanup_min_age_seconds = 1;
    })
    .await;

    let fixture = BucketFixture::create(&app.settings, bucket.clone())
        .await
        .expect("failed to create isolated test bucket");

    let archive_orphan = format!("archive/{}/{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let avatar_orphan = format!("avatar/{}.png", uuid::Uuid::new_v4());
    let temp_object = format!("temp/{}/{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4());

    fixture
        .put_object_bytes(&archive_orphan, b"archive-orphan".to_vec(), "text/plain")
        .await
        .expect("failed to put archive orphan object");
    fixture
        .put_object_bytes(&avatar_orphan, b"avatar-orphan".to_vec(), "text/plain")
        .await
        .expect("failed to put avatar orphan object");
    fixture
        .put_object_bytes(&temp_object, b"temp-object".to_vec(), "text/plain")
        .await
        .expect("failed to put temp object");

    wait_until(Duration::from_secs(20), Duration::from_millis(300), || {
        let archive_key = archive_orphan.clone();
        let avatar_key = avatar_orphan.clone();
        let fixture_ref = &fixture;
        async move {
            matches!(fixture_ref.object_exists(&archive_key).await, Ok(false))
                && matches!(fixture_ref.object_exists(&avatar_key).await, Ok(false))
        }
    })
    .await
    .expect("archive/avatar orphan objects were not cleaned in time");

    let temp_exists = fixture
        .object_exists(&temp_object)
        .await
        .expect("failed to check temp object existence");
    assert!(
        temp_exists,
        "temp object should not be cleaned by periodic task"
    );

    app.shutdown().await;
    fixture.cleanup().await.expect("failed to cleanup bucket");
}

#[tokio::test]
async fn periodic_cleanup_keeps_avatar_referenced_by_database() {
    let bucket = unique_bucket_name();
    let mut app = TestApp::spawn_with_overrides(|settings| {
        settings.storage.bucket_name = bucket.clone();
        settings.tasks.command_worker_concurrency = 1;
        settings.tasks.orphan_cleanup_interval_seconds = 1;
        settings.tasks.orphan_cleanup_min_age_seconds = 1;
    })
    .await;

    let fixture = BucketFixture::create(&app.settings, bucket.clone())
        .await
        .expect("failed to create isolated test bucket");

    let mut user = TestUser::new();
    user.store(&app.db_pool).await;
    app.login(&user).await;

    let filename = "avatar.png";
    let content = generate_a_dummy_file_content(64 * 1024);
    let presigned_req = serde_json::json!({
        "filename": filename,
        "content_length": content.len(),
    });

    let presigned_res = app
        .post_to_presigned_avatar_url(&presigned_req)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("failed to parse presigned response");
    check_response_code_and_message(&presigned_res, 201, "获取头像上传预签名 URL 成功");

    let file_id = presigned_res["data"]["file_id"]
        .as_str()
        .expect("file_id should be present");
    let presigned_url = presigned_res["data"]["url"]
        .as_str()
        .expect("presigned url should be present");

    let upload_res = reqwest::Client::new()
        .put(presigned_url)
        .header("Content-Type", "image/png")
        .header("x-amz-meta-original-filename", filename)
        .body(content)
        .send()
        .await
        .expect("failed to upload avatar");
    assert!(upload_res.status().is_success());

    let update_res = app
        .patch_to_update_avatar(file_id)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("failed to parse update avatar response");
    check_response_code_and_message(&update_res, 200, "更新用户头像成功");

    let user_id = user.user_id.expect("user_id should exist");
    let avatar_key: String = sqlx::query_scalar(
        r#"
        SELECT avatar_key
        FROM sys_user
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(&app.db_pool)
    .await
    .expect("failed to query saved avatar key");

    let avatar_object_key = avatar_key
        .split('.')
        .next()
        .expect("avatar_key should include object key")
        .to_string();

    let avatar_orphan = format!("avatar/{}.png", uuid::Uuid::new_v4());
    fixture
        .put_object_bytes(&avatar_orphan, b"avatar-orphan".to_vec(), "text/plain")
        .await
        .expect("failed to put avatar orphan object");

    wait_until(Duration::from_secs(20), Duration::from_millis(300), || {
        let orphan_key = avatar_orphan.clone();
        let fixture_ref = &fixture;
        async move { matches!(fixture_ref.object_exists(&orphan_key).await, Ok(false)) }
    })
    .await
    .expect("avatar orphan object was not cleaned in time");

    let referenced_avatar_exists = fixture
        .object_exists(&avatar_object_key)
        .await
        .expect("failed to check referenced avatar existence");
    assert!(
        referenced_avatar_exists,
        "avatar referenced by sys_user.avatar_key should not be cleaned"
    );

    app.shutdown().await;
    fixture.cleanup().await.expect("failed to cleanup bucket");
}

#[tokio::test]
async fn periodic_cleanup_is_skipped_while_advisory_lock_is_held() {
    const S3_CLEANUP_ADVISORY_LOCK_KEY: i64 = 4_292_430_981;

    let bucket = unique_bucket_name();
    let app = TestApp::spawn_with_overrides(|settings| {
        settings.storage.bucket_name = bucket.clone();
        settings.tasks.command_worker_concurrency = 1;
        settings.tasks.orphan_cleanup_interval_seconds = 1;
        settings.tasks.orphan_cleanup_min_age_seconds = 1;
    })
    .await;

    let fixture = BucketFixture::create(&app.settings, bucket.clone())
        .await
        .expect("failed to create isolated test bucket");

    let orphan_key = format!("archive/{}/{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    fixture
        .put_object_bytes(&orphan_key, b"locked-cleanup-orphan".to_vec(), "text/plain")
        .await
        .expect("failed to put orphan object");

    let mut lock_conn = app
        .db_pool
        .acquire()
        .await
        .expect("failed to acquire database connection for advisory lock");

    let lock_acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(S3_CLEANUP_ADVISORY_LOCK_KEY)
        .fetch_one(&mut *lock_conn)
        .await
        .expect("failed to acquire advisory lock");
    assert!(
        lock_acquired,
        "test should hold periodic cleanup advisory lock"
    );

    tokio::time::sleep(Duration::from_secs(3)).await;

    let still_exists = fixture
        .object_exists(&orphan_key)
        .await
        .expect("failed to check orphan object existence while lock is held");
    assert!(
        still_exists,
        "orphan object should not be cleaned while advisory lock is held by another session"
    );

    let unlocked: bool = sqlx::query_scalar("SELECT pg_advisory_unlock($1)")
        .bind(S3_CLEANUP_ADVISORY_LOCK_KEY)
        .fetch_one(&mut *lock_conn)
        .await
        .expect("failed to release advisory lock");
    assert!(unlocked, "advisory lock should be released");
    drop(lock_conn);

    wait_until(Duration::from_secs(20), Duration::from_millis(300), || {
        let key = orphan_key.clone();
        let fixture_ref = &fixture;
        async move { matches!(fixture_ref.object_exists(&key).await, Ok(false)) }
    })
    .await
    .expect("orphan object should be cleaned after advisory lock is released");

    app.shutdown().await;
    fixture.cleanup().await.expect("failed to cleanup bucket");
}
