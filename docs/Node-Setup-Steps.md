# How to set up a test node

Our test net runs on Google Cloud using their VM instances. Each node runs Ubuntu 18.04.

First, install a C compiler toolchain:

```sh
sudo apt-get update
sudo apt-get install build-essential
```

Install the Rust toolchain:

```sh
sudo curl https://sh.rustup.rs -sSf | sh -s -- --verbose
```

`cd` into the directory in which TsengCoin Core will run:

```sh
cd /opt
```

Clone the repo into the directory and give yourself ownership:

```sh
https://github.com/Dezzmeister/tsengcoin.git
sudo chown -R <user> tsengcoin/
```

`cd` into the main crate (`tsengcoin-core`) and make a headless release build:

```sh
cd tsengcoin/tsengcoin-core
cargo build --release --features "debug"
```

This will create a release build with debug features and no GUI. You can include other features if you want a miner:

```sh
cargo build --release --features "debug, cl_miner"
```

Optionally you can just use one of the configured builds:

```sh
cargo build-headless-cl
```

_See [config.toml](../tsengcoin-core/.cargo/config.toml) for more build options. Because our test net nodes are running in a server environment without a desktop, they are missing many of the graphical libraries needed to link the GUI application, and they can only run the core client in headless mode._

The release binary will be in `tsengcoin-core/target/release`.

## Alternate Option

You can run [test-update.sh](../tsengcoin-core/test-update.sh) to pull the latest changes from the repo, make a headless release build, and move the binary to `tsengcoin-core`. Then you can run `./tsengcoin-core ...`
