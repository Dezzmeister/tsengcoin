use std::{sync::Mutex, collections::HashMap, error::Error};

use crate::{v1::state::State, command::{CommandMap, CommandInvocation, Command}};

fn get_utxos(_invocation: &CommandInvocation, state: Option<&Mutex<State>>) -> Result<(), Box<dyn Error>> {
    let guard = state.unwrap().lock().unwrap();
    let state = &*guard;

    println!("{:#?}", state.blockchain.utxo_pool);

    Ok(())
}

pub fn make_command_map<'a>() -> CommandMap<&'a Mutex<State>> {
    let mut map: CommandMap<&Mutex<State>> = HashMap::new();
    let get_utxos_cmd: Command<&Mutex<State>> = Command {
        processor: get_utxos,
        expected_fields: vec![],
        flags: vec![],
        desc: String::from("Print the UTXO database"),
    };

    map.insert(String::from("get-utxos"), get_utxos_cmd);

    map
}
