pub mod correct;
pub mod note;

use anyhow::Result;
use crate::cli::{CorrectCommands, NoteCommands};

pub fn run_note(command: NoteCommands) -> Result<()> {
    match command {
        NoteCommands::Add { content, module, tag } => {
            note::add(&content, module.as_deref(), &tag)
        }
        NoteCommands::List { module, tag } => {
            note::list(module.as_deref(), tag.as_deref())
        }
    }
}

pub fn run_correct(command: CorrectCommands) -> Result<()> {
    match command {
        CorrectCommands::Add { description, module, tag } => {
            correct::add(&description, module.as_deref(), &tag)
        }
        CorrectCommands::List { since, module } => {
            correct::list(since.as_deref(), module.as_deref())
        }
    }
}
