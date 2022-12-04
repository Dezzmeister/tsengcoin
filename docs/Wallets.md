# Wallets

Wallets in TsengCoin are a little different from wallets in Bitcoin. In Bitcoin, a wallet can have multiple addresses to which payments are made. In TsengCoin, a wallet has one and only one corresponding address. The "wallet" is not an application but instead a file containing an ECDSA keypair. The keypair consists of a private and public key, and the file is encrypted with an AES block cipher. The key used for AES encryption/decryption is generated from a password based key derivation function (PBKDF2).

## Creating a Wallet

You can use `cargo run create-address` to create a wallet and address. You will need to provide a password and a file to save the keypair to. The command will print your address, which can be used to send and receive TsengCoin. The address will look something like this: `2LuJkN1xDRRM2R2h2H4qnSspy4qmwoZfor`. The address is created by taking the public key, hashing it, and encoding it in base58check:

```
ADDRESS = BASE58CHECK(RIPEMD160(SHA256(PUBLIC_KEY)))
```

This is the address in encoded form; in transactions and in code the address is represented as an unencoded 20-byte RIPEMD160 hash. An address can be converted between the two forms with base58check using the prefix 0x03. This prefix ensures that all addresses in encoded form start with a 2.

## Using a Wallet

When you connect to the TsengCoin network with the core client, you need to unlock your wallet first. When running `start-seed` or `connect` you provide a path to the wallet file and the password to the wallet. If these are valid, your wallet will be decrypted and you'll be able to make transactions and use the client with your address.

When you want to spend some TsengCoin that someone else sent to you, you must construct a transaction with one or more inputs pointing to previous transactions in which you received TsengCoin. You must prove that you can spend each input with an unlock script (more details in [Transactions](./Transactions.md)). For [P2PKH](./Transactions.md#pay-to-public-key-hash) transactions, this is all handled for you by the client. The address plays a crucial role in veriyfing P2PKH transactions - because it is a hash of a public key, it can be used to specify a recipient of TsengCoin. The person owning the corresponding private key can prove ownership by taking some data (in this case, transaction data), signing it, and providing the signature as well as the public key. Anyone looking to verify the transaction can reproduce the signed data, verify the signature with the public key, and then hash the public key to see if it matches the intended recipient's address. This verification will fail if the transaction data was tampered with, if the wrong private key was used, or if the wrong public key is provided. This is explained in much more detail in [Transactions](./Transactions.md), but the basic idea is that valid P2PKH transactions can only be produced by people who are authorized to spend the listed inputs.
