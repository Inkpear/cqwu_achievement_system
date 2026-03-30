use std::time::Duration;

use crate::tasks::dispatcher::TaskDispatcher;

pub async fn pull_outbox_tasks(
    dispatcher: &TaskDispatcher,
    pull_batch_size: usize,
    running_timeout: Duration,
) -> anyhow::Result<()> {
    let reclaimed = dispatcher
        .reclaim_stale_running_outbox(running_timeout)
        .await?;
    if reclaimed > 0 {
        tracing::warn!("reclaimed {} stale running task(s) from outbox", reclaimed);
    }

    let pulled = dispatcher.pump_outbox_once(pull_batch_size).await?;
    if pulled > 0 {
        tracing::info!("pulled {} task(s) from outbox", pulled);
    }

    Ok(())
}
