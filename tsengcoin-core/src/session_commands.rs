use std::{sync::Mutex, collections::HashMap, error::Error};

use crate::{command::{dispatch_command, CommandInvocation, Command}, v1::state::State};

fn getpeerinfo(_command_name: &String, _invocation: &CommandInvocation, state: Option<&Mutex<State>>) -> Result<(), Box<dyn Error>> {
    let guard = state.unwrap().lock().unwrap();
    let state = &*guard;

    let peers = &state.network.peers;

    println!("{} peers", peers.len());
    println!("{:?}", peers);

    Ok(())
}

fn getknowninfo(_command_name: &String, _invocation: &CommandInvocation, state: Option<&Mutex<State>>) -> Result<(), Box<dyn Error>> {
    let guard = state.unwrap().lock().unwrap();
    let state = &*guard;

    let known_nodes = &state.network.known_nodes;

    println!("{} known nodes", known_nodes.len());
    println!("{:?}", known_nodes);

    Ok(())
}

pub fn listen_for_commands(state_mut: &Mutex<State>) {
    let mut command_map = HashMap::new();
    let getpeerinfo_cmd: Command<&Mutex<State>> = Command {
        processor: getpeerinfo,
        expected_fields: vec![],
        flags: vec![],
        desc: String::from("Get info about direct peers with which this node communicates"),
    };
    let getknowninfo_cmd: Command<&Mutex<State>> = Command {
        processor: getknowninfo,
        expected_fields: vec![],
        flags: vec![],
        desc: String::from("Get info about all nodes that this node knows about")
    };

    command_map.insert(String::from("getpeerinfo"), getpeerinfo_cmd);
    command_map.insert(String::from("getknowninfo"), getknowninfo_cmd);

    let mut buffer = String::new();
    let stdin = std::io::stdin();

    loop {
        let res = stdin.read_line(&mut buffer);

        if res.is_err() {
            println!("Error reading command: {:?}", res.err());
            continue;
        }

        let args: Vec<&str> = buffer.trim().split(' ').collect();

        if args.len() < 1 {
            println!("Need to supply a command");
            continue;
        }

        let cmd_args = args.to_vec().iter().map(|&s| s.into()).collect();

        dispatch_command(&cmd_args, &command_map, Some(state_mut));
        buffer.clear();
    }
}
