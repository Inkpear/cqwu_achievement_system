use std::{future::Future, pin::Pin};

use super::context::TaskExecutionContext;

pub type CommandFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), CommandExecutionError>> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Retryable,
    NonRetryable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Timeout,
    Throttled,
    Network,
    ServiceUnavailable,
    AccessDenied,
    InvalidArgument,
    ResourceNotFound,
    Internal,
    Unknown,
}

#[derive(Debug)]
pub struct CommandExecutionError {
    pub kind: ErrorKind,
    pub category: ErrorCategory,
    pub message: String,
    pub failed_keys: Vec<String>,
    pub source: Option<anyhow::Error>,
}

impl CommandExecutionError {
    pub fn retryable(
        category: ErrorCategory,
        message: impl Into<String>,
        failed_keys: Vec<String>,
        source: Option<anyhow::Error>,
    ) -> Self {
        Self {
            kind: ErrorKind::Retryable,
            category,
            message: message.into(),
            failed_keys,
            source,
        }
    }

    pub fn non_retryable(
        category: ErrorCategory,
        message: impl Into<String>,
        failed_keys: Vec<String>,
        source: Option<anyhow::Error>,
    ) -> Self {
        Self {
            kind: ErrorKind::NonRetryable,
            category,
            message: message.into(),
            failed_keys,
            source,
        }
    }

    pub fn into_anyhow(self) -> anyhow::Error {
        let details = if self.failed_keys.is_empty() {
            String::new()
        } else {
            format!(", failed_keys={:?}", self.failed_keys)
        };

        let base = anyhow::anyhow!(
            "command execution failed: kind={:?}, category={:?}, message={}{}",
            self.kind,
            self.category,
            self.message,
            details
        );

        if let Some(source) = self.source {
            base.context(source)
        } else {
            base
        }
    }
}

pub trait Executable: Send {
    fn name(&self) -> &'static str;
    fn execute<'a>(&'a mut self, context: &'a TaskExecutionContext) -> CommandFuture<'a>;
}
