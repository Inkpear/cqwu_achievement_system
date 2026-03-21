use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboxStatus {
    Pending,
    Running,
    Dead,
}

impl OutboxStatus {
    pub fn as_i16(self) -> i16 {
        match self {
            OutboxStatus::Pending => 0,
            OutboxStatus::Running => 1,
            OutboxStatus::Dead => 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// 最大重试次数
    pub max_retries: u32,
    /// 初始重试间隔
    pub backoff: Duration,
    /// 重试间隔倍数
    pub backoff_multiplier: u32,
    /// 最大重试间隔
    pub max_wait_duration: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 10,
            backoff: Duration::from_secs(1),
            backoff_multiplier: 2,
            max_wait_duration: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TaskCommand {
    DeleteArchiveObjects { object_keys: Vec<String> },
}

#[derive(Debug, Clone)]
pub struct QueuedTask {
    pub outbox_id: Option<i64>,
    pub command: TaskCommand,
}

impl QueuedTask {
    pub fn in_memory(command: TaskCommand) -> Self {
        Self {
            outbox_id: None,
            command,
        }
    }

    pub fn from_outbox(outbox_id: i64, command: TaskCommand) -> Self {
        Self {
            outbox_id: Some(outbox_id),
            command,
        }
    }
}

impl TaskCommand {
    pub fn command_type(&self) -> &'static str {
        match self {
            TaskCommand::DeleteArchiveObjects { .. } => "DeleteArchiveObjects",
        }
    }
}
