use std::{sync::Mutex, collections::HashMap, error::Error};

use crate::{v1::{state::State, block::{RawBlockHeader, make_merkle_root_from_hashes}}, command::{CommandMap, CommandInvocation, Command, FieldType, Field}, hash::hash_sha256, wallet::Hash256};

fn get_utxos(_invocation: &CommandInvocation, state: Option<&Mutex<State>>) -> Result<(), Box<dyn Error>> {
    let guard = state.unwrap().lock().unwrap();
    let state = &*guard;

    println!("{:#?}", state.blockchain.utxo_pool);

    Ok(())
}

fn hash_test(_invocation: &CommandInvocation, state: Option<&Mutex<State>>) -> Result<(), Box<dyn Error>> {
    let guard = state.unwrap().lock().unwrap();
    let state = &*guard;
    let genesis = &state.blockchain.blocks[0];

    let raw_header: RawBlockHeader = (&genesis.header).into();
    let bytes = bincode::serialize(&raw_header).unwrap();
    let hash = hash_sha256(&bytes);

    println!("hash: {}", hex::encode(&hash));

    Ok(())
}

fn merkle_test(invocation: &CommandInvocation, _state: Option<&Mutex<State>>) -> Result<(), Box<dyn Error>> {
    let raw_hashes_str = invocation.get_field("hashes").unwrap();
    let raw_hashes = raw_hashes_str.split(" ");
    let mut hashes: Vec<Hash256> = vec![];

    for raw_hash in raw_hashes {
        let bytes = hex::decode(&raw_hash).unwrap();
        let mut hash = [0 as u8; 32];
        
        hash.copy_from_slice(&bytes);
        hashes.push(hash);
    }

    let root = make_merkle_root_from_hashes(hashes);
    println!("{}", hex::encode(&root));

    Ok(())
}

fn print_blockchain(_invocation: &CommandInvocation, state: Option<&Mutex<State>>) -> Result<(), Box<dyn Error>> {
    let guard = state.unwrap().lock().unwrap();
    let state = &*guard;

    println!("{:#?}", state.blockchain);

    Ok(())
}

pub fn make_command_map<'a>() -> CommandMap<&'a Mutex<State>> {
    let mut map: CommandMap<&Mutex<State>> = HashMap::new();
    let get_utxos_cmd: Command<&Mutex<State>> = Command {
        processor: get_utxos,
        expected_fields: vec![],
        flags: vec![],
        optionals: vec![],
        desc: String::from("Print the UTXO database"),
    };
    let hash_test_cmd: Command<&Mutex<State>> = Command {
        processor: hash_test,
        expected_fields: vec![],
        flags: vec![],
        optionals: vec![],
        desc: String::from("Hash test")
    };
    let merkle_test_cmd: Command<&Mutex<State>> = Command {
        processor: merkle_test,
        expected_fields: vec![
            Field::new(
                "hashes",
                FieldType::Spaces(0),
                "The hashes to include in the merkle root"
            )
        ],
        flags: vec![],
        optionals: vec![],
        desc: String::from("Make a Merkle root from the given hashes")
    };
    let print_blockchain_cmd: Command<&Mutex<State>> = Command {
        processor: print_blockchain,
        expected_fields: vec![],
        flags: vec![],
        optionals: vec![],
        desc: String::from("Print the blockchain structure")
    };

    map.insert(String::from("get-utxos"), get_utxos_cmd);
    map.insert(String::from("hash-test"), hash_test_cmd);
    map.insert(String::from("merkle-test"), merkle_test_cmd);
    map.insert(String::from("print-blockchain"), print_blockchain_cmd);

    map
}
