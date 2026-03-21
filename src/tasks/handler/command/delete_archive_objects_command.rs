use aws_sdk_s3::{
    error::{ProvideErrorMetadata, SdkError},
    operation::delete_objects::DeleteObjectsError,
};

use crate::{
    tasks::handler::{
        context::TaskExecutionContext,
        executable::{CommandExecutionError, CommandFuture, ErrorCategory, ErrorKind, Executable},
    },
    utils::s3_storage::DeleteObjectFailure,
};

pub struct DeleteArchiveObjectsCommand {
    pending_keys: Vec<String>,
}

impl DeleteArchiveObjectsCommand {
    pub fn new(object_keys: Vec<String>) -> Self {
        Self {
            pending_keys: object_keys,
        }
    }

    fn is_retryable_delete_object_failure(failure: &DeleteObjectFailure) -> bool {
        if failure.key.is_empty() || failure.key == "<unknown>" {
            return false;
        }

        match failure.code.as_deref() {
            Some(
                "SlowDown"
                | "Throttling"
                | "ThrottlingException"
                | "RequestTimeout"
                | "InternalError"
                | "ServiceUnavailable",
            ) => true,
            Some("AccessDenied" | "InvalidArgument" | "NoSuchBucket") => false,
            Some(_) => false,
            None => true,
        }
    }

    fn format_failures(failures: &[DeleteObjectFailure]) -> String {
        failures
            .iter()
            .map(|f| {
                format!(
                    "key={}, code={}, message={}",
                    f.key,
                    f.code.as_deref().unwrap_or("<none>"),
                    f.message.as_deref().unwrap_or("<none>")
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn classify_delete_archive_error(err: &anyhow::Error) -> (ErrorKind, ErrorCategory) {
        if let Some(sdk_error) = err.downcast_ref::<SdkError<DeleteObjectsError>>() {
            return match sdk_error {
                SdkError::TimeoutError(_) => (ErrorKind::Retryable, ErrorCategory::Timeout),
                SdkError::DispatchFailure(_) => (ErrorKind::Retryable, ErrorCategory::Network),
                SdkError::ResponseError(_) => (ErrorKind::Retryable, ErrorCategory::Network),
                SdkError::ServiceError(service_error) => {
                    let code = service_error.err().code().unwrap_or_default();
                    match code {
                        "SlowDown" | "Throttling" | "ThrottlingException" => {
                            (ErrorKind::Retryable, ErrorCategory::Throttled)
                        }
                        "RequestTimeout" => (ErrorKind::Retryable, ErrorCategory::Timeout),
                        "InternalError" => (ErrorKind::Retryable, ErrorCategory::Internal),
                        "ServiceUnavailable" => {
                            (ErrorKind::Retryable, ErrorCategory::ServiceUnavailable)
                        }
                        "AccessDenied" => (ErrorKind::NonRetryable, ErrorCategory::AccessDenied),
                        "InvalidArgument" => {
                            (ErrorKind::NonRetryable, ErrorCategory::InvalidArgument)
                        }
                        "NoSuchBucket" => {
                            (ErrorKind::NonRetryable, ErrorCategory::ResourceNotFound)
                        }
                        _ => (ErrorKind::NonRetryable, ErrorCategory::Unknown),
                    }
                }
                SdkError::ConstructionFailure(_) => {
                    (ErrorKind::NonRetryable, ErrorCategory::InvalidArgument)
                }
                _ => (ErrorKind::Retryable, ErrorCategory::Unknown),
            };
        }

        let message = err.to_string();
        if message.contains("构建删除对象失败") {
            return (ErrorKind::NonRetryable, ErrorCategory::InvalidArgument);
        }

        if message.contains("AccessDenied") {
            return (ErrorKind::NonRetryable, ErrorCategory::AccessDenied);
        }

        if message.contains("InvalidArgument") {
            return (ErrorKind::NonRetryable, ErrorCategory::InvalidArgument);
        }

        if message.contains("NoSuchBucket") {
            return (ErrorKind::NonRetryable, ErrorCategory::ResourceNotFound);
        }

        (ErrorKind::Retryable, ErrorCategory::Unknown)
    }
}

impl Executable for DeleteArchiveObjectsCommand {
    fn name(&self) -> &'static str {
        "delete_archive_objects"
    }

    fn execute<'a>(&'a mut self, context: &'a TaskExecutionContext) -> CommandFuture<'a> {
        Box::pin(async move {
            if self.pending_keys.is_empty() {
                return Ok(());
            }

            match context
                .s3_storage()
                .delete_objects_with_report(&self.pending_keys)
                .await
            {
                Ok(report) => {
                    if report.failed.is_empty() {
                        tracing::info!(
                            "delete_archive_objects succeeded, deleted_count={}",
                            report.deleted_keys.len()
                        );
                        return Ok(());
                    }

                    let (retryable_failures, non_retryable_failures): (Vec<_>, Vec<_>) = report
                        .failed
                        .into_iter()
                        .partition(Self::is_retryable_delete_object_failure);

                    let retryable_failed_keys = retryable_failures
                        .iter()
                        .map(|f| f.key.clone())
                        .collect::<Vec<_>>();

                    if !retryable_failed_keys.is_empty() {
                        self.pending_keys = retryable_failed_keys.clone();
                    }

                    if !non_retryable_failures.is_empty() {
                        let details = Self::format_failures(&non_retryable_failures);

                        if !retryable_failed_keys.is_empty() {
                            tracing::warn!(
                                "delete_archive_objects mixed partial failure: dropping non-retryable keys, retrying retryable subset, non_retryable_details={}, retryable_keys={:?}",
                                details,
                                retryable_failed_keys
                            );
                            return Err(CommandExecutionError::retryable(
                                ErrorCategory::ServiceUnavailable,
                                "delete_archive_objects mixed partial failure, retrying retryable failed keys only",
                                retryable_failed_keys,
                                None,
                            ));
                        }

                        return Err(CommandExecutionError::non_retryable(
                            ErrorCategory::InvalidArgument,
                            format!(
                                "delete_archive_objects failed with non-retryable errors: {}",
                                details
                            ),
                            non_retryable_failures
                                .iter()
                                .map(|f| f.key.clone())
                                .collect(),
                            None,
                        ));
                    }

                    if retryable_failed_keys.is_empty() {
                        return Ok(());
                    }

                    Err(CommandExecutionError::retryable(
                        ErrorCategory::ServiceUnavailable,
                        "delete_archive_objects partial failure, retrying failed keys only",
                        retryable_failed_keys,
                        None,
                    ))
                }
                Err(err) => {
                    let (kind, category) = Self::classify_delete_archive_error(&err);
                    let message = format!("delete_archive_objects request failed: {}", err);
                    match kind {
                        ErrorKind::Retryable => Err(CommandExecutionError::retryable(
                            category,
                            message,
                            self.pending_keys.clone(),
                            Some(err),
                        )),
                        ErrorKind::NonRetryable => Err(CommandExecutionError::non_retryable(
                            category,
                            message,
                            self.pending_keys.clone(),
                            Some(err),
                        )),
                    }
                }
            }
        })
    }
}
