use chrono::{DateTime, Duration, Utc};
use cust::prelude::*;
use lazy_static::lazy_static;
use std::sync::{
    mpsc::{Receiver, TryRecvError},
    Mutex,
};

use crate::{
    hash::hash_chunks,
    v1::{
        block::{
            make_merkle_root, Block, BlockHeader, RawBlock, RawBlockHeader,
            MAX_TRANSACTION_FIELD_SIZE,
        },
        block_verify::verify_block,
        request::Request,
        state::State,
        transaction::{coinbase_size_estimate, compute_fee, make_coinbase_txn, Transaction},
        VERSION,
    },
    wallet::Hash256,
};

use super::api::MinerMessage;

static MINER_PTX: &str = include_str!("../../../kernels/miner.ptx");

/// Update the hashes per sec metric every 5 seconds
const HASH_PER_SEC_INTERVAL: i64 = 5;

lazy_static! {
    /// Poll the MinerMessage receiver every 5 seconds
    static ref POLL_INTERVAL: Duration = Duration::seconds(5);
}

struct CUDAContext {
    _context: Context,
    _device: Device,
    module: Module,
    stream: Stream,
}

pub fn mine(state_mut: &Mutex<State>, receiver: Receiver<MinerMessage>) {
    let CUDAContext {
        _context,
        _device,
        module,
        stream,
    } = setup_cuda();
    let kernel = module
        .get_function("finish_hash")
        .expect("Failed to load mining function");
    let (grid_size, block_size) = kernel
        .suggested_launch_configuration(0, 0.into())
        .expect("Unable to determine launch config");
    let num_nonces: usize = (grid_size * block_size).try_into().unwrap();

    println!(
        "Running CUDA miner kernel with grid size {}, block size {}, and {} nonces per round",
        grid_size, block_size, num_nonces
    );
    let mut raw_block = make_raw_block(state_mut);

    println!(
        "Difficulty target is {}",
        hex::encode(raw_block.header.difficulty_target)
    );

    let mut raw_header_bytes = bincode::serialize(&raw_block.header).unwrap();
    let (mut schedule, mut hash_vars) = hash_chunks(&raw_header_bytes, 1);

    let mut nonces = vec![0_u8; num_nonces * 32];
    let mut hashes = vec![0_u8; num_nonces * 32];

    let mut nonces_gpu = DeviceBuffer::from_slice(&nonces).expect("Failed to create device memory");
    let mut prev_gpu =
        DeviceBuffer::from_slice(&schedule[0..11]).expect("Failed to create device memory");
    let mut hash_vars_gpu =
        DeviceBuffer::from_slice(&hash_vars).expect("Failed to create device memory");
    let hashes_gpu = DeviceBuffer::from_slice(&hashes).expect("Failed to create device memory");

    let mut now: DateTime<Utc>;

    let mut reset_time = Utc::now() + Duration::minutes(30);

    let mut print_time = Utc::now();
    let mut total_hashes: usize = 0;

    let mut last_poll_time = Utc::now();

    loop {
        now = Utc::now();

        if now - last_poll_time > *POLL_INTERVAL {
            let msg_result = receiver.try_recv();
            match msg_result {
                Err(TryRecvError::Disconnected) => {
                    println!("Stopping miner thread due to unexpected channel closing");
                    return;
                }
                Ok(MinerMessage::NewBlock(_, _)) | Ok(MinerMessage::NewTransactions(_))
                    if raw_block.transactions.len() == 1 =>
                {
                    // Force a reset by moving the reset time into the past
                    reset_time = Utc::now() - Duration::hours(1);
                    println!("Miner received instruction to reset");
                }
                Ok(MinerMessage::NewDifficulty(diff)) => {
                    reset_time = Utc::now() - Duration::hours(1);
                    println!("New difficulty target: {}", hex::encode(diff));
                }
                _ => (),
            }

            last_poll_time = now;
        }

        // If we have passed the reset time, then generate a fresh candidate block
        if reset_time < now {
            println!("Generating new candidate block");
            raw_block = make_raw_block(state_mut);
            raw_header_bytes = bincode::serialize(&raw_block.header).unwrap();
            let temp = hash_chunks(&raw_header_bytes, 1);
            schedule = temp.0;
            hash_vars = temp.1;

            prev_gpu
                .copy_from(&schedule[0..11])
                .expect("Failed to copy from host to device memory");
            hash_vars_gpu
                .copy_from(&hash_vars)
                .expect("Failed to copy from host to device memory");

            reset_time = now + Duration::minutes(30);
        }

        randomize(&mut nonces);
        nonces_gpu
            .copy_from(&nonces)
            .expect("Failed to copy memory from host to device");

        unsafe {
            launch!(
                kernel<<<grid_size, block_size, 0, stream>>>(
                    nonces_gpu.as_device_ptr(),
                    nonces_gpu.len(),
                    prev_gpu.as_device_ptr(),
                    hash_vars_gpu.as_device_ptr(),
                    hashes_gpu.as_device_ptr()
                )
            )
            .expect("Failed to launch mining kernel");
        }

        stream
            .synchronize()
            .expect("Failed to synchronize device stream");
        hashes_gpu
            .copy_to(&mut hashes)
            .expect("Failed to copy memory from device to host");

        total_hashes += num_nonces;

        if now - print_time > Duration::seconds(HASH_PER_SEC_INTERVAL) {
            state_mut.lock().unwrap().hashes_per_second =
                total_hashes / (HASH_PER_SEC_INTERVAL as usize);
            print_time = now;
            total_hashes = 0;
        }

        match find_winner(&nonces, &hashes, &raw_block.header.difficulty_target) {
            None => (),
            Some((nonce, hash)) => {
                let state = &mut state_mut.lock().unwrap();
                println!("Confirmed new block: {}", hex::encode(&hash));
                let new_block = Block {
                    header: BlockHeader {
                        version: raw_block.header.version,
                        prev_hash: raw_block.header.prev_hash,
                        merkle_root: raw_block.header.merkle_root,
                        timestamp: raw_block.header.timestamp,
                        difficulty_target: raw_block.header.difficulty_target,
                        nonce,
                        hash,
                    },
                    transactions: raw_block.transactions.clone(),
                };

                let verify_result = verify_block(new_block.clone(), state);
                match verify_result {
                    Ok(true) => {
                        // Why would this even happen? Who would mine a block with no parent?
                        println!("New block is an orphan. Rejecting");
                    }
                    Err(err) => {
                        println!("Rejecting new block: {}", err);
                    }
                    Ok(false) => {
                        state.network.broadcast_msg(&Request::NewBlock(new_block));
                    }
                }
                // Force a reset! If we don't do this, we may start working on a fork block because we may loop
                // again before the NewBlock message reaches us
                reset_time = now - Duration::hours(1);
            }
        }
    }
}

