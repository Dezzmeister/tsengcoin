pub mod api;

#[cfg(feature = "cuda_miner")]
pub mod cuda;

#[cfg(feature = "cl_miner")]
pub mod cl;
