use anyhow::{Context, anyhow};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{
    Client,
    config::Credentials,
    error::SdkError,
    operation::head_object::{HeadObjectError, HeadObjectOutput},
    presigning::PresigningConfig,
    types::{Delete, MetadataDirective, ObjectIdentifier},
};
use secrecy::ExposeSecret;
use uuid::Uuid;

use crate::configuration::StorageSettings;

pub async fn build_s3_client(config: &StorageSettings) -> Client {
    let shared_config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(config.region.clone()))
        .endpoint_url(config.endpoint.clone())
        .credentials_provider(Credentials::new(
            config.access_key.clone(),
            config.secret_key.expose_secret(),
            None,
            None,
            "static",
        ))
        .load()
        .await;

    let s3_config = aws_sdk_s3::config::Builder::from(&shared_config)
        .force_path_style(true)
        .build();

    Client::from_conf(s3_config)
}

#[derive(Clone)]
pub struct S3Storage {
    client: Client,
    bucket_name: String,
    sig_exp: std::time::Duration,
    view_exp: std::time::Duration,
}

#[derive(Debug, Clone)]
pub struct DeleteObjectFailure {
    pub key: String,
    pub code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DeleteObjectsReport {
    pub deleted_keys: Vec<String>,
    pub failed: Vec<DeleteObjectFailure>,
}

impl DeleteObjectsReport {
    pub fn failed_keys(&self) -> Vec<String> {
        self.failed.iter().map(|f| f.key.clone()).collect()
    }
}

impl S3Storage {
    pub async fn from_config(settings: &StorageSettings) -> Self {
        let client = build_s3_client(settings).await;
        S3Storage {
            client,
            bucket_name: settings.bucket_name.clone(),
            sig_exp: std::time::Duration::from_secs(settings.sig_exp_seconds),
            view_exp: std::time::Duration::from_secs(settings.view_exp_seconds),
        }
    }

    pub async fn generate_presigned_url(
        &self,
        object_key: &str,
        content_type: &str,
        content_length: i64,
        filename: &str,
    ) -> Result<String, anyhow::Error> {
        let presigned_config = PresigningConfig::expires_in(self.sig_exp)?;
        let filename = urlencoding::encode(filename);

        let presigned_request = self
            .client
            .put_object()
            .bucket(&self.bucket_name)
            .key(object_key)
            .content_type(content_type)
            .content_length(content_length)
            .metadata("original-filename", filename)
            .presigned(presigned_config)
            .await?;

        Ok(presigned_request.uri().to_string())
    }

    pub async fn generate_view_url(
        &self,
        filename: &str,
        object_key: &str,
    ) -> Result<String, anyhow::Error> {
        let encoded_filename = urlencoding::encode(filename);
        let input = format!("attachment; filename=\"{}\"", encoded_filename);
        let presigned_config = PresigningConfig::expires_in(self.view_exp)?;

        let presigned_request = self
            .client
            .get_object()
            .bucket(&self.bucket_name)
            .key(object_key)
            .response_content_disposition(input)
            .presigned(presigned_config)
            .await?;

        Ok(presigned_request.uri().to_string())
    }

    pub async fn copy_source_to_dest(
        &self,
        source_key: &str,
        dest_key: &str,
    ) -> Result<(), anyhow::Error> {
        let copy_source = format!("{}/{}", self.bucket_name, source_key);
        let copy_source = urlencoding::encode(&copy_source);

        self.client
            .copy_object()
            .bucket(&self.bucket_name)
            .key(dest_key)
            .copy_source(copy_source.as_ref())
            .metadata_directive(MetadataDirective::Copy)
            .send()
            .await
            .context("复制远程文件失败")?;

        Ok(())
    }

    pub async fn delete_object(&self, object_key: &str) -> Result<(), anyhow::Error> {
        self.client
            .delete_object()
            .bucket(&self.bucket_name)
            .key(object_key)
            .send()
            .await
            .context("删除远程文件失败")?;

        Ok(())
    }

    pub async fn delete_objects(&self, object_keys: &[String]) -> Result<(), anyhow::Error> {
        let report = self.delete_objects_with_report(object_keys).await?;
        if report.failed.is_empty() {
            return Ok(());
        }

        let details = report
            .failed
            .iter()
            .map(|e| {
                format!(
                    "key={}, code={}, message={}",
                    e.key,
                    e.code.clone().unwrap_or_else(|| "<none>".to_string()),
                    e.message.clone().unwrap_or_else(|| "<none>".to_string())
                )
            })
            .collect::<Vec<_>>()
            .join("; ");

        Err(anyhow!("批量删除远程文件出现部分失败: {}", details))
    }