fn find_winner(nonces: &[u8], hashes: &[u8], difficulty: &Hash256) -> Option<(Hash256, Hash256)> {
    for i in 0..(nonces.len() / 32) {
        let t = i * 32;
        let hash: &[u8; 32] = hashes[t..(t + 32)].try_into().unwrap();

        if hash < difficulty {
            let nonce: [u8; 32] = nonces[t..(t + 32)].try_into().unwrap();

            return Some((nonce, hash.to_owned()));
        }
    }

    None
}

fn randomize(bytes: &mut [u8]) {
    for i in 0..bytes.len() {
        bytes[i] = rand::random();
    }
}

fn make_raw_block(state_mut: &Mutex<State>) -> RawBlock {
    let state = state_mut.lock().unwrap();
    let txns = state.pending_txns.clone();
    let (mut best_txns, fees) = pick_best_transactions(&txns, &state, coinbase_size_estimate());
    let coinbase = make_coinbase_txn(&state.address, String::from(""), fees, rand::random());

    let mut block_txns = vec![coinbase];
    block_txns.append(&mut best_txns);

    let prev_hash = state.blockchain.top_hash(0);
    let difficulty_target = state.blockchain.current_difficulty();

    let merkle_root = make_merkle_root(&block_txns);
    let header = RawBlockHeader {
        version: VERSION,
        prev_hash,
        merkle_root,
        timestamp: Utc::now().timestamp().try_into().unwrap(),
        difficulty_target,
        nonce: [0; 32],
    };

    RawBlock {
        header,
        transactions: block_txns,
    }
}

/// The problem here is to pick which transactions we will include in a block. Generally we want to maximize
/// the total fees while staying under the block size limit. This is the knapsack problem, and it is NP hard -
/// so rather than deal with it here we just take as many transactions as we can fit regardless of fee. We could take
/// a greedy approach to this problem and take the transactions with the highest fees, but then we would have to ensure that
/// we don't leave any dependency transactions behind. We chose not to deal with this because the network is small
/// and there won't be enough transactions to even approach the block size limit.
fn pick_best_transactions(
    txns: &[Transaction],
    state: &State,
    coinbase_size: usize,
) -> (Vec<Transaction>, u64) {
    let mut out: Vec<Transaction> = vec![];
    let mut size: usize = coinbase_size;
    let mut fees: u64 = 0;

    for txn in txns {
        let txn_size = txn.size();

        if (txn_size + size) > MAX_TRANSACTION_FIELD_SIZE {
            continue;
        }

        let fee = compute_fee(txn, state);
        out.push(txn.clone());
        size += txn_size;
        fees += fee;
    }

    (out, fees)
}

fn setup_cuda() -> CUDAContext {
    let ctx = cust::quick_init().expect("Failed to create CUDA context");
    // Just pick the first device for now because none of us have more than one CUDA GPU
    let device = Device::get_device(0).expect("Failed to get CUDA device");
    println!(
        "Using CUDA device: {}",
        device.name().expect("Failed to get device name")
    );

    let module = Module::from_ptx(MINER_PTX, &[]).expect("Failed to load mining code");
    let stream = Stream::new(StreamFlags::NON_BLOCKING, None)
        .expect("Failed to initialize stream to submit work to CUDA device");

    CUDAContext {
        _context: ctx,
        _device: device,
        module,
        stream,
    }
}
