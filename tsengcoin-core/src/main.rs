use std::{error::Error};

use tsengscript_interpreter::{execute, ExecutionResult};

use crate::tsengscript_interpreter::Token;

pub mod command;
pub mod wallet;
pub mod transaction;
pub mod block;
pub mod tsengscript_interpreter;
pub mod error;

fn main() -> Result<(), Box<dyn Error>> {
    tsengscript_test()?;

    Ok(())
}

fn tsengscript_test() -> Result<(), Box<dyn Error>> {
    let script = String::from("5 2 OP_ADD 7 OP_SUB 0 OP_EQUAL TRUE OP_EQUALVERIFY");
    let ExecutionResult{top, stack: _stack } = execute(&script)?;

    match top {
        None => println!("Result is None"),
        Some(Token::Bool(val)) => println!("Result is bool: {}", val),
        Some(Token::UByteSeq(bigint)) => println!("Result is bigint: {:x?}", bigint),
        Some(Token::Operator(_)) => println!("Result is an operator!")
    };

    Ok(())
}