    pub async fn delete_objects_with_report(
        &self,
        object_keys: &[String],
    ) -> Result<DeleteObjectsReport, anyhow::Error> {
        if object_keys.is_empty() {
            return Ok(DeleteObjectsReport::default());
        }

        const MAX_DELETE_PER_REQUEST: usize = 1000;
        let mut report = DeleteObjectsReport::default();

        for chunk in object_keys.chunks(MAX_DELETE_PER_REQUEST) {
            let mut delete_objects = Vec::with_capacity(chunk.len());
            for key in chunk {
                let object_identifier = ObjectIdentifier::builder()
                    .key(key)
                    .build()
                    .map_err(|e| anyhow!("构建删除对象失败, key={}, error={}", key, e))?;
                delete_objects.push(object_identifier);
            }

            let output = self
                .client
                .delete_objects()
                .bucket(&self.bucket_name)
                .delete(
                    Delete::builder()
                        .set_objects(Some(delete_objects))
                        .build()?,
                )
                .send()
                .await
                .context("批量删除远程文件失败")?;

            report.deleted_keys.extend(
                output
                    .deleted()
                    .iter()
                    .filter_map(|d| d.key())
                    .map(ToString::to_string),
            );

            report
                .failed
                .extend(output.errors().iter().map(|e| DeleteObjectFailure {
                    key: e.key().unwrap_or("<unknown>").to_string(),
                    code: e.code().map(ToString::to_string),
                    message: e.message().map(ToString::to_string),
                }));
        }

        Ok(report)
    }

    pub async fn list_object_keys_with_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<String>, anyhow::Error> {
        self.list_object_keys_with_prefix_older_than(prefix, std::time::Duration::ZERO)
            .await
    }

    pub async fn list_object_keys_with_prefix_older_than(
        &self,
        prefix: &str,
        min_age: std::time::Duration,
    ) -> Result<Vec<String>, anyhow::Error> {
        let mut keys = Vec::new();
        let mut continuation_token: Option<String> = None;
        let now_epoch_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .context("系统时钟异常，无法计算对象年龄")?
            .as_secs() as i64;
        let min_age_secs = min_age.as_secs() as i64;

        loop {
            let mut req = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket_name)
                .prefix(prefix);

            if let Some(token) = continuation_token.as_deref() {
                req = req.continuation_token(token);
            }

            let output = req.send().await.context("列举远程对象失败")?;

            keys.extend(output.contents().iter().filter_map(|obj| {
                let key = obj.key()?;
                if min_age_secs == 0 {
                    return Some(key.to_string());
                }

                // Only cleanup sufficiently old objects to avoid racing with in-flight writes.
                let last_modified_secs = obj.last_modified()?.secs();
                if now_epoch_secs.saturating_sub(last_modified_secs) >= min_age_secs {
                    Some(key.to_string())
                } else {
                    None
                }
            }));

            if output.is_truncated().unwrap_or(false) {
                continuation_token = output.next_continuation_token().map(ToString::to_string);
                if continuation_token.is_none() {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(keys)
    }

    pub async fn get_head_object_output(
        &self,
        object_key: &str,
    ) -> Result<HeadObjectOutput, SdkError<HeadObjectError>> {
        let head_object_output = self
            .client
            .head_object()
            .bucket(&self.bucket_name)
            .key(object_key)
            .send()
            .await?;

        Ok(head_object_output)
    }

    pub async fn object_exists(&self, object_key: &str) -> Result<bool, anyhow::Error> {
        match self.get_head_object_output(object_key).await {
            Ok(_) => Ok(true),
            Err(e) => match e.into_service_error() {
                HeadObjectError::NotFound(_) => Ok(false),
                other => Err(anyhow::anyhow!("检查对象是否存在时发生错误: {}", other)),
            },
        }
    }
}

pub fn build_upload_session_key(session_id: &Uuid) -> String {
    format!("archive:upload_session:{}", session_id)
}

pub fn build_temp_object_key(session_id: &Uuid, file_id: &Uuid) -> String {
    format!("temp/{}/{}", session_id, file_id)
}

pub fn build_archive_dest_key(record_id: &Uuid, file_id: &Uuid) -> String {
    format!("archive/{}/{}", record_id, file_id)
}

pub fn build_avatar_dest_key(user_id: &Uuid) -> String {
    format!("avatar/{}", user_id)
}

pub fn build_temp_avatar_key(file_id: &Uuid) -> String {
    format!("temp/avatar/{}", file_id)
}
