use std::{collections::HashMap, error::Error};

use crate::{command::{CommandMap, Command, CommandInvocation, Field, FieldType}, tsengscript_interpreter::{execute, ExecutionResult, Token}, wallet::{address_from_public_key, address_to_b58c, b58c_to_address}};

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

fn random_test_address(_command_name: &String, _invocation: &CommandInvocation, _state: Option<()>) -> Result<(), Box<dyn Error>> {
    let rand_bytes: [u8; 32] = rand::random();
    let address = address_from_public_key(&rand_bytes.to_vec());

    println!("{}", hex::encode(&address));

    Ok(())
}

fn b58c_encode(_command_name: &String, invocation: &CommandInvocation, _state: Option<()>) -> Result<(), Box<dyn Error>> {
    let raw = invocation.get_field("hex-bytes").unwrap();
    let bytes = hex::decode(raw)?;
    let encoded = address_to_b58c(&bytes);

    println!("{}", encoded);

    Ok(())
}

fn b58c_decode(_command_name: &String, invocation: &CommandInvocation, _state: Option<()>) -> Result<(), Box<dyn Error>> {
    let raw = invocation.get_field("encoded").unwrap();
    let decoded = b58c_to_address(raw)?;
    let hex_str = hex::encode(decoded);

    println!("{}", hex_str);

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
    let random_test_address_cmd: Command<()> = Command {
        processor: random_test_address,
        expected_fields: vec![]
    };
    let b58c_encode_cmd: Command<()> = Command {
        processor: b58c_encode,
        expected_fields: vec![
            Field::new("hex-bytes", FieldType::Pos(0))
        ]
    };
    let b58c_decode_cmd: Command<()> = Command {
        processor: b58c_decode,
        expected_fields: vec![
            Field::new("encoded", FieldType::Pos(0))
        ]
    };

    out.insert(String::from("run-script"), run_script_cmd);
    out.insert(String::from("random-test-address"), random_test_address_cmd);
    out.insert(String::from("b58c-encode"), b58c_encode_cmd);
    out.insert(String::from("b58c-decode"), b58c_decode_cmd);

    out
}

