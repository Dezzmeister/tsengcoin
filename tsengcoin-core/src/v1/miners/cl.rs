use std::{
    ptr,
    sync::{mpsc::{Receiver, TryRecvError}, Mutex},
};

use chrono::{DateTime, Utc, Duration};
use opencl3::{
    command_queue::{CommandQueue, CL_QUEUE_PROFILING_ENABLE},
    context::Context,
    device::{get_all_devices, Device, CL_DEVICE_TYPE_CPU, CL_DEVICE_TYPE_GPU},
    error_codes::ClError,
    kernel::{ExecuteKernel, Kernel},
    memory::{Buffer, CL_MEM_READ_ONLY, CL_MEM_WRITE_ONLY},
    program::Program,
    types::{cl_event, cl_uchar, cl_uint, CL_NON_BLOCKING},
};

use crate::{
    hash::{hash_chunks},
    v1::{
        block::{BlockHeader, Block},
        state::State, miners::{api::{make_raw_block, POLL_INTERVAL, randomize, find_winner}, stats::DEFAULT_GRANULARITY}, block_verify::verify_block, request::Request, net::{broadcast_async_blast},
    },
};

use super::api::MinerMessage;

static MINER_CL_CODE: &str = include_str!("../../../kernels/cl_miner.cl");

// TODO: Remove
#[allow(deprecated)]
pub fn mine(state_mut: &Mutex<State>, receiver: Receiver<MinerMessage>) {
    let device = match pick_best_device() {
        Err(err) => {
            println!("Error picking OpenCL device: {}", err);
            return;
        }
        Ok(None) => {
            println!("No OpenCL devices available");
            return;
        }
        Ok(Some(device)) => device,
    };
    let max_compute_units = device.max_compute_units().unwrap();
    let max_wg_size = device.max_work_group_size().unwrap();
    println!("Using OpenCL device: {}", device.name().unwrap());
    println!("Max compute units: {}", max_compute_units);
    println!("Max work group size: {}", max_wg_size);

    let (wg_size, work_groups) = {
        let state = &state_mut.lock().unwrap();

        (state.wg_size.unwrap_or(1), state.num_work_groups.unwrap_or(max_compute_units.try_into().unwrap()))
    };
    // The global work size
    let num_nonces = wg_size * work_groups;

    println!("Running OpenCL miner with work group size {} and {} work groups: {} nonces per round", wg_size, work_groups, num_nonces);

    let context = Context::from_device(&device).expect("Failed to create OpenCL context");
    let queue = CommandQueue::create_default(&context, CL_QUEUE_PROFILING_ENABLE)
        .expect("Failed to create command queue");
    let program = Program::create_and_build_from_source(&context, MINER_CL_CODE, "")
        .expect("Failed to build OpenCL program");
    let kernel = Kernel::create(&program, "finish_hash").expect("Failed to create OpenCL kernel");

    let mut raw_block = make_raw_block(state_mut);
    let mut raw_header_bytes = bincode::serialize(&raw_block.header).unwrap();
    let (mut schedule, mut hash_vars) = hash_chunks(&raw_header_bytes, 1);

    let mut nonces = vec![0 as cl_uchar; num_nonces * 32];
    let mut hashes = vec![0 as cl_uchar; num_nonces * 32];

    let mut nonces_buf = unsafe {
        Buffer::<cl_uchar>::create(&context, CL_MEM_READ_ONLY, nonces.len(), ptr::null_mut())
            .expect("Failed to create buffer for nonces")
    };

    let mut schedule_buf = unsafe {
        Buffer::<cl_uint>::create(&context, CL_MEM_READ_ONLY, 11, ptr::null_mut())
            .expect("Failed to create buffer for schedule")
    };

    let mut hash_vars_buf = unsafe {
        Buffer::<cl_uint>::create(&context, CL_MEM_READ_ONLY, 8, ptr::null_mut())
            .expect("Failed to create buffer for hash variables")
    };

    let hashes_buf = unsafe {
        Buffer::<cl_uchar>::create(&context, CL_MEM_WRITE_ONLY, hashes.len(), ptr::null_mut())
            .expect("Failed to create buffer for hashes")
    };

    let schedule_write_event = unsafe {
        queue
            .enqueue_write_buffer(&mut schedule_buf, CL_NON_BLOCKING, 0, &schedule[0..11], &[])
            .expect("Failed to write to schedule buffer")
    };

    let hash_vars_write_event = unsafe {
        queue
            .enqueue_write_buffer(&mut hash_vars_buf, CL_NON_BLOCKING, 0, &hash_vars, &[])
            .expect("Failed to write to hash vars buffer")
    };

    schedule_write_event.wait().unwrap();
    hash_vars_write_event.wait().unwrap();

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

        if reset_time < now {
            println!("Generating new candidate block");
            raw_block = make_raw_block(state_mut);
            raw_header_bytes = bincode::serialize(&raw_block.header).unwrap();
            let temp = hash_chunks(&raw_header_bytes, 1);
            schedule = temp.0;
            hash_vars = temp.1;

            let schedule_write_event = unsafe {
                queue
                    .enqueue_write_buffer(&mut schedule_buf, CL_NON_BLOCKING, 0, &schedule[0..11], &[])
                    .expect("Failed to write to schedule buffer")
            };
        
            let hash_vars_write_event = unsafe {
                queue
                    .enqueue_write_buffer(&mut hash_vars_buf, CL_NON_BLOCKING, 0, &hash_vars, &[])
                    .expect("Failed to write to hash vars buffer")
            };
        
            schedule_write_event.wait().unwrap();
            hash_vars_write_event.wait().unwrap();

            reset_time = now + Duration::minutes(30);
        }

        randomize(&mut nonces);
        let nonces_write_event = unsafe {
            queue
                .enqueue_write_buffer(&mut nonces_buf, CL_NON_BLOCKING, 0, &nonces, &[])
                .expect("Failed to write to nonce buffer")
        };

        let kernel_event = unsafe {
            ExecuteKernel::new(&kernel)
                .set_arg(&nonces_buf)
                .set_arg(&schedule_buf)
                .set_arg(&hash_vars_buf)
                .set_arg(&hashes_buf)
                .set_global_work_size(num_nonces)
                .set_local_work_size(wg_size)
                .set_wait_event(&hash_vars_write_event)
                .enqueue_nd_range(&queue)
                .expect("Failed to create kernel event")
        };

        let events: Vec<cl_event> = vec![nonces_write_event.get(), kernel_event.get()];

        let read_event = unsafe {
            queue
                .enqueue_read_buffer(&hashes_buf, CL_NON_BLOCKING, 0, &mut hashes, &events)
                .expect("Failed to read hashes from device")
        };
    
        read_event.wait().unwrap();

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

                reset_time = now - Duration::hours(1);
            }
        }
    }
}

fn pick_best_device() -> Result<Option<Device>, ClError> {
    let mut devices = get_all_devices(CL_DEVICE_TYPE_GPU)?;
    let mut cpus = get_all_devices(CL_DEVICE_TYPE_CPU)?;

    devices.append(&mut cpus);

    println!("Found {} OpenCL device(s)", devices.len());

    let device_id = match devices.first() {
        Some(id) => *id,
        None => return Ok(None),
    };

    let device = Device::new(device_id);

    Ok(Some(device))
}
