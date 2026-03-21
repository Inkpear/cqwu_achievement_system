pub mod command;
pub mod command_runner;
pub mod context;
pub mod default_handler;
pub mod executable;

use crate::tasks::models::QueuedTask;

pub use default_handler::DefaultTaskHandler;

pub trait HandleTaskCommand {
    fn handle(&self, task: QueuedTask) -> impl Future<Output = anyhow::Result<()>> + Send;
}
