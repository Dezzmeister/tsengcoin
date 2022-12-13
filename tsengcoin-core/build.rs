#[cfg(feature = "cuda_miner_kernel")]
fn build_kernels() {
    cuda_builder::CudaBuilder::new("../cuda-miner")
        .copy_to("kernels/miner.ptx")
        .build()
        .unwrap();
}

fn main() {
    #[cfg(feature = "cuda_miner_kernel")]
    build_kernels();
}
