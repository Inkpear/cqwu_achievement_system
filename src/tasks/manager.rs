use std::{sync::Arc, time::Duration};

use tokio::{
    sync::{mpsc, watch},
    task::{JoinHandle, JoinSet},
    time,
};

use sqlx::PgPool;

use crate::tasks::{dispatcher::TaskDispatcher, handler::HandleTaskCommand, models::QueuedTask};

pub struct TaskManager {
    stop_tx: watch::Sender<bool>,
    handles: Vec<JoinHandle<()>>,
    cmd_rx: Option<mpsc::Receiver<QueuedTask>>,
}

impl TaskManager {
    pub fn new_pair(queue_size: usize, pool: Arc<PgPool>) -> (Self, TaskDispatcher) {
        let (cmd_tx, cmd_rx) = mpsc::channel(queue_size);
        let task_dispatcher = TaskDispatcher::new(cmd_tx, pool);

        (
            Self {
                stop_tx: watch::channel(false).0,
                handles: Vec::new(),
                cmd_rx: Some(cmd_rx),
            },
            task_dispatcher,
        )
    }

    pub fn add_interval_task<F, Fut>(
        &mut self,
        task_name: impl Into<String>,
        task_fn: F,
        interval: Duration,
    ) where
        F: Fn() -> Fut + Send + 'static,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let mut stop_rx = self.stop_tx.subscribe();
        let task_name = task_name.into();

        let handle = tokio::spawn(async move {
            let task_fn = task_fn;
            let mut ticker = time::interval(interval);
            ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
            ticker.tick().await;

            loop {
                tokio::select! {
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            tracing::info!("stopping interval task {}", task_name);
                            break;
                        }
                    }

                    _ = ticker.tick() => {
                        tracing::info!("running interval task {}", task_name);
                        if let Err(e) = task_fn().await {
                            tracing::error!("interval task {} failed: {:?}", task_name, e);
                        }
                    }
                }
            }
        });

        self.handles.push(handle);
    }

    pub fn add_quiet_interval_task<F, Fut>(
        &mut self,
        task_name: impl Into<String>,
        task_fn: F,
        interval: Duration,
    ) where
        F: Fn() -> Fut + Send + 'static,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let mut stop_rx = self.stop_tx.subscribe();
        let task_name = task_name.into();

        let handle = tokio::spawn(async move {
            let task_fn = task_fn;
            let mut ticker = time::interval(interval);
            ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
            ticker.tick().await;

            loop {
                tokio::select! {
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            tracing::info!("stopping interval task {}", task_name);
                            break;
                        }
                    }

                    _ = ticker.tick() => {
                        if let Err(e) = task_fn().await {
                            tracing::error!("interval task {} failed: {:?}", task_name, e);
                        }
                    }
                }
            }
        });

        self.handles.push(handle);
    }

    pub fn add_command_worker<H>(&mut self, handler: H)
    where
        H: HandleTaskCommand + Send + Sync + 'static,
    {
        self.add_command_worker_with_concurrency(handler, 16);
    }

    pub fn add_command_worker_with_concurrency<H>(&mut self, handler: H, max_in_flight: usize)
    where
        H: HandleTaskCommand + Send + Sync + 'static,
    {
        let mut stop_rx = self.stop_tx.subscribe();
        let mut rx = self
            .cmd_rx
            .take()
            .expect("command worker already registered");
        let max_in_flight = max_in_flight.max(1);
        let handler = Arc::new(handler);

        let handle = tokio::spawn(async move {
            let mut in_flight = JoinSet::new();
            let mut stopping = false;

            loop {
                tokio::select! {
                    changed = stop_rx.changed(), if !stopping => {
                        if changed.is_err() || *stop_rx.borrow() {
                            tracing::info!("stopping command worker");
                            stopping = true;
                        }
                    }

                    cmd = rx.recv(), if !stopping && in_flight.len() < max_in_flight => {
                        match cmd {
                            Some(cmd) => {
                                let handler = handler.clone();
                                in_flight.spawn(async move { handler.handle(cmd).await });
                            }
                            None => stopping = true,
                        }
                    }

                    completed = in_flight.join_next(), if !in_flight.is_empty() => {
                        match completed {
                            Some(Ok(Ok(()))) => {}
                            Some(Ok(Err(e))) => {
                                tracing::error!("task command failed: {:?}", e);
                            }
                            Some(Err(e)) => {
                                tracing::error!("task command join error: {:?}", e);
                            }
                            None => {}
                        }
                    }

                    else => {
                        if stopping {
                            break;
                        }
                    }
                }

                if stopping && in_flight.is_empty() {
                    break;
                }
            }
        });

        self.handles.push(handle);
    }

    pub fn close_command_channel(&mut self) {
        if let Some(rx) = self.cmd_rx.as_mut() {
            rx.close();
        }
    }

    pub async fn shutdown(&mut self) {
        tracing::info!("starting task manager shutdown");
        let _ = self.stop_tx.send(true);

        for mut handle in self.handles.drain(..) {
            tokio::select! {
                res = &mut handle => {
                    if let Err(e) = res {
                        tracing::error!("task failed to shutdown gracefully: {:?}", e);
                    }
                }
                _ = time::sleep(Duration::from_secs(30)) => {
                    handle.abort();
                    tracing::error!("task did not shutdown within timeout; abort issued");
                }
            }
        }

        tracing::info!("task manager shutdown complete");
    }
}
