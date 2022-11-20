use std::{collections::HashMap, error::Error};

use ring::signature::KeyPair;

use crate::{command::{CommandMap, Command, CommandInvocation, Field, FieldType, Flag}, tsengscript_interpreter::{execute, ExecutionResult, Token}, wallet::{address_from_public_key, address_to_b58c, b58c_to_address, create_keypair, load_keypair}};

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

fn create_address(_command_name: &String, invocation: &CommandInvocation, _state: Option<()>) -> Result<(), Box<dyn Error>> {
    let path = invocation.get_field("keypair-path").unwrap();
    let password = invocation.get_field("password").unwrap();
    let keypair = create_keypair(&password, &path)?;

    let pubkey = keypair.public_key().as_ref();
    let address = address_from_public_key(&pubkey.to_vec());
    let encoded = address_to_b58c(&address.to_vec());

    println!("Created new keypair and saved it to {path}. Protect this file!");
    println!("Your new address is {}", encoded);

    Ok(())
}

fn test_load_keypair(_command_name: &String, invocation: &CommandInvocation, _state: Option<()>) -> Result<(), Box<dyn Error>> {
    let path = invocation.get_field("keypair-path").unwrap();
    let password = invocation.get_field("password").unwrap();
    let keypair = load_keypair(&password, &path)?;

    println!("Successfully loaded keypair");

    let pubkey = keypair.public_key().as_ref();
    let address = address_from_public_key(&pubkey.to_vec());
    let encoded = address_to_b58c(&address.to_vec());

    println!("Your address is {}", encoded);

    Ok(())
}

pub fn make_command_map() -> CommandMap<()> {
    let mut out: CommandMap<()> = HashMap::new();
    let run_script_cmd: Command<()> = Command {
        processor: run_script,
        expected_fields: vec![
            Field::new(
                "script",
                FieldType::Spaces(0),
                "Code written in TsengScript"
            )
        ],
        flags: vec![
            Flag::new("show-stack", "Print the contents of the stack when the program finishes")
        ],
        desc: String::from("Run a TsengScript program and see the output and stack trace")
    };
    let random_test_address_hex_cmd: Command<()> = Command {
        processor: random_test_address,
        expected_fields: vec![],
        flags: vec![],
        desc: String::from("Generate a random test TsengCoin address in hex")
    };
    let b58c_encode_cmd: Command<()> = Command {
        processor: b58c_encode,
        expected_fields: vec![
            Field::new(
                "hex-bytes",
                FieldType::Pos(0),
                "The hex string to be encoded. Do not include the '0x' prefix"
            )
        ],
        flags: vec![],
        desc: String::from("Encode a hex string in base58check. The hex string is treated as a TsengCoin address")
    };
    let b58c_decode_cmd: Command<()> = Command {
        processor: b58c_decode,
        expected_fields: vec![
            Field::new(
                "encoded",
                FieldType::Pos(0),
                "A string encoded in base58check"
            )
        ],
        flags: vec![],
        desc: String::from("Decode a base58check string to hex. The encoded string is treated as a TsengCoin address")
    };
    let create_address_cmd: Command<()> = Command {
        processor: create_address,
        expected_fields: vec![
            Field::new(
                "keypair-path",
                FieldType::Pos(0),
                "Path to a keypair file"
            ),
            Field::new(
                "password",
                FieldType::Spaces(1),
                "Password to the given keypair file"
            )
        ],
        flags: vec![],
        desc: String::from(
            "Create a TsengCoin address and lock it with a password. The file created by this command must be protected because it contains your private key"
        )
    };
    let test_load_keypair_cmd: Command<()> = Command {
        processor: test_load_keypair,
        expected_fields: vec![
            Field::new(
                "keypair-path",
                FieldType::Pos(0),
                "Path to a keypair file"
            ),
            Field::new(
                "password",
                FieldType::Spaces(1),
                "Password to the given keypair file"
            )
        ],
        flags: vec![],
        desc: String::from("Load a keypair file locked with a password and get the address out of it. The file is encrypted so this only works if you have the right password")
    };

    out.insert(String::from("run-script"), run_script_cmd);
    out.insert(String::from("random-test-address-hex"), random_test_address_hex_cmd);
    out.insert(String::from("b58c-encode"), b58c_encode_cmd);
    out.insert(String::from("b58c-decode"), b58c_decode_cmd);
    out.insert(String::from("create-address"), create_address_cmd);
    out.insert(String::from("test-load-keypair"), test_load_keypair_cmd);

    out
}

