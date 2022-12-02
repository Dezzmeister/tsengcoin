use std::{collections::HashMap, error::Error};
use std::sync::Mutex;

use ring::signature::KeyPair;

use crate::v1::request::send_new_txn;
use crate::v1::{VERSION};
use crate::v1::transaction::{p2pkh_utxos_for_addr, make_p2pkh_lock, collect_enough_change, TxnOutput, UnsignedTransaction, sign_txn, make_p2pkh_unlock, TxnInput, UnhashedTransaction, hash_txn};
use crate::v1::txn_verify::verify_transaction;
use crate::wallet::b58c_to_address;
use crate::{command::{dispatch_command, CommandInvocation, Command, FieldType, Field, Flag}, v1::{state::State}};

#[cfg(feature = "debug")]
use super::debug::make_command_map;

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

    let my_utxos = p2pkh_utxos_for_addr(state, state.address);
    
    let total_unspent = 
        my_utxos
            .iter()
            .fold(0, |a, e| a + e.amount);

    println!("You have {} total unspent TsengCoin", total_unspent);
    
    Ok(())
}

fn send_coins_p2pkh(invocation: &CommandInvocation, state: Option<&Mutex<State>>) -> Result<(), Box<dyn Error>> {
    let pkh = invocation.get_field("dest-address").unwrap();
    let amount = invocation.get_field("amount").unwrap().parse::<u64>().unwrap();
    let fee = invocation.get_field("fee").unwrap().parse::<u64>().unwrap();
    let show_structure = invocation.get_flag("show-structure");
    let guard = state.unwrap().lock().unwrap();
    let state = &*guard;

    let required_input = amount + fee;

    let change = match collect_enough_change(state, state.address, required_input) {
        None => {
            println!("You don't have enough TsengCoin to make that transaction");
            return Ok(());
        },
        Some(utxos) => utxos
    };

    let actual_input = 
        change
            .iter()
            .fold(0, |a, e| a + e.amount);
    
    let dest_address = b58c_to_address(pkh)?;
    let lock_script = make_p2pkh_lock(&dest_address);
    let mut outputs: Vec<TxnOutput> = vec![TxnOutput { amount, lock_script }];

    let change_back = actual_input - required_input;

    if change_back > 0 {
        let my_lock_script = make_p2pkh_lock(&state.address);

        outputs.push(TxnOutput {
            amount: change_back,
            lock_script: my_lock_script
        });
    }

    let metadata = String::from("");
    
    let unsigned_txn = UnsignedTransaction {
        version: VERSION,
        outputs: outputs.clone(),
        meta: metadata.clone(),
    };

    let sig = sign_txn(&unsigned_txn, &state.keypair)?;
    let pubkey = state.keypair.public_key().as_ref().to_vec();
    let unlock_script = make_p2pkh_unlock(sig, pubkey);
    let txn_inputs =
        change
            .iter()
            .map(|c| {
                TxnInput {
                    txn_hash: c.txn,
                    output_idx: c.output,
                    unlock_script: unlock_script.clone(),
                }
            })
            .collect::<Vec<TxnInput>>();

    let unhashed = UnhashedTransaction {
        version: VERSION,
        inputs: txn_inputs,
        outputs,
        meta: metadata,
    };

    let hash = hash_txn(&unhashed)?;
    let full_txn = unhashed.to_hashed(hash);

    if show_structure {
        println!("{:#?}", full_txn.clone());
    }

    match verify_transaction(full_txn.clone(), state) {
        Ok(_) => {
            send_new_txn(full_txn, state)?;
            println!("Successfully submitted transaction");
        },
        Err(err) => {
            println!("There was a problem verifying your transaction: {}", err.to_string())
        }
    };

    Ok(())
}

fn hashes_per_sec(_invocation: &CommandInvocation, state: Option<&Mutex<State>>) -> Result<(), Box<dyn Error>> {
    let guard = state.unwrap().lock().unwrap();
    let state = &*guard;

    println!("Hashes per second: {}", state.hashes_per_second);

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
    let send_coins_p2pkh_cmd: Command<&Mutex<State>> = Command {
        processor: send_coins_p2pkh,
        expected_fields: vec![
            Field::new(
                "dest-address",
                FieldType::Pos(0),
                "The address you want to send TsengCoin to"
            ),
            Field::new(
                "amount",
                FieldType::Pos(1),
                "The amount of TsengCoin you want to send"
            ),
            Field::new(
                "fee",
                FieldType::Pos(2),
                "The transaction fee you will pay, must be nonzero"
            )
        ],
        flags: vec![
            Flag::new(
                "show-structure",
                "Show the structure of the transaction after it is created"
            )
        ],
        desc: String::from("Send a recipient TsengCoins in a P2PKH transaction. This is the most widely used style of transaction")
    };
    let hashes_per_sec_cmd: Command<&Mutex<State>> = Command {
        processor: hashes_per_sec,
        expected_fields: vec![],
        flags: vec![],
        desc: String::from("Get the hashrate of the miner, if it's running.")
    };

    command_map.insert(String::from("getpeerinfo"), getpeerinfo_cmd);
    command_map.insert(String::from("getknowninfo"), getknowninfo_cmd);
    command_map.insert(String::from("getblock"), getblock_cmd);
    command_map.insert(String::from("blockchain-stats"), blockchain_stats_cmd);
    command_map.insert(String::from("balance-p2pkh"), balance_p2pkh_cmd);
    command_map.insert(String::from("send-coins-p2pkh"), send_coins_p2pkh_cmd);
    command_map.insert(String::from("hashes-per-sec"), hashes_per_sec_cmd);

    // Include debug commands if the feature is enabled
    #[cfg(feature = "debug")]
    {
        let dbg_cmds = make_command_map();
        for (key, val) in dbg_cmds.into_iter() {
            command_map.insert(key, val);
        }
    }

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
