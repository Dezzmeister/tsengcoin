use std::{error::Error, collections::HashMap};

use crate::{command::{CommandInvocation, CommandMap, Command}, v1::{block::{RawBlockHeader, genesis_block}}, hash::{hash_sha256, hash_chunks}};
use cust::{prelude::*, device::DeviceAttribute};

fn cuda_hash_test(_invocation: &CommandInvocation, _state: Option<()>) -> Result<(), Box<dyn Error>> {
    static MINER_PTX: &str = include_str!("../../kernels/miner.ptx");
    let genesis = genesis_block();

    let raw_header: RawBlockHeader = (&genesis.header).into();
    let bytes = bincode::serialize(&raw_header).unwrap();
    let exp_hash = hash_sha256(&bytes);
    println!("Expecting hash: {}", hex::encode(&exp_hash));

    let _ctx = cust::quick_init().expect("Failed to create CUDA context");

    let device = Device::get_device(0).expect("Failed to get CUDA device");
    println!("Device Name: {}", device.name().expect("Failed to get device name"));
    println!("Max threads per device block: {}", device.get_attribute(DeviceAttribute::MaxThreadsPerBlock).expect("Failed to get max threads per block"));
    let module = Module::from_ptx(MINER_PTX, &[]).expect("Failed to load hashing module");
    let kernel = module.get_function("finish_hash").expect("Failed to load hashing kernel");
    let stream = Stream::new(StreamFlags::NON_BLOCKING, None).expect("Failed to create stream to submit work to device");

    let (schedule, hash_vars) = hash_chunks(&bytes, 1);

    let nonce_mem = genesis.header.nonce.clone();
    let hash_mem = vec![0 as u8; 32];

    let nonce_gpu = DeviceBuffer::from_slice(&nonce_mem).expect("Failed to create device memory for nonce");
    let prev_gpu = DeviceBuffer::from_slice(&schedule[0..11]).expect("Failed to create device memory for partial schedule");
    let hash_vars_gpu = DeviceBuffer::from_slice(&hash_vars).expect("Failed to create device memory for hash vars");
    let hashes_gpu = DeviceBuffer::from_slice(hash_mem.as_slice()).expect("Failed to create device memory for hash");

    let mut hashes_out = vec![0 as u8; 32];

    unsafe {
        launch!(
            kernel<<<1, 1, 0, stream>>>(
                nonce_gpu.as_device_ptr(),
                nonce_gpu.len(),
                prev_gpu.as_device_ptr(),
                hash_vars_gpu.as_device_ptr(),
                hashes_gpu.as_device_ptr()
            )
        ).expect("Failed to launch hashing kernel");
    }

    stream.synchronize().expect("Failed to synchronize device stream");

    hashes_gpu.copy_to(&mut hashes_out).expect("Failed to copy hash from device memory back to host");

    println!("GPU hash: {}", hex::encode(&hashes_out));

    match hashes_out == exp_hash {
        true => println!("Hashes match"),
        false => println!("Hashes do not match!")
    };

    Ok(())
}

pub fn make_command_map<'a>() -> CommandMap<()> {
    let mut map: CommandMap<()> = HashMap::new();
    let cuda_hash_test_cmd: Command<()> = Command {
        processor: cuda_hash_test,
        expected_fields: vec![],
        flags: vec![],
        optionals: vec![],
        desc: String::from("CUDA hash test"),
    };

    map.insert(String::from("cuda-hash-test"), cuda_hash_test_cmd);

    map
}