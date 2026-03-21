use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::mpsc;

use crate::tasks::models::{OutboxStatus, QueuedTask, TaskCommand};

static DROPPED_BY_QUEUE_FULL: AtomicU64 = AtomicU64::new(0);
static DROPPED_BY_QUEUE_CLOSED: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct TaskDispatcher {
    tx: mpsc::Sender<QueuedTask>,
    pool: Arc<PgPool>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OutboxRecoveryResult {
    pub reclaimed_running: u64,
    pub recovered_pending: usize,
}

impl TaskDispatcher {
    pub fn new(tx: mpsc::Sender<QueuedTask>, pool: Arc<PgPool>) -> Self {
        Self { tx, pool }
    }

    pub async fn submit(&self, command: TaskCommand) -> anyhow::Result<()> {
        let payload = serde_json::to_value(&command)?;

        let outbox_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO task_outbox (command_type, payload, status, next_retry_at, updated_at)
            VALUES ($1, $2, $3, NOW(), NOW())
            RETURNING id
            "#,
        )
        .bind(command.command_type())
        .bind(payload)
        .bind(OutboxStatus::Pending.as_i16())
        .fetch_one(self.pool.as_ref())
        .await?;

        self.try_enqueue_outbox_task(outbox_id, command).await;

        Ok(())
    }

    pub async fn pump_outbox_once(&self, batch_size: usize) -> anyhow::Result<usize> {
        if batch_size == 0 {
            return Ok(0);
        }

        let rows = sqlx::query_as::<_, (i64, serde_json::Value)>(
            r#"
            WITH picked AS (
                SELECT id
                FROM task_outbox
                WHERE status = $1
                  AND next_retry_at <= NOW()
                ORDER BY id ASC
                LIMIT $2
                                FOR UPDATE SKIP LOCKED
            )
            UPDATE task_outbox t
            SET status = $3,
                updated_at = NOW()
            FROM picked
            WHERE t.id = picked.id
                            AND t.status = $1
            RETURNING t.id, t.payload
            "#,
        )
        .bind(OutboxStatus::Pending.as_i16())
        .bind(batch_size as i64)
        .bind(OutboxStatus::Running.as_i16())
        .fetch_all(self.pool.as_ref())
        .await?;

        if rows.is_empty() {
            return Ok(0);
        }

        let mut enqueued = 0;
        let mut remaining_ids = Vec::new();
        let mut rows_iter = rows.into_iter();

        while let Some((id, payload)) = rows_iter.next() {
            let command: TaskCommand = match serde_json::from_value(payload) {
                Ok(v) => v,
                Err(err) => {
                    self.mark_outbox_dead(id, format!("invalid outbox payload: {}", err))
                        .await?;
                    continue;
                }
            };

            match self.try_reserve_slot(command.command_type()) {
                Ok(permit) => {
                    permit.send(QueuedTask::from_outbox(id, command));
                    enqueued += 1;
                }
                Err(QueueReserveError::Full) | Err(QueueReserveError::Closed) => {
                    remaining_ids.push(id);
                    remaining_ids.extend(rows_iter.map(|(rest_id, _)| rest_id));
                    break;
                }
            }
        }

        if !remaining_ids.is_empty() {
            self.requeue_outbox_rows(&remaining_ids, Duration::from_secs(1))
                .await?;
        }

        Ok(enqueued)
    }

    pub async fn requeue_retryable_outbox(
        &self,
        outbox_id: i64,
        error_message: &str,
        delay: Duration,
    ) -> anyhow::Result<()> {
        let delay_seconds = delay.as_secs().min(i64::MAX as u64) as i64;
        sqlx::query(
            r#"
            UPDATE task_outbox
            SET status = $1,
                attempts = attempts + 1,
                next_retry_at = NOW() + ($2 * INTERVAL '1 second'),
                last_error = $3,
                updated_at = NOW()
            WHERE id = $4
            "#,
        )
        .bind(OutboxStatus::Pending.as_i16())
        .bind(delay_seconds)
        .bind(error_message)
        .bind(outbox_id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    pub async fn mark_outbox_dead(
        &self,
        outbox_id: i64,
        error_message: String,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE task_outbox
            SET status = $1,
                attempts = attempts + 1,
                last_error = $2,
                updated_at = NOW()
            WHERE id = $3
            "#,
        )
        .bind(OutboxStatus::Dead.as_i16())
        .bind(error_message)
        .bind(outbox_id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    pub async fn delete_outbox_row(&self, outbox_id: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM task_outbox WHERE id = $1")
            .bind(outbox_id)
            .execute(self.pool.as_ref())
            .await?;
        Ok(())
    }

    pub async fn recover_outbox_tasks(
        &self,
        running_timeout: Duration,
    ) -> anyhow::Result<OutboxRecoveryResult> {
        let reclaimed_running = self.reclaim_stale_running_outbox(running_timeout).await?;

        let rows = sqlx::query_as::<_, (i64, serde_json::Value)>(
            r#"
            SELECT id, payload
            FROM task_outbox
            WHERE status = $1
              AND next_retry_at <= NOW()
            ORDER BY id ASC
            "#,
        )
        .bind(OutboxStatus::Pending.as_i16())
        .fetch_all(self.pool.as_ref())
        .await?;

        let total = rows.len();
        for (id, payload) in rows {
            let command: TaskCommand = serde_json::from_value(payload)?;
            self.try_enqueue_outbox_task(id, command).await;
        }

        Ok(OutboxRecoveryResult {
            reclaimed_running,
            recovered_pending: total,
        })
    }

    pub async fn reclaim_stale_running_outbox(&self, timeout: Duration) -> anyhow::Result<u64> {
        if timeout.is_zero() {
            return Ok(0);
        }

        let timeout_seconds = timeout.as_secs().min(i64::MAX as u64) as i64;
        let result = sqlx::query(
            r#"
            UPDATE task_outbox
            SET status = $1,
                next_retry_at = NOW(),
                last_error = CASE
                    WHEN last_error IS NULL OR last_error = '' THEN $2
                    ELSE last_error || E'\n' || $2
                END,
                updated_at = NOW()
            WHERE status = $3
              AND updated_at < NOW() - ($4 * INTERVAL '1 second')
            "#,
        )
        .bind(OutboxStatus::Pending.as_i16())
        .bind("reclaimed stale running task during recovery")
        .bind(OutboxStatus::Running.as_i16())
        .bind(timeout_seconds)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected())
    }

    pub fn queue_in_memory(&self, command: TaskCommand) -> anyhow::Result<()> {
        let command_type = command.command_type();
        let permit = self
            .try_reserve_slot(command_type)
            .map_err(queue_error_to_anyhow)?;
        permit.send(QueuedTask::in_memory(command));
        Ok(())
    }

    async fn try_enqueue_outbox_task(&self, outbox_id: i64, command: TaskCommand) {
        match self.try_reserve_slot(command.command_type()) {
            Ok(permit) => {
                let rows_affected = sqlx::query(
                    r#"
                    UPDATE task_outbox
                    SET status = $1,
                        updated_at = NOW()
                    WHERE id = $2
                      AND status = $3
                    "#,
                )
                .bind(OutboxStatus::Running.as_i16())
                .bind(outbox_id)
                .bind(OutboxStatus::Pending.as_i16())
                .execute(self.pool.as_ref())
                .await;

                match rows_affected {
                    Ok(res) if res.rows_affected() == 1 => {
                        permit.send(QueuedTask::from_outbox(outbox_id, command));
                    }
                    Ok(_) => {
                        tracing::debug!(
                            "skip immediate enqueue because outbox row was already claimed, outbox_id={}",
                            outbox_id
                        );
                    }
                    Err(err) => {
                        tracing::warn!(
                            "failed to mark outbox row running before immediate enqueue, outbox_id={}, error={:?}",
                            outbox_id,
                            err
                        );
                    }
                }
            }
            Err(QueueReserveError::Full) => {
                let dropped = DROPPED_BY_QUEUE_FULL.fetch_add(1, Ordering::Relaxed) + 1;
                tracing::warn!(
                    "in-memory task cache is full, task will stay in outbox and be pulled later, command_type={}, queue_capacity={}, deferred_total_full={}",
                    command.command_type(),
                    self.tx.max_capacity(),
                    dropped
                );
            }
            Err(QueueReserveError::Closed) => {
                let dropped = DROPPED_BY_QUEUE_CLOSED.fetch_add(1, Ordering::Relaxed) + 1;
                tracing::warn!(
                    "in-memory task cache is closed, task will stay in outbox and be pulled later, command_type={}, deferred_total_closed={}",
                    command.command_type(),
                    dropped
                );
            }
        }
    }

    async fn requeue_outbox_rows(&self, outbox_ids: &[i64], delay: Duration) -> anyhow::Result<()> {
        if outbox_ids.is_empty() {
            return Ok(());
        }

        let delay_seconds = delay.as_secs().min(i64::MAX as u64) as i64;
        sqlx::query(
            r#"
            UPDATE task_outbox
            SET status = $1,
                next_retry_at = NOW() + ($2 * INTERVAL '1 second'),
                updated_at = NOW()
            WHERE id = ANY($3)
            "#,
        )
        .bind(OutboxStatus::Pending.as_i16())
        .bind(delay_seconds)
        .bind(outbox_ids)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    fn try_reserve_slot(
        &self,
        command_type: &'static str,
    ) -> Result<tokio::sync::mpsc::Permit<'_, QueuedTask>, QueueReserveError> {
        match self.tx.try_reserve() {
            Ok(permit) => Ok(permit),
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                tracing::debug!(
                    "task command could not reserve queue slot because queue is full, command_type={}, queue_capacity={}",
                    command_type,
                    self.tx.max_capacity()
                );
                Err(QueueReserveError::Full)
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                tracing::debug!(
                    "task command could not reserve queue slot because queue is closed, command_type={}",
                    command_type
                );
                Err(QueueReserveError::Closed)
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum QueueReserveError {
    Full,
    Closed,
}

fn queue_error_to_anyhow(err: QueueReserveError) -> anyhow::Error {
    match err {
        QueueReserveError::Full => anyhow::anyhow!("task command queue is full"),
        QueueReserveError::Closed => anyhow::anyhow!("task command queue is closed"),
    }
}
