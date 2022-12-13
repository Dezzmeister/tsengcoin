use std::{collections::HashMap, error::Error, net::{SocketAddr, IpAddr, Ipv4Addr}, sync::{Arc, mpsc::channel}, thread};
use std::sync::Mutex;

use ring::signature::KeyPair;
use thread_priority::{ThreadPriority, ThreadBuilderExt};

use crate::{command::{CommandMap, Command, CommandInvocation, Field, FieldType, Flag, Condition, VarField}, tsengscript_interpreter::{execute, ExecutionResult, Token}, wallet::{address_from_public_key, address_to_b58c, b58c_to_address, create_keypair, load_keypair, Address}, v1::{request::{get_first_peers, discover, advertise_self, download_latest_blocks}, state::State, net::listen_for_connections, miners::{api::{start_miner, num_miners, miners}}}, gui::gui::{gui_req_loop, GUIState, main_gui_loop}};
use super::session::listen_for_commands;

#[cfg(all(feature = "debug", feature = "cuda_miner"))]
use super::cuda_debug::make_command_map as make_cuda_dbg_command_map;

fn run_script(invocation: &CommandInvocation, _state: Option<()>) -> Result<(), Box<dyn Error>> {
    let script = invocation.get_field("script").unwrap();
    let show_stack = invocation.get_flag("show-stack");
    let ExecutionResult{top, stack } = execute(&script, &vec![])?;

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

fn random_test_address(_invocation: &CommandInvocation, _state: Option<()>) -> Result<(), Box<dyn Error>> {
    let rand_bytes: [u8; 32] = rand::random();
    let address = address_from_public_key(&rand_bytes.to_vec());

    println!("{}", hex::encode(&address));

    Ok(())
}

fn b58c_encode(invocation: &CommandInvocation, _state: Option<()>) -> Result<(), Box<dyn Error>> {
    let raw = invocation.get_field("hex-bytes").unwrap();
    let bytes = hex::decode(raw)?;
    let encoded = address_to_b58c(&bytes);

    println!("{}", encoded);

    Ok(())
}

fn b58c_decode(invocation: &CommandInvocation, _state: Option<()>) -> Result<(), Box<dyn Error>> {
    let raw = invocation.get_field("encoded").unwrap();
    let decoded = b58c_to_address(raw)?;
    let hex_str = hex::encode(decoded);

    println!("{}", hex_str);

    Ok(())
}

fn create_address(invocation: &CommandInvocation, _state: Option<()>) -> Result<(), Box<dyn Error>> {
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

fn test_load_keypair(invocation: &CommandInvocation, _state: Option<()>) -> Result<(), Box<dyn Error>> {
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

fn connect(invocation: &CommandInvocation, _state: Option<()>) -> Result<(), Box<dyn Error>> {
    let seed_ip = invocation.get_field("seed-ip").unwrap().parse::<IpAddr>().unwrap();
    let seed_port = invocation.get_field("seed-port").unwrap().parse::<u16>().unwrap();
    let listen_port = invocation.get_field("listen-port").unwrap().parse::<u16>().unwrap();
    let wallet_path = invocation.get_field("wallet-path").unwrap();
    let wallet_password = invocation
        .get_field("wallet-password")
        .unwrap_or_else(||
            fltk::dialog::password_default("Enter your wallet password", "").expect("Need to supply a password!")
        );
    let with_gui = invocation.get_flag("gui");
    let gui_state = match with_gui {
        false => None,
        true => Some(GUIState::new())
    };
    let miner_names = miners();
    let miner = match num_miners() {
        0 => None,
        1 if !invocation.get_flag("with-miner") => None,
        1 => Some(miner_names[0].clone()),
        _ if invocation.get_optional("miner").is_none() => None,
        _ => {
            let miner_name = invocation.get_optional("miner").unwrap();
            if miner_names.contains(&miner_name) {
                Some(miner_name)
            } else {
                return Err(format!("Miner {} is not recognized/supported", miner_name))?;
            }
        }
    };

    let keypair = load_keypair(&wallet_password, &wallet_path)?;
    let address: Address = address_from_public_key(&keypair.public_key().as_ref().to_vec());
    let b58c_address = address_to_b58c(&address.to_vec());

    println!("Loaded wallet for address {}", b58c_address);

    let seed_addr = SocketAddr::new(seed_ip, seed_port);
    let addr_me = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), listen_port);

    println!("Connecting to node at {} and starting bootstrap process", seed_addr);

    let (gui_req_sender, gui_req_receiver) = channel();
    let (gui_res_sender, gui_res_receiver) = channel();

    let (state, miner_receiver) = State::new(addr_me, keypair, gui_req_sender.clone(), gui_state, miner.clone());
    let state_mut = Mutex::new(state);
    let state_arc = Arc::new(state_mut);
    let state_arc_2 = Arc::clone(&state_arc);
    let state_arc_3 = Arc::clone(&state_arc);

    get_first_peers(seed_addr, &state_arc)?;
    discover(seed_addr, &state_arc)?;
    download_latest_blocks(&state_arc)?;

    println!("Starting network listener thread");
    thread::Builder::new().name(String::from("network-listener")).spawn(move || {
        listen_for_connections(addr_me, &gui_req_sender, &gui_res_receiver, &state_arc_2).expect("Network listener thread crashed");
        advertise_self(&state_arc_2).expect("Failed to advertise self to network");
    }).unwrap();

    println!("Bootstrapping complete\nStarting worker threads");

    if miner.is_some() {
        let state_arc_miner = Arc::clone(&state_arc);

        println!("Starting miner thread");
        thread::Builder::new()
            .name(String::from("miner"))
            .spawn_with_priority(ThreadPriority::Max, move |_| {
                start_miner(&state_arc_miner, miner_receiver, &miner.unwrap());
            }).unwrap();
    }

    thread::Builder::new().name(String::from("command")).spawn(move || {
        println!("Type a command, or 'help' for a list of commands");
        listen_for_commands(&state_arc_3);
    }).unwrap();

    if with_gui {
        main_gui_loop(state_arc);
    } else {
        gui_req_loop(gui_req_receiver, gui_res_sender);
    }

    Ok(())
}

fn start_seed(invocation: &CommandInvocation, _state: Option<()>) -> Result<(), Box<dyn Error>> {
    let listen_port = invocation.get_field("listen-port").unwrap().parse::<u16>().unwrap();
    let wallet_path = invocation.get_field("wallet-path").unwrap();
    let wallet_password = invocation
        .get_field("wallet-password")
        .unwrap_or_else(||
            fltk::dialog::password_default("Enter your wallet password", "").expect("Need to supply a password!")
        );
    let with_gui = invocation.get_flag("gui");
    let gui_state = match with_gui {
        false => None,
        true => Some(GUIState::new())
    };
    let miner_names = miners();
    let miner = match num_miners() {
        0 => None,
        1 if !invocation.get_flag("with-miner") => None,
        1 => Some(miner_names[0].clone()),
        _ if invocation.get_optional("miner").is_none() => None,
        _ => {
            let miner_name = invocation.get_optional("miner").unwrap();
            if miner_names.contains(&miner_name) {
                Some(miner_name)
            } else {
                return Err(format!("Miner {} is not recognized/supported", miner_name))?;
            }
        }
    };

    let keypair = load_keypair(&wallet_password, &wallet_path)?;
    let address: Address = address_from_public_key(&keypair.public_key().as_ref().to_vec());
    let b58c_address = address_to_b58c(&address.to_vec());

    println!("Loaded wallet for address {}", b58c_address);

    let addr_me = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), listen_port);

    let (gui_req_sender, gui_req_receiver) = channel();
    let (gui_res_sender, gui_res_receiver) = channel();

    let (state, miner_receiver) = State::new(addr_me, keypair, gui_req_sender.clone(), gui_state, miner.clone());
    let state_mut = Mutex::new(state);
    let state_arc = Arc::new(state_mut);
    let state_arc_2 = Arc::clone(&state_arc);
    let state_arc_3 = Arc::clone(&state_arc);

    println!("Skipping bootstrapping, because `start-seed` was used instead of `connect`. Run `connect` if you wish to connect to an existing TsengCoin network");

    println!("Starting network listener thread");
    thread::Builder::new().name(String::from("network-listener")).spawn(move || {
        listen_for_connections(addr_me, &gui_req_sender, &gui_res_receiver, &state_arc_2).expect("Network listener thread crashed");
    }).unwrap();

    if miner.is_some() {
        let state_arc_miner = Arc::clone(&state_arc);

        println!("Starting miner thread");
        thread::Builder::new()
            .name(String::from("miner"))
            .spawn_with_priority(ThreadPriority::Max, move |_| {
                start_miner(&state_arc_miner, miner_receiver, &miner.unwrap());
            }).unwrap();
    }

    thread::Builder::new().name(String::from("command")).spawn(move || {
        println!("Type a command, or 'help' for a list of commands");
        listen_for_commands(&state_arc_3);
    }).unwrap();

    if with_gui {
        main_gui_loop(state_arc);
    } else {
        gui_req_loop(gui_req_receiver, gui_res_sender);
    }

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
        optionals: vec![],
        desc: String::from("Run a TsengScript program and see the output and stack trace")
    };
    let random_test_address_hex_cmd: Command<()> = Command {
        processor: random_test_address,
        expected_fields: vec![],
        flags: vec![],
        optionals: vec![],
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
        optionals: vec![],
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
        optionals: vec![],
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
        optionals: vec![],
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
        optionals: vec![],
        desc: String::from("Load a keypair file locked with a password and get the address out of it. The file is encrypted so this only works if you have the right password")
    };

    let num_miners = num_miners();
    let mut connect_flags = vec![Flag::new(
        "gui",
        "Set this flag to start the GUI application as well. You can still use TsengCoin from the console, but some GUI-only features will also be available."
    )];
    let mut connect_optionals = vec![];
    if num_miners == 1 {
        connect_flags.append(&mut vec![
            Flag::new(
                "with-miner",
                "Set this flag if you want to mine TsengCoin in the background"
            )
        ]);
    } else if num_miners > 1 {
        let miners = miners();
        let placeholder = miner_placeholder(&miners);
        let readable_miners = miner_list(&miners);
        connect_optionals.push(VarField::new_placeholder(
            "miner",
            &format!("Set this to start the client with a miner. There are different mining kernels you can use, the options are{}", readable_miners),
            &placeholder
        ));
    }

    let connect_cmd: Command<()> = Command {
        processor: connect,
        expected_fields: vec![
            Field::new(
                "seed-ip",
                FieldType::Pos(0),
                "IP address of a node in the network to connect to"
            ),
            Field::new(
                "seed-port",
                FieldType::Pos(1),
                "Port of a node in the network to connect to, corresponding to the seed IP"
            ),
            Field::new(
                "listen-port",
                FieldType::Pos(2),
                "Port to listen for incoming connections on"
            ),
            Field::new(
                "wallet-path",
                FieldType::Pos(3),
                "Path to your wallet file"
            ),
            Field::new_condition(
                "wallet-password",
                FieldType::Spaces(4),
                "Password to your wallet file",
                Condition::new(
                    "pwgui",
                    "Set this flag to enter the password through a dialog box instead of passing it in as a command line argument."
                )
            )
        ],
        flags: connect_flags.clone(),
        optionals: connect_optionals.clone(),
        desc: String::from("Connect to the TsengCoin network as a full node. Unless you're trying to do fancy stuff, this is probably the command you want. If you don't have a wallet yet, run `create-address` first.")
    };
    let start_seed_cmd: Command<()> = Command {
        processor: start_seed,
        expected_fields: vec![
            Field::new(
                "listen-port",
                FieldType::Pos(0),
                "Port to listen for incoming connections on"
            ),
            Field::new(
                "wallet-path",
                FieldType::Pos(1),
                "Path to your wallet file"
            ),
            Field::new_condition(
                "wallet-password",
                FieldType::Spaces(2),
                "Password to your wallet file",
                Condition::new(
                    "pwgui",
                    "Set this flag to enter the password through a dialog box instead of passing it in as a command line argument."
                )
            )
        ],
        flags: connect_flags,
        optionals: connect_optionals,
        desc: String::from("Start as a full node without bootstrapping. The node will not attempt to connect to any network, and it will use whatever blockchain data it has locally.")
    };

    out.insert(String::from("run-script"), run_script_cmd);
    out.insert(String::from("random-test-address-hex"), random_test_address_hex_cmd);
    out.insert(String::from("b58c-encode"), b58c_encode_cmd);
    out.insert(String::from("b58c-decode"), b58c_decode_cmd);
    out.insert(String::from("create-address"), create_address_cmd);
    out.insert(String::from("test-load-keypair"), test_load_keypair_cmd);
    out.insert(String::from("connect"), connect_cmd);
    out.insert(String::from("start-seed"), start_seed_cmd);

    #[cfg(all(feature = "debug", feature = "cuda_miner"))]
    {
        let cuda_dbg_cmds = make_cuda_dbg_command_map();
        for (key, val) in cuda_dbg_cmds.into_iter() {
            out.insert(key, val);
        }
    }

    out
}

fn miner_placeholder(miners: &Vec<String>) -> String {
    let mut out = String::from("(");

    for miner in miners {
        out.push_str(miner);
        out.push_str("|");
    }

    out.remove(out.len() - 1);
    out.push_str(")");
    out
}

fn miner_list(miners: &Vec<String>) -> String {
    let mut out = String::from("");

    for miner in miners {
        out.push_str(" ");
        out.push_str(&format!("\"{}\",", miner));
    }

    out.remove(out.len() - 1);

    out
}
