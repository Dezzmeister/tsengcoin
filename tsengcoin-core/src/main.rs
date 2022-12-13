#![feature(thread_is_running)]
pub mod commands;
pub mod gui;
pub mod v1;

pub mod command;
pub mod difficulty;
pub mod hash;
pub mod script_error;
pub mod tsengscript_interpreter;
pub mod wallet;

use std::{env, error::Error};

use command::dispatch_command;
use commands::top_level::make_command_map;

fn main() -> Result<(), Box<dyn Error>> {
    let command_map = make_command_map();
    let args: Vec<String> = env::args().collect();

    dispatch_command(&args[1..].to_vec(), &command_map, None);

    Ok(())
}
