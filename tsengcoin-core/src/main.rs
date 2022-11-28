#![feature(thread_is_running)]
pub mod v1;

pub mod command;
pub mod wallet;
pub mod tsengscript_interpreter;
pub mod script_error;
pub mod top_level_commands;
pub mod session_commands;
pub mod difficulty;

use std::{error::Error, env};

use command::{dispatch_command};
use top_level_commands::make_command_map;

fn main() -> Result<(), Box<dyn Error>> {
    let command_map = make_command_map();
    let args: Vec<String> = env::args().collect();
    
    dispatch_command(&args[1..].to_vec(), &command_map, None);

    Ok(())
}