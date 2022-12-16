use chrono::{DateTime, Duration, Utc};
use cust::prelude::*;
use std::sync::{
    mpsc::{Receiver, TryRecvError},
    Mutex,
};

use crate::{
    hash::hash_chunks,
    v1::{
        block::{
            Block, BlockHeader,
        },
        block_verify::verify_block,
        request::Request,
        state::State,
        miners::{api::{make_raw_block, POLL_INTERVAL, randomize, find_winner}, stats::DEFAULT_GRANULARITY}, net::{broadcast_async_blast},
    },
};

use super::api::MinerMessage;

static MINER_PTX: &str = include_str!("../../../kernels/miner.ptx");

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

    let hashrate_interval = state_mut.lock().unwrap().miner_stats.as_ref().map(|m| m.granularity).unwrap_or(DEFAULT_GRANULARITY);
    let hash_per_sec_duration = Duration::milliseconds(hashrate_interval as i64);

    if let Some(stats) = &mut state_mut.lock().unwrap().miner_stats {
        println!("Recording miner stats for {}s. Stats will be saved to {}", stats.record_for / 1000, stats.filename);
        stats.start();
    }

    let mut printed_stats_done = false;

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

        if now - print_time > hash_per_sec_duration {
            let state = &mut state_mut.lock().unwrap();

            let hashrate = 
                1000 * (total_hashes  / hashrate_interval as usize);
            print_time = now;
            total_hashes = 0;

            state.hashes_per_second = hashrate;

            if let Some(stats) = &mut state.miner_stats {
                if !stats.done() {
                    stats.add_record(hashrate);
                    stats.save().expect(&format!("Failed to save stats to file: {}", stats.filename));
                } else if !printed_stats_done {
                    println!("Finished recording miner statistics. Saved to file {}", stats.filename);
                    printed_stats_done = true;
                }
            }
        }

        match find_winner(&nonces, &hashes, &raw_block.header.difficulty_target) {
            None => (),
            Some((nonce, hash)) => {
                let mut guard = state_mut.lock().unwrap();
                let state = &mut *guard;
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
                        let peers = state.network.peer_addrs();
                        drop(guard);

                        broadcast_async_blast(Request::NewBlock(new_block), &peers, None);
                    }
                }
                // Force a reset! If we don't do this, we may start working on a fork block because we may loop
                // again before the NewBlock message reaches us
                reset_time = now - Duration::hours(1);
            }
        }
    }
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
