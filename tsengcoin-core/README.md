# TsengCoin Core

This project consists of several parts which each warrant their own documentation:

- [Networking](./docs/Networking.md)
- [Transactions](./docs/Transactions.md)
- [Blocks](./docs/Blocks.md)
- [TsengScript](./docs/TsengScript.md)

## Building

The project makes use of Rust's "features" which allow parts of the application to be included or excluded from compilation. You can build a public release binary with

```
cargo build --release --features "cuda_miner, cuda_miner_kernel"
```

The `cuda_miner_kernel` feature instructs the compiler to build the [CUDA miner kernel](../cuda-miner/README.md) as well. Building this component takes a long time so it is usually best to exclude this flag and only use it when you change the CUDA kernel. Building the kernel will create a `PTX` file in the `kernels` folder which gets loaded at runtime.

The `cuda_miner` feature instructs the compiler to include the CUDA mining code. This is behind a feature flag so that builds can be made for devices without CUDA.

The miner in the release build is generally much faster than the miner in a regular debug build.

### Debug

You can run the application with

```
cargo run --features "debug, cuda_miner" <command>
```

The `debug` feature includes some debugging commands that you wouldn't want in a release build. Combined with the `cuda_miner` flag, this includes some debugging commands to test CUDA mining.

`<command>` is one of several top-level commands recognized by the application. You can run the `help` command to get a list of commands, and you can run `help <command>` for more detailed information about a command including how to use it.
