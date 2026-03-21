pub mod delete_archive_objects_command;

use crate::tasks::models::TaskCommand;

use super::executable::Executable;
use delete_archive_objects_command::DeleteArchiveObjectsCommand;

pub fn into_executable_command(command: TaskCommand) -> Box<dyn Executable> {
    match command {
        TaskCommand::DeleteArchiveObjects { object_keys } => {
            Box::new(DeleteArchiveObjectsCommand::new(object_keys))
        }
    }
}
