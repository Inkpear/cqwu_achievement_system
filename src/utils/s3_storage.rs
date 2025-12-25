use anyhow::Context;
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{
    Client,
    config::Credentials,
    error::SdkError,
    operation::head_object::{HeadObjectError, HeadObjectOutput},
    presigning::PresigningConfig,
    types::MetadataDirective,
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

pub struct S3Storage {
    client: Client,
    bucket_name: String,
    sig_exp: std::time::Duration,
    view_exp: std::time::Duration,
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
            .metadata("original-filename", filename.as_ref())
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
