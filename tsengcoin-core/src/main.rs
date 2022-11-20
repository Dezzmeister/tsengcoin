use std::{error::Error, env};

use command::{dispatch_command};
use top_level_commands::make_command_map;

pub mod command;
pub mod wallet;
pub mod transaction;
pub mod block;
pub mod tsengscript_interpreter;
pub mod error;
pub mod top_level_commands;

fn main() -> Result<(), Box<dyn Error>> {
    let command_map = make_command_map();
    let args: Vec<String> = env::args().collect();
    
    dispatch_command(&args[1..].to_vec(), &command_map, None);

    Ok(())
}