use std::{collections::HashMap};
use num_bigint::BigUint;

use crate::error::Result;
use crate::error::ErrorKind::InvalidTransactionScriptToken;
use crate::error::ErrorKind::ScriptStackUnderflow;
use crate::error::ErrorKind::InvalidTokenType;
use crate::error::ErrorKind::IntegerOverflow;

type OperatorFn = fn(stack: &mut Vec<Token>) -> Result<()>;

pub enum Token {
    ULiteralByteSeq(BigUint),
    Bool(bool),
    Operator(OperatorFn)
}

fn make_operator_name_map() -> HashMap<String, OperatorFn> {
    let mut out: HashMap<String, OperatorFn> = HashMap::new();

    out.insert(String::from("OP_ADD"), op_add);
    out.insert(String::from("OP_SUB"), op_sub);
    out.insert(String::from("OP_EQUAL"), op_equal);

    out
}

fn split(input: &String) -> Vec<String> {
    input.split(" ").map(|s| s.to_owned()).collect()
}

fn tokenize(raw_tokens: &Vec<String>) -> Result<Vec<Token>> {
    let mut out: Vec<Token> = vec![];
    let operator_map = make_operator_name_map();

    for raw_token in raw_tokens {
        let operator_opt = operator_map.get(raw_token);

        // Check if it is an operator
        if operator_opt.is_some() {
            let operator = operator_opt.unwrap().to_owned();
            out.push(Token::Operator(operator));
            continue;
        }

        // Check if it is a bool
        if raw_token == "TRUE" {
            out.push(Token::Bool(true));
            continue;
        } else if raw_token == "FALSE" {
            out.push(Token::Bool(false));
            continue;
        }

        let padded_token = 
            &match raw_token.len() % 2 == 0{
                true => raw_token.to_owned(),
                false => format!("0{}", raw_token),
            };

        // Pad and check if it is a hex string
        let hex_opt = hex::decode(padded_token);
        if hex_opt.is_err() {
            return Err(Box::new(InvalidTransactionScriptToken(raw_token.to_owned())));
        }

        let bytes = hex_opt.unwrap();
        let bigint = BigUint::from_bytes_be(&bytes);
        out.push(Token::ULiteralByteSeq(bigint));
    }

    Ok(out)
}

/// Executes a TsengScript, returning a single value.
pub fn execute(script: &String) -> Result<Option<Token>> {
    let raw_tokens = split(script);
    let tokens = tokenize(&raw_tokens)?;
    let mut stack: Vec<Token> = vec![];

    for token in tokens {
        match token {
            Token::Operator(op) => op(&mut stack)?,
            literal => stack.push(literal)
        };
    }

    // Return the last item on the stack - this is the result of the script
    Ok(stack.pop())
}

fn op_add(stack: &mut Vec<Token>) -> Result<()> {
    if stack.len() < 2 {
        return Err(Box::new(ScriptStackUnderflow))
    }

    let op1 = stack.pop().unwrap();
    let op2 = stack.pop().unwrap();

    match (op1, op2) {
        (Token::ULiteralByteSeq(bigint1), Token::ULiteralByteSeq(bigint2)) => {
            let result = bigint1 + bigint2;
            stack.push(Token::ULiteralByteSeq(result));
        },
        (_, _) => return Err(Box::new(InvalidTokenType)),
    };

    Ok(())
}

fn op_sub(stack: &mut Vec<Token>) -> Result<()> {
    if stack.len() < 2 {
        return Err(Box::new(ScriptStackUnderflow))
    }

    let op1 = stack.pop().unwrap();
    let op2 = stack.pop().unwrap();

    match (op1, op2) {
        (Token::ULiteralByteSeq(bigint1), Token::ULiteralByteSeq(bigint2)) => {
            if bigint1 > bigint2 {
                return Err(Box::new(IntegerOverflow));
            }

            let result = bigint2 - bigint1;
            stack.push(Token::ULiteralByteSeq(result));
        },
        (_, _) => return Err(Box::new(InvalidTokenType)),
    };

    Ok(())
}

fn op_equal(stack: &mut Vec<Token>) -> Result<()> {
    if stack.len() < 2 {
        return Err(Box::new(ScriptStackUnderflow))
    }

    let op1 = stack.pop().unwrap();
    let op2 = stack.pop().unwrap();

    match (op1, op2) {
        (Token::ULiteralByteSeq(item1), Token::ULiteralByteSeq(item2)) => {
            stack.push(Token::Bool(item1 == item2));
        },
        (Token::Bool(item1), Token::Bool(item2)) => {
            stack.push(Token::Bool(item1 == item2));
        },
        (_, _) => return Err(Box::new(InvalidTokenType)),
    };

    Ok(())
}