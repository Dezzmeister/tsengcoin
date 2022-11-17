use std::error::Error;

use tsengscript_interpreter::execute;

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
    let script = String::from("7 5 2 OP_ADD 2 OP_SUB OP_SUB 2 FALSE FALSE OP_EQUAL");
    let res = execute(&script)?;

    match res {
        None => println!("Result is None"),
        Some(Token::Bool(val)) => println!("Result is bool: {}", val),
        Some(Token::ULiteralByteSeq(bigint)) => println!("Result is bigint: {:x?}", bigint),
        Some(Token::Operator(_)) => println!("Result is an operator!")
    };

    Ok(())
}