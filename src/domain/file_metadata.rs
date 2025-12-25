use anyhow::Context;
use aws_sdk_s3::operation::head_object::HeadObjectOutput;

pub struct FileMetadata {
    pub filename: String,
    pub file_size: i64,
    pub mime_type: String,
}

impl FileMetadata {
    pub fn try_from_head(head_object_output: &HeadObjectOutput) -> Result<Self, anyhow::Error> {
        let raw_filename = head_object_output
            .metadata
            .as_ref()
            .and_then(|meta| meta.get("original-filename"))
            .ok_or(anyhow::anyhow!("文件名元数据丢失"))?;

        let filename = urlencoding::decode(raw_filename)
            .context("文件名解码失败")?
            .to_string();

        let mime_type = head_object_output
            .content_type
            .clone()
            .ok_or(anyhow::anyhow!("MIME类型丢失"))?;

        let file_size = head_object_output
            .content_length
            .ok_or(anyhow::anyhow!("文件大小丢失"))?;

        Ok(FileMetadata {
            filename,
            file_size,
            mime_type,
        })
    }
}
