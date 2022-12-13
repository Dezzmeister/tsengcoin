use std::{sync::{Mutex, mpsc::Receiver}, ptr};

use opencl3::{device::{Device, get_all_devices, CL_DEVICE_TYPE_GPU, CL_DEVICE_TYPE_CPU}, error_codes::ClError, context::Context, command_queue::{CommandQueue, CL_QUEUE_PROFILING_ENABLE}, program::Program, kernel::{Kernel, ExecuteKernel}, types::{cl_uint, cl_uchar, CL_NON_BLOCKING, cl_event}, memory::{Buffer, CL_MEM_READ_ONLY, CL_MEM_WRITE_ONLY}};

use crate::{v1::{state::State, block::{genesis_block, RawBlockHeader}}, hash::{hash_chunks, hash_sha256}};

use super::api::MinerMessage;


static MINER_CL_CODE: &str = include_str!("../../../kernels/cl_miner.cl");

// TODO: Remove
#[allow(unused, deprecated)]
pub fn mine(_state_mut: &Mutex<State>, _receiver: Receiver<MinerMessage>) {
    let device = match pick_best_device() {
        Err(err) => {
            println!("Error picking OpenCL device: {}", err);
            return;
        },
        Ok(None) => {
            println!("No OpenCL devices available");
            return;
        },
        Ok(Some(device)) => device
    };
    let max_compute_units = device.max_compute_units().unwrap();
    println!("max compute units: {}", max_compute_units);
    println!("device: {}", device.name().unwrap());

    let context = Context::from_device(&device).expect("Failed to create OpenCL context");
    let queue = CommandQueue::create_default(&context, CL_QUEUE_PROFILING_ENABLE)
        .expect("Failed to create command queue");
    let program = Program::create_and_build_from_source(&context, MINER_CL_CODE, "")
        .expect("Failed to build OpenCL program");
    let kernel = Kernel::create(&program, "finish_hash").expect("Failed to create OpenCL kernel");

    let genesis_block = genesis_block();
    let exp_hash = genesis_block.header.hash;
    let unhashed: RawBlockHeader = (&genesis_block.header).into();
    let raw_data = bincode::serialize(&unhashed).unwrap();
    let exp_hash_2 = hash_sha256(&raw_data);
    let (schedule, hash_vars) = hash_chunks(&raw_data, 1);

    let nonces: [cl_uchar; 32] = genesis_block.header.nonce;
    let schedule_part: [cl_uint; 11] = (&schedule[0..11]).try_into().unwrap();
    let mut hashes: [cl_uchar; 32] = [0; 32];

    let mut nonces_buf = unsafe {
        Buffer::<cl_uchar>::create(&context, CL_MEM_READ_ONLY, 32, ptr::null_mut())
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

    let mut hashes_buf = unsafe {
        Buffer::<cl_uchar>::create(&context, CL_MEM_WRITE_ONLY, 32, ptr::null_mut())
            .expect("Failed to create buffer for hashes")
    };

    let nonces_write_event = unsafe {
        queue.enqueue_write_buffer(&mut nonces_buf, CL_NON_BLOCKING, 0, &nonces, &[])
            .expect("Failed to write to nonce buffer")
    };

    let schedule_write_event = unsafe {
        queue.enqueue_write_buffer(&mut schedule_buf, CL_NON_BLOCKING, 0, &schedule_part, &[])
            .expect("Failed to write to schedule buffer")
    };

    let hash_vars_write_event = unsafe {
        queue.enqueue_write_buffer(&mut hash_vars_buf, CL_NON_BLOCKING, 0, &hash_vars, &[])
            .expect("Failed to write to hash vars buffer")
    };

    nonces_write_event.wait().unwrap();
    schedule_write_event.wait().unwrap();

    let kernel_event = unsafe {
        ExecuteKernel::new(&kernel)
            .set_arg(&nonces_buf)
            .set_arg(&schedule_buf)
            .set_arg(&hash_vars_buf)
            .set_arg(&hashes_buf)
            .set_global_work_size(1)
            .set_wait_event(&hash_vars_write_event)
            .enqueue_nd_range(&queue)
            .expect("Failed to create kernel event")
    };

    let events: Vec<cl_event> = vec![kernel_event.get()];
    
    let read_event = unsafe {
        queue.enqueue_read_buffer(&hashes_buf, CL_NON_BLOCKING, 0, &mut hashes, &events)
            .expect("Failed to read hashes from device")
    };

    read_event.wait().unwrap();

    let device_hash = hex::encode(&hashes);
    let real_hash = hex::encode(&exp_hash);

    panic!();
}

fn pick_best_device() -> Result<Option<Device>, ClError> {
    let mut devices = get_all_devices(CL_DEVICE_TYPE_GPU)?;
    let mut cpus = get_all_devices(CL_DEVICE_TYPE_CPU)?;

    devices.append(&mut cpus);

    let device_id = match devices.first() {
        Some(id) => *id,
        None => return Ok(None)
    };

    let device = Device::new(device_id);

    Ok(Some(device))
}
