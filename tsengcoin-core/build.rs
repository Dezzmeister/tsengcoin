use std::error::Error;

#[cfg(windows)]
use winres::WindowsResource;

#[cfg(feature = "cuda_miner_kernel")]
fn build_kernels() {
    cuda_builder::CudaBuilder::new("../cuda-miner")
        .copy_to("kernels/miner.ptx")
        .build()
        .unwrap();
}

fn main() -> Result<(), Box<dyn Error>>{
    #[cfg(feature = "cuda_miner_kernel")]
    build_kernels();

    #[cfg(windows)]
    WindowsResource::new()
        .set_icon("assets/logo.ico")
        .compile()?;

    Ok(())
}
