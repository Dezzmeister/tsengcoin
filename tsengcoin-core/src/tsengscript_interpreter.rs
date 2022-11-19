use std::{collections::HashMap};
use num_bigint::BigUint;

use crate::error::Result;
use crate::error::ErrorKind::ScriptTooLong;
use crate::error::ErrorKind::InvalidScriptToken;
use crate::error::ErrorKind::ScriptStackUnderflow;
use crate::error::ErrorKind::ScriptStackOverflow;
use crate::error::ErrorKind::InvalidTokenType;
use crate::error::ErrorKind::IntegerOverflow;
use crate::error::ErrorKind::EqualVerifyFailed;
use crate::wallet::address_from_public_key;

/// Scripts can be 1kb max
const MAX_SCRIPT_LEN: usize = 1024;

/// Stack can have up to 2048 tokens
/// This will allow TsengScript to support small, non-recursize procedures
const MAX_STACK_SIZE: usize = 2048;

type OperatorFn = fn(stack: &mut Vec<Token>) -> Result<()>;

#[derive(Clone)]
pub enum Token {
    UByteSeq(BigUint),
    Bool(bool),
    Operator(OperatorFn)
}

pub struct ExecutionResult {
    pub top: Option<Token>,
    pub stack: Vec<Token>
}

fn make_operator_name_map() -> HashMap<String, OperatorFn> {
    let mut out: HashMap<String, OperatorFn> = HashMap::new();

    out.insert(String::from("OP_ADD"), op_add);
    out.insert(String::from("OP_SUB"), op_sub);
    out.insert(String::from("OP_EQUAL"), op_equal);
    out.insert(String::from("OP_EQUALVERIFY"), op_equalverify);
    out.insert(String::from("OP_DUP"), op_dup);
    out.insert(String::from("HASH160"), hash160);

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
            return Err(Box::new(InvalidScriptToken(raw_token.to_owned())));
        }

        let bytes = hex_opt.unwrap();
        let bigint = BigUint::from_bytes_be(&bytes);
        out.push(Token::UByteSeq(bigint));
    }

    Ok(out)
}

/// Executes a TsengScript, returning the top of the stack plus the stack's contents.
pub fn execute(script: &String) -> Result<ExecutionResult> {
    let script_len = script.as_bytes().len();
    if script_len > MAX_SCRIPT_LEN {
        return Err(Box::new(ScriptTooLong(MAX_SCRIPT_LEN, script_len)));
    }

    let raw_tokens = split(script);
    let tokens = tokenize(&raw_tokens)?;
    let mut stack: Vec<Token> = vec![];

    for token in tokens {
        match token {
            Token::Operator(op) => op(&mut stack)?,
            literal => stack.push(literal)
        };

        if stack.len() > MAX_STACK_SIZE {
            return Err(Box::new(ScriptStackOverflow));
        }
    }

    // Return the last item on the stack - this is the result of the script
    Ok(ExecutionResult { top: stack.pop(), stack })
}

fn op_add(stack: &mut Vec<Token>) -> Result<()> {
    if stack.len() < 2 {
        return Err(Box::new(ScriptStackUnderflow))
    }

    let op1 = stack.pop().unwrap();
    let op2 = stack.pop().unwrap();

    match (op1, op2) {
        (Token::UByteSeq(bigint1), Token::UByteSeq(bigint2)) => {
            let result = bigint1 + bigint2;
            stack.push(Token::UByteSeq(result));
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
        (Token::UByteSeq(bigint1), Token::UByteSeq(bigint2)) => {
            if bigint2 > bigint1 {
                return Err(Box::new(IntegerOverflow));
            }

            let result = bigint1 - bigint2;
            stack.push(Token::UByteSeq(result));
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
        (Token::UByteSeq(item1), Token::UByteSeq(item2)) => {
            stack.push(Token::Bool(item1 == item2));
        },
        (Token::Bool(item1), Token::Bool(item2)) => {
            stack.push(Token::Bool(item1 == item2));
        },
        (_, _) => return Err(Box::new(InvalidTokenType)),
    };

    Ok(())
}

fn op_equalverify(stack: &mut Vec<Token>) -> Result<()> {
    if stack.len() < 2 {
        return Err(Box::new(ScriptStackUnderflow))
    }

    let op1 = stack.pop().unwrap();
    let op2 = stack.pop().unwrap();

    match (op1, op2) {
        (Token::UByteSeq(item1), Token::UByteSeq(item2)) => {
            if item1 != item2 {
                return Err(Box::new(EqualVerifyFailed));
            }
        },
        (Token::Bool(item1), Token::Bool(item2)) => {
            if item1 != item2 {
                return Err(Box::new(EqualVerifyFailed));
            }
        },
        (_, _) => return Err(Box::new(InvalidTokenType)),
    };

    stack.push(Token::Bool(true));

    Ok(())
}

fn op_dup(stack: &mut Vec<Token>) -> Result<()> {
    if stack.len() < 1 {
        return Err(Box::new(ScriptStackUnderflow))
    }

    let op1 = stack.pop().unwrap();
    let op2 = op1.clone();
    stack.push(op1);
    stack.push(op2);

    Ok(())
}

fn hash160(stack: &mut Vec<Token>) -> Result<()> {
    if stack.len() < 1 {
        return Err(Box::new(ScriptStackUnderflow))
    }

    let op1 = stack.pop().unwrap();
    
    match op1 {
        Token::UByteSeq(bigint) => {
            let bytes = bigint.to_bytes_be();
            let hash = address_from_public_key(&bytes);

            stack.push(Token::UByteSeq(BigUint::from_bytes_be(&hash)));
        },
        _ => return Err(Box::new(InvalidTokenType)),
    };

    Ok(())
}
