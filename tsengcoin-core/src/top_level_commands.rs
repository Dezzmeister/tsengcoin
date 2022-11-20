use std::{collections::HashMap, error::Error};

use crate::{command::{CommandMap, Command, CommandInvocation, Field, FieldType}, tsengscript_interpreter::{execute, ExecutionResult, Token}};

fn run_script(_command_name: &String, invocation: &CommandInvocation, _state: Option<()>) -> Result<(), Box<dyn Error>> {
    let script = invocation.get_field("script").unwrap();
    let show_stack = invocation.get_flag("show-stack");
    let ExecutionResult{top, stack } = execute(&script)?;

    match top {
        None => println!("Stack was empty"),
        Some(Token::Bool(val)) => println!("Bool: {}", val),
        Some(Token::UByteSeq(bigint)) => println!("UByteSeq: {}", bigint),
        Some(Token::Operator(_)) => println!("Result is an operator!")
    };

    if show_stack {
        println!("Stack: {:?}", stack);
    }

    Ok(())
}

pub fn make_command_map() -> CommandMap<()> {
    let mut out: CommandMap<()> = HashMap::new();
    let run_script_cmd: Command<()> = Command {
        processor: run_script,
        expected_fields: vec![
            Field::new("script", FieldType::Spaces(0))
        ]
    };

    out.insert(String::from("run-script"), run_script_cmd);

    out
}

