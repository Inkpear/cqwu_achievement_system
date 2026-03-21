use crate::tasks::models::RetryConfig;

use super::{
    context::TaskExecutionContext,
    executable::{CommandExecutionError, ErrorKind, Executable},
};

pub async fn run_executable_with_retry(
    command: &mut dyn Executable,
    context: &TaskExecutionContext,
    retry_config: RetryConfig,
) -> Result<(), CommandExecutionError> {
    let mut retries = 0;
    let mut backoff = retry_config.backoff;

    loop {
        match command.execute(context).await {
            Ok(()) => {
                tracing::info!("task command {} succeeded", command.name());
                return Ok(());
            }
            Err(err) if err.kind == ErrorKind::NonRetryable => {
                tracing::error!(
                    "task command {} failed with non-retryable error: category={:?}, message={}, failed_keys={:?}",
                    command.name(),
                    err.category,
                    err.message,
                    err.failed_keys
                );
                return Err(err);
            }
            Err(err) => {
                if retries >= retry_config.max_retries {
                    tracing::error!(
                        "task command {} exceeded max retries: category={:?}, message={}, failed_keys={:?}",
                        command.name(),
                        err.category,
                        err.message,
                        err.failed_keys
                    );
                    return Err(err);
                }

                retries += 1;
                tracing::warn!(
                    "task command {} failed, retrying attempt {}/{}, category={:?}, message={}, failed_keys={:?}",
                    command.name(),
                    retries,
                    retry_config.max_retries,
                    err.category,
                    err.message,
                    err.failed_keys
                );

                tokio::time::sleep(backoff).await;
                backoff = std::cmp::min(
                    backoff * retry_config.backoff_multiplier,
                    retry_config.max_wait_duration,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Arc, time::Duration};

    use secrecy::SecretString;
    use sqlx::postgres::PgPoolOptions;

    use crate::{
        configuration::{RedisSettings, StorageSettings},
        tasks::{
            handler::{
                command_runner::run_executable_with_retry,
                context::TaskExecutionContext,
                executable::{CommandExecutionError, CommandFuture, ErrorCategory, Executable},
            },
            models::RetryConfig,
        },
        utils::{redis_cache::RedisCache, s3_storage::S3Storage},
    };

    #[derive(Clone, Copy)]
    enum Step {
        Ok,
        Retryable,
        NonRetryable,
    }

    struct FakeCommand {
        name: &'static str,
        steps: VecDeque<Step>,
        attempts: usize,
    }

    impl FakeCommand {
        fn new(name: &'static str, steps: Vec<Step>) -> Self {
            Self {
                name,
                steps: steps.into(),
                attempts: 0,
            }
        }
    }

    impl Executable for FakeCommand {
        fn name(&self) -> &'static str {
            self.name
        }

        fn execute<'a>(&'a mut self, _context: &'a TaskExecutionContext) -> CommandFuture<'a> {
            Box::pin(async move {
                self.attempts += 1;
                match self.steps.pop_front().unwrap_or(Step::Ok) {
                    Step::Ok => Ok(()),
                    Step::Retryable => Err(CommandExecutionError::retryable(
                        ErrorCategory::Network,
                        "retryable test error",
                        vec!["k1".to_string()],
                        None,
                    )),
                    Step::NonRetryable => Err(CommandExecutionError::non_retryable(
                        ErrorCategory::InvalidArgument,
                        "non-retryable test error",
                        vec!["k1".to_string()],
                        None,
                    )),
                }
            })
        }
    }

    async fn build_test_context() -> TaskExecutionContext {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@127.0.0.1/test")
            .expect("failed to build lazy pg pool");

        let redis = RedisCache::from_config(&RedisSettings {
            uri: "redis://127.0.0.1/".to_string(),
        });

        let s3 = S3Storage::from_config(&StorageSettings {
            endpoint: "http://127.0.0.1:9000".to_string(),
            region: "us-east-1".to_string(),
            access_key: "test".to_string(),
            secret_key: SecretString::from("test".to_string()),
            bucket_name: "test-bucket".to_string(),
            sig_exp_seconds: 60,
            view_exp_seconds: 60,
        })
        .await;

        TaskExecutionContext::new(Arc::new(pool), Arc::new(s3), Arc::new(redis))
    }

    fn fast_retry_config(max_retries: u32) -> RetryConfig {
        RetryConfig {
            max_retries,
            backoff: Duration::from_millis(1),
            backoff_multiplier: 1,
            max_wait_duration: Duration::from_millis(1),
        }
    }

    #[tokio::test]
    async fn should_retry_then_succeed() {
        let context = build_test_context().await;
        let mut command =
            FakeCommand::new("fake", vec![Step::Retryable, Step::Retryable, Step::Ok]);

        let result = run_executable_with_retry(&mut command, &context, fast_retry_config(3)).await;

        assert!(result.is_ok());
        assert_eq!(command.attempts, 3);
    }

    #[tokio::test]
    async fn should_not_retry_for_non_retryable_error() {
        let context = build_test_context().await;
        let mut command = FakeCommand::new("fake", vec![Step::NonRetryable]);

        let result = run_executable_with_retry(&mut command, &context, fast_retry_config(3)).await;

        assert!(result.is_err());
        assert_eq!(command.attempts, 1);
    }

    #[tokio::test]
    async fn should_stop_when_max_retries_reached() {
        let context = build_test_context().await;
        let mut command = FakeCommand::new(
            "fake",
            vec![Step::Retryable, Step::Retryable, Step::Retryable],
        );

        let result = run_executable_with_retry(&mut command, &context, fast_retry_config(2)).await;

        assert!(result.is_err());
        assert_eq!(command.attempts, 3);
    }
}
