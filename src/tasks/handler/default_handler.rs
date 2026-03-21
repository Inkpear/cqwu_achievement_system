use std::sync::Arc;

use sqlx::PgPool;

use crate::{
    tasks::{
        dispatcher::TaskDispatcher,
        handler::{
            HandleTaskCommand, command::into_executable_command,
            command_runner::run_executable_with_retry, context::TaskExecutionContext,
            executable::ErrorKind,
        },
        models::{QueuedTask, RetryConfig},
    },
    utils::{redis_cache::RedisCache, s3_storage::S3Storage},
};

pub struct DefaultTaskHandler {
    context: TaskExecutionContext,
    retry_config: RetryConfig,
    dispatcher: TaskDispatcher,
}

impl DefaultTaskHandler {
    pub fn new(
        dispatcher: TaskDispatcher,
        pool: Arc<PgPool>,
        s3_storage: Arc<S3Storage>,
        redis_cache: Arc<RedisCache>,
        retry_config: RetryConfig,
    ) -> Self {
        Self {
            context: TaskExecutionContext::new(pool, s3_storage, redis_cache),
            retry_config,
            dispatcher,
        }
    }
}

impl HandleTaskCommand for DefaultTaskHandler {
    fn handle(&self, task: QueuedTask) -> impl Future<Output = anyhow::Result<()>> + Send {
        let context = self.context.clone();
        let retry_config = self.retry_config.clone();
        let dispatcher = self.dispatcher.clone();

        async move {
            let outbox_id = task.outbox_id;
            let mut executable = into_executable_command(task.command);
            let command_result =
                run_executable_with_retry(executable.as_mut(), &context, retry_config).await;

            match command_result {
                Ok(()) => {
                    if let Some(id) = outbox_id {
                        dispatcher.delete_outbox_row(id).await?;
                    }
                    Ok(())
                }
                Err(err) => {
                    if let Some(id) = outbox_id {
                        let error_message = format!(
                            "category={:?}, message={}, failed_keys={:?}",
                            err.category, err.message, err.failed_keys
                        );

                        match err.kind {
                            ErrorKind::Retryable => {
                                if let Err(update_err) = dispatcher
                                    .requeue_retryable_outbox(
                                        id,
                                        &error_message,
                                        std::time::Duration::from_secs(5),
                                    )
                                    .await
                                {
                                    tracing::error!(
                                        "failed to requeue outbox row for retry, outbox_id={}, error={:?}",
                                        id,
                                        update_err
                                    );
                                    return Err(err.into_anyhow().context(format!(
                                        "failed to requeue outbox row {}: {}",
                                        id, update_err
                                    )));
                                }
                            }
                            ErrorKind::NonRetryable => {
                                if let Err(update_err) =
                                    dispatcher.mark_outbox_dead(id, error_message).await
                                {
                                    tracing::error!(
                                        "failed to mark outbox row dead, outbox_id={}, error={:?}",
                                        id,
                                        update_err
                                    );
                                    return Err(err.into_anyhow().context(format!(
                                        "failed to mark outbox row {} dead: {}",
                                        id, update_err
                                    )));
                                }
                            }
                        }
                    }

                    Err(err.into_anyhow())
                }
            }
        }
    }
}
