use std::time::{Duration, Instant};

use anyhow::{Context, anyhow};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{
    Client,
    config::Credentials,
    operation::head_object::HeadObjectError,
    types::{Delete, ObjectIdentifier},
};
use cqwu_achievement_system::configuration::Settings;
use secrecy::ExposeSecret;
use sqlx::PgPool;
use uuid::Uuid;

use crate::helper::{
    TestApp, TestUser, check_response_code_and_message, generate_a_dummy_file_content,
};

pub fn unique_bucket_name() -> String {
    format!("itest-archive-{}", Uuid::new_v4().simple())
}

pub async fn build_test_s3_client(settings: &Settings) -> Client {
    let shared_config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(settings.storage.region.clone()))
        .endpoint_url(settings.storage.endpoint.clone())
        .credentials_provider(Credentials::new(
            settings.storage.access_key.clone(),
            settings.storage.secret_key.expose_secret(),
            None,
            None,
            "itest-static",
        ))
        .load()
        .await;

    let s3_config = aws_sdk_s3::config::Builder::from(&shared_config)
        .force_path_style(true)
        .build();

    Client::from_conf(s3_config)
}

pub struct BucketFixture {
    pub name: String,
    client: Client,
}

impl BucketFixture {
    pub async fn create(settings: &Settings, bucket: String) -> anyhow::Result<Self> {
        let client = build_test_s3_client(settings).await;

        client
            .create_bucket()
            .bucket(&bucket)
            .send()
            .await
            .with_context(|| format!("failed to create bucket {}", bucket))?;

        Ok(Self {
            name: bucket,
            client,
        })
    }

    pub async fn object_exists(&self, key: &str) -> anyhow::Result<bool> {
        let result = self
            .client
            .head_object()
            .bucket(&self.name)
            .key(key)
            .send()
            .await;
        match result {
            Ok(_) => Ok(true),
            Err(err) => {
                if let Some(service_err) = err.as_service_error()
                    && matches!(service_err, HeadObjectError::NotFound(_))
                {
                    return Ok(false);
                }

                let text = err.to_string();
                if text.contains("NotFound") || text.contains("NoSuchKey") || text.contains("404") {
                    Ok(false)
                } else {
                    Err(anyhow!("failed to check object existence: {}", text))
                }
            }
        }
    }

    pub async fn put_object_bytes(
        &self,
        key: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> anyhow::Result<()> {
        self.client
            .put_object()
            .bucket(&self.name)
            .key(key)
            .body(bytes.into())
            .content_type(content_type)
            .send()
            .await
            .with_context(|| format!("failed to put object {}", key))?;

        Ok(())
    }

    pub async fn cleanup(self) -> anyhow::Result<()> {
        self.delete_bucket_and_contents().await
    }

    pub async fn delete_bucket_and_contents(&self) -> anyhow::Result<()> {
        let objects = self
            .client
            .list_objects_v2()
            .bucket(&self.name)
            .send()
            .await;

        let objects = match objects {
            Ok(v) => v,
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("NoSuchBucket") || msg.contains("NotFound") || msg.contains("404") {
                    return Ok(());
                }
                return Err(anyhow!("failed to list objects in {}: {}", self.name, msg));
            }
        };

        let keys = objects
            .contents()
            .iter()
            .filter_map(|obj| obj.key())
            .map(|key| {
                ObjectIdentifier::builder()
                    .key(key)
                    .build()
                    .map_err(|e| anyhow!("failed to build object identifier for {}: {}", key, e))
            })
            .collect::<Result<Vec<_>, _>>()?;

        if !keys.is_empty() {
            self.client
                .delete_objects()
                .bucket(&self.name)
                .delete(Delete::builder().set_objects(Some(keys)).build()?)
                .send()
                .await
                .with_context(|| format!("failed to delete objects from {}", self.name))?;
        }

        let delete_bucket_result = self.client.delete_bucket().bucket(&self.name).send().await;

        if let Err(e) = delete_bucket_result {
            let msg = e.to_string();
            if !(msg.contains("NoSuchBucket") || msg.contains("NotFound") || msg.contains("404")) {
                return Err(anyhow!("failed to delete bucket {}: {}", self.name, msg));
            }
        }

        Ok(())
    }

    pub async fn recreate_bucket(&self) -> anyhow::Result<()> {
        let create_result = self.client.create_bucket().bucket(&self.name).send().await;
        if let Err(e) = create_result {
            let msg = e.to_string();
            if !(msg.contains("BucketAlreadyOwnedByYou") || msg.contains("BucketAlreadyExists")) {
                return Err(anyhow!("failed to recreate bucket {}: {}", self.name, msg));
            }
        }

        Ok(())
    }
}

