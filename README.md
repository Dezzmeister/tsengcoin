# TsengCoin Core

This project consists of several parts which each warrant their own documentation:

- [Networking](./docs/Networking.md)
- [Wallets](./docs/Wallets.md)
- [Transactions](./docs/Transactions.md)
- [Blocks](./docs/Blocks.md)
- [Mining](./docs/Mining.md)
- [TsengScript](./docs/TsengScript.md)
- [Chain Requests](./docs/Chain-Requests.md)
- [Optimizations](./docs/Optimizations.md)
- [Test Node Setup](./docs/Node-Setup-Steps.md)

## Building

The main crate here is `tsengcoin-core`. To run the core client you should first `cd tsengcoin-core` then do `cargo build`, `cargo run`, etc.

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

If you're testing a new TsengCoin network, you will need to start a node up that doesn't know anything about the blockchain and doesn't try to connect to anyone. This will be a "seed node" to which the next nodes can connect. You can use `cargo run start-seed` for that - do `cargo run help start-seed` to get more information.

You can then connect to this node with `cargo run connect`. If you're testing mining, you will likely want to build with the `--release` flag and test a release binary - this is orders of magnitude faster than `cargo run`.
