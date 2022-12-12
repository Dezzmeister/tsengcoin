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

`cd` into the main crate and make a release build:

```sh
cd tsengcoin/tsengcoin-core
cargo build --release --features "debug"
```

This will create a release build with debug features. You can include other features if you want a miner:

```sh
cargo build --release --features "debug, cl_miner"
```