pub async fn wait_until<F, Fut>(
    timeout: Duration,
    interval: Duration,
    mut condition: F,
) -> anyhow::Result<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if condition().await {
            return Ok(());
        }
        tokio::time::sleep(interval).await;
    }

    Err(anyhow!("wait_until timed out after {:?}", timeout))
}

pub async fn create_archive_record_with_uploaded_file(
    app: &mut TestApp,
) -> anyhow::Result<(String, String, String)> {
    let user = TestUser::default_admin(&app.db_pool).await;
    app.login(&user).await;

    let template_body = serde_json::json!({
        "name": format!("s3-reliability-template-{}", Uuid::new_v4()),
        "category": "可靠性测试",
        "description": "S3可靠性测试模板",
        "schema": {
            "schema_def": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" }
                },
                "required": ["title"]
            }
        },
        "schema_files": [
            {
                "field": "attachment",
                "title": "附件",
                "file_config": {
                    "allowed_types": [".pdf"],
                    "quota": 1,
                    "max_size": 1048576,
                    "required": true
                }
            }
        ]
    });

    let template_res = app
        .post_create_template(&template_body)
        .await
        .json::<serde_json::Value>()
        .await?;
    check_response_code_and_message(&template_res, 201, "收集模板创建成功");
    let template_id = template_res["data"]["template_id"]
        .as_str()
        .ok_or_else(|| anyhow!("missing template_id"))?
        .to_string();

    let init_res = app
        .get_init_upload_session(&template_id)
        .await
        .json::<serde_json::Value>()
        .await?;
    check_response_code_and_message(&init_res, 201, "初始化上传会话成功");

    let session_id = init_res["data"]
        .as_str()
        .ok_or_else(|| anyhow!("missing session_id"))?;

    let filename = "report.pdf";
    let content = generate_a_dummy_file_content(16 * 1024);
    let presigned_body = serde_json::json!({
        "session_id": session_id,
        "field": "attachment",
        "filename": filename,
        "content_length": content.len(),
    });

    let presigned_res = app
        .post_presigned_upload_url(&template_id, &presigned_body)
        .await
        .json::<serde_json::Value>()
        .await?;
    check_response_code_and_message(&presigned_res, 201, "获取预签名上传URL成功");

    let upload_url = presigned_res["data"]["url"]
        .as_str()
        .ok_or_else(|| anyhow!("missing upload url"))?;
    let file_id = presigned_res["data"]["file_id"]
        .as_str()
        .ok_or_else(|| anyhow!("missing file_id"))?;

    let upload_res = app
        .put_to_upload_file(upload_url, &content, "application/pdf", filename)
        .await;
    if upload_res.status().as_u16() != 200 {
        return Err(anyhow!("failed to upload file to presigned url"));
    }

    let create_body = serde_json::json!({
        "data": {
            "title": "reliability-record",
            "attachment": file_id,
        },
        "session_id": session_id,
    });

    let create_res = app
        .post_create_archive_record(&template_id, &create_body)
        .await
        .json::<serde_json::Value>()
        .await?;
    check_response_code_and_message(&create_res, 201, "创建归档记录成功");

    let record_id = create_res["data"]["record_id"]
        .as_str()
        .ok_or_else(|| anyhow!("missing record_id"))?
        .to_string();

    let object_key: String = sqlx::query_scalar(
        r#"
        SELECT object_key
        FROM sys_file
        WHERE record_id = $1
        LIMIT 1
        "#,
    )
    .bind(Uuid::parse_str(&record_id)?)
    .fetch_one(&app.db_pool)
    .await?;

    Ok((template_id, record_id, object_key))
}

pub async fn count_outbox_rows(pool: &PgPool) -> anyhow::Result<i64> {
    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_outbox")
        .fetch_one(pool)
        .await?;
    Ok(cnt)
}

pub async fn count_outbox_rows_by_status(pool: &PgPool, status: i16) -> anyhow::Result<i64> {
    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_outbox WHERE status = $1")
        .bind(status)
        .fetch_one(pool)
        .await?;
    Ok(cnt)
}

pub async fn list_outbox_statuses(pool: &PgPool) -> anyhow::Result<Vec<i16>> {
    sqlx::query_scalar(
        r#"
        SELECT status
        FROM task_outbox
        ORDER BY id ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| anyhow!("failed to load outbox statuses: {}", e))
}

pub async fn list_outbox_commands(
    pool: &PgPool,
) -> anyhow::Result<Vec<cqwu_achievement_system::tasks::models::TaskCommand>> {
    let payloads: Vec<serde_json::Value> = sqlx::query_scalar(
        r#"
        SELECT payload
        FROM task_outbox
        ORDER BY id ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    payloads
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow!("failed to decode outbox payload: {}", e))
}
