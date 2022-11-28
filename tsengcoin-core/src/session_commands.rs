use std::{sync::Mutex, collections::HashMap, error::Error};

use crate::{command::{dispatch_command, CommandInvocation, Command, FieldType, Field, Flag}, v1::{state::State, transaction::{get_p2pkh_addr}}};

fn getpeerinfo(_invocation: &CommandInvocation, state: Option<&Mutex<State>>) -> Result<(), Box<dyn Error>> {
    let guard = state.unwrap().lock().unwrap();
    let state = &*guard;

    let peers = &state.network.peers;

    println!("{} peers", peers.len());
    println!("{:#?}", peers);

    Ok(())
}

fn getknowninfo(_invocation: &CommandInvocation, state: Option<&Mutex<State>>) -> Result<(), Box<dyn Error>> {
    let guard = state.unwrap().lock().unwrap();
    let state = &*guard;

    let known_nodes = &state.network.known_nodes;

    println!("{} known nodes", known_nodes.len());
    println!("{:#?}", known_nodes);

    Ok(())
}

fn getblock(invocation: &CommandInvocation, state: Option<&Mutex<State>>) -> Result<(), Box<dyn Error>> {
    let hash_vec = hex::decode(invocation.get_field("hash").unwrap())?;
    let header_only = invocation.get_flag("header-only");
    let guard = state.unwrap().lock().unwrap();
    let state = &*guard;

    let mut hash = [0 as u8; 32];
    hash[32 - hash_vec.len()..].copy_from_slice(&hash_vec);

    let block_opt = state.blockchain.get_block(hash);

    match block_opt {
        None => println!("No such block exists"),
        Some((block, chain, pos)) if chain == 0 && !header_only => println!("Block found in main chain at pos {}\n{:#?}", pos, block),
        Some((block, _, pos)) if !header_only => println!("Block found in fork at pos {}\n{:#?}", pos, block),
        Some((block, chain, pos)) if chain == 0 && header_only => println!("Block found in main chain at pos {}\n{:#?}", pos, block.header),
        Some((block, _, pos)) => println!("Block found in fork at pos {}\n{:#?}", pos, block.header)
    }

    Ok(())
}

fn blockchain_stats(_invocation: &CommandInvocation, state: Option<&Mutex<State>>) -> Result<(), Box<dyn Error>> {
    let guard = state.unwrap().lock().unwrap();
    let state = &*guard;

    let (best_height, chain_idx, _) = &state.blockchain.best_chain();

    match chain_idx {
        0 => println!("The best chain is the main chain"),
        _ => println!("The best chain is a fork")
    };
    println!("Height of best chain: {best_height}");
    println!("Latest block on best chain: {}", hex::encode(&state.blockchain.top_hash(*chain_idx)));

    println!("{} forks", &state.blockchain.forks.len());

    Ok(())
}

fn balance_p2pkh(_invocation: &CommandInvocation, state: Option<&Mutex<State>>) -> Result<(), Box<dyn Error>> {
    let guard = state.unwrap().lock().unwrap();
    let state = &*guard;

    let my_utxos = 
        state.blockchain.utxo_pool.utxos
            .iter()
            .fold(vec![] as Vec<u64>, |mut a, u| {
                let txn = state.get_pending_or_confirmed_txn(u.txn).unwrap();
                let mut outputs = 
                    u.outputs
                        .iter()
                        .map(|idx| &txn.outputs[*idx])
                        .filter(|out| {
                            let dest_addr = get_p2pkh_addr(&out.lock_script.code);
                            match dest_addr {
                                None => false,
                                Some(dest) => dest == state.address
                            }
                        })
                        .map(|out| out.amount)
                        .collect::<Vec<u64>>();

                a.append(&mut outputs);
                a
            });
    
    let total_unspent = 
        my_utxos
            .iter()
            .fold(0, |a, e| a + e);

    println!("You have {} total unspent TsengCoin", total_unspent);
    
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
    let getblock_cmd: Command<&Mutex<State>> = Command {
        processor: getblock,
        expected_fields: vec![
            Field::new(
                "hash",
                FieldType::Pos(0),
                "The hash of this block"
            )
        ],
        flags: vec![
            Flag::new(
                "header-only",
                "Show only the block header. This will omit the transactions and some other info."
            )
        ],
        desc: String::from("Get the block with the given hash"),
    };
    let blockchain_stats_cmd: Command<&Mutex<State>> = Command {
        processor: blockchain_stats,
        expected_fields: vec![],
        flags: vec![],
        desc: String::from("Get some info about the current state of the blockchain")
    };
    let balance_p2pkh_cmd: Command<&Mutex<State>> = Command {
        processor: balance_p2pkh,
        expected_fields: vec![],
        flags: vec![],
        desc: String::from("Get the total unspent balance of your wallet. Balance may change if the network is forked.")
    };

    command_map.insert(String::from("getpeerinfo"), getpeerinfo_cmd);
    command_map.insert(String::from("getknowninfo"), getknowninfo_cmd);
    command_map.insert(String::from("getblock"), getblock_cmd);
    command_map.insert(String::from("blockchain-stats"), blockchain_stats_cmd);
    command_map.insert(String::from("balance-p2pkh"), balance_p2pkh_cmd);

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
