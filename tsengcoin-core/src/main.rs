pub mod v1;
pub mod commands;

pub mod command;
pub mod wallet;
pub mod tsengscript_interpreter;
pub mod script_error;
pub mod difficulty;
pub mod hash;
pub mod banner;

use std::{error::Error, io};

use banner::{BANNER};



use command::{dispatch_command};
use commands::top_level::make_command_map;

fn main() -> Result<(), Box<dyn Error>> {
    // println!("{}", BANNER); -> Cool looking banner.
    let command_map = make_command_map();
    loop {
        println!("\nType 'help' to see a list of commands\n");
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer)?;
        buffer.pop();
        let str_array = buffer.split(" ").map(|s| s.to_string()).collect::<Vec<String>>();
        dispatch_command(&str_array, &command_map, None);
    }
    

    // Ok(())
}