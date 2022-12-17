# Transactions

Transactions in TsengCoin are very much like transactions in Bitcoin. Each transaction has a set of inputs and outputs. The inputs point to previous transaction outputs in which the sender received some TsengCoin, and the outputs specify how to spend the TsengCoin accumulated in the inputs. Whatever is left over from the inputs is taken by miners as a transaction fee:

```
fee = inputs - outputs
```

This means that when a transaction output is used as a valid input somewhere else, that TsengCoin can no longer be spent; the transaction must spend all of it. Let's say that there are have two separate transactions in which you received 500 TsengCoin and 250 TsengCoin respectively. You want to send 600 TsengCoin to your friend, and you're willing to pay a transaction fee of 10 TsengCoin. You will need to create a transaction that pools these two previous transactions as inputs, because neither is enough to cover the 610 TsengCoin you want to spend. You will also need to create a transaction output in which you authorize your friend to 600 TsengCoin. Of course, there is still 140 TsengCoin left over that you don't want to spend, so you need to create a second output in which you receive 140 TsengCoin from your own transaction. This is a lot like receiving change from a cashier - if you can only pay in high value bills, then you will receive some change back. In this case, the total input to your transaction is 750 TsengCoin, and the total output is 740 TsengCoin. The difference is 10 TsengCoin, which will be taken by a miner as a transaction fee.

A transaction in TsengCoin is uniquely identified by its hash. The hash is computed by serializing all of the transaction's fields and hashing the data with SHA256.

## Confirmed/Unconfirmed Transactions

The distinction between a confirmed and an unconfirmed transaction is that confirmed transactions exist in blocks on the blockchain and have therefore been "confirmed." The number of confirmations of a transaction/block is the depth of the transaction/block on the blockchain. Higher confirmations means that an item is unlikely to be removed from the blockchain. See [Blocks](./Blocks.md) for a more detailed explanation of the conditions that might cause blocks to be removed from the blockchain.

If a confirmed transaction is any that lives in a verified and connected block, then an unconfirmed transaction is any valid transaction that does not. The pending transaction pool and orphan transaction pool both contain unconfirmed transactions, waiting for miners to group them into blocks and confirm them.

1. The pending pool and orphan pool must ALWAYS contain only valid transactions.
2. The logical order of transactions must always be preserved! At no point can a confirmed transaction depend on an unconfirmed transaction. This is enforced when new blocks are validated, and this is preserved by the block's Merkle root.

## Authorization Methods

A transaction output does not directly specify a recipient of some amount of TsengCoin. Instead, the output specifies a condition that must be met in order for the recipient to claim the TsengCoin. This condition is encoded as a script (written in [TsengScript](./TsengScript.md)). The condition specified in the transaction is called the locking script. Anyone who wants to claim the transaction output must provide an unlocking script that satisifes the condition imposed by the locking script. In order for a transaction input to satisfy the condition imposed by a previous transaction output, the following is done:

1. A stack is initialized with the unsigned transaction data and the unlocking script is run.
2. After the unlocking script finishes, it leaves the stack in some state. The locking script is run with the left over stack.
3. The locking script finishes, leaving one or more items on the stack. The transaction input satisfies the locking condition only if the topmost item is the boolean value `TRUE`.

In theory, the locking script could specify any condition, as long as it can be encoded in TsengScript. The unlocking script would need to know how to produce the right stack to satisfy the locking script. In practice, there are a few different types of scripts that Bitcoin recognizes as "standard." These correspond to different ways of authorizing another person (or multiple people) to spend a transaction output. The most common one of these is P2PKH, or pay to public key hash.

### Pay to Public Key Hash

In P2PKH, the sender specifies the recipient's "public key hash" in a lock script of the following format:

```
DUP HASH160 <address> REQUIRE_EQUAL CHECKSIG
```

The "public key hash" is just an address decoded from base58check to hex. For example, the genesis block contains a coinbase transaction with one output containing the address `2LuJkN1xDRRM2R2h2H4qnSspy4qmwoZfor`. The output script looks like this:

```
DUP HASH160 5686215dbe4915045db3def6ab7172a1bdf3e6e4 REQUIRE_EQUAL CHECKSIG
```

The hex string `5686215dbe4915045db3def6ab7172a1bdf3e6e4` produces the address `2LuJkN1xDRRM2R2h2H4qnSspy4qmwoZfor` when encoded in base58check. The hex string is produced by taking the user's public key and passing it through two hash functions, like such:

```
Address_bytes = RIPEMD160(SHA256(pubkey))
```

This is why a script of this format is called "pay to public key hash." A P2PKH locking script is satisfied by an unlocking script of the form:

```
<signature> <public_key>
```

Remember, the unlocking script is run before the locking script, so the final script that needs to result in `TRUE` looks like this:

```
<signature> <public_key> DUP HASH160 5686215dbe4915045db3def6ab7172a1bdf3e6e4 REQUIRE_EQUAL CHECKSIG
```

Before the script is run, a stack is initialized with the transaction data that the recipient claims to have signed. The script starts running, and the two hex literals at the beginning (signature and public key) are pushed on the stack. `DUP` copies the previous item on the stack, so that the stack contains:

```
<txn_data> | <signature> | <public_key> | <public_key>
```

`HASH160` transforms a raw public key into the hex bytes representing an unencoded address. This raw address is pushed on the stack, so that the stack looks like this:

```
<txn_data> | <signature> | <public_key> | <hashed_public_key>
```

Now the raw address in the locking script is pushed on the stack. In our case, this is `5686215dbe4915045db3def6ab7172a1bdf3e6e4`:

```
<txn_data> | <signature> | <public_key> | <hashed_public_key> | 5686215dbe4915045db3def6ab7172a1bdf3e6e4
```

The next instruction, `REQUIRE_EQUAL`, pops the top two items off the stack and compares them. If they are equal, it does nothing, and if they are not equal, it halts execution and throws an error. The presence of this instruction in the locking script indicates that the sender expects the recipient to use a public key that hashes to the given raw address. Assuming that the unlocking script provided such a public key and `REQUIRE_EQUAL` does not panic, the stack now looks like this:

```
<txn_data> | <signature> | <public_key>
```

If you look at the TsengScript documentation you will see that these are the exact arguments for the `CHECKSIG` instruction. `CHECKSIG` will check that the `signature` was generated by someone who signed the given `txn_data` with the private key corresponding to the given `public_key`. Assuming that a person's wallet has not been compromised, we know that if this check passes, then the unlocking script MUST have been generated by the person holding the private key matching `public_key`. `CHECKSIG` will push a boolean onto the stack indicating the result of this check - `TRUE` if the signature and public key match, `FALSE` if not. At this point the script has finished executing, so if `CHECKSIG` pushes `TRUE` then our unlocking script satisfies the condition set by the locking script.

### Custom Transactions

A transaction does not have to use the P2PKH scheme described above. There are many other ways to authorize recipients of a transaction, and you can write any locking script you want for each of your transaction outputs. It may not be valid, but you can do it. Custom transactions are any transaction in which one or more outputs has a locking script that does not match any scheme known to the client implementation. TODO: Add the ability to build up custom transactions with commands

## UTXOs

If you use TsengCoin for anything you will likely have transactions in which you received some TsengCoin that you haven't spent yet. (Remember that the transaction is not the thing authorizing you to TsengCoin, but instead an individual output of the transaction. Transactions can have multiple outputs that authorize different addresses to TsengCoin.) An unspent transaction output is a UTXO, and the core client maintains a database of all UTXOs (for all addresses). The core client's UTXO database consists only of outputs with locking scripts of a known format. As of 11/28/2022, the core client only includes P2PKH outputs in the UTXO database. This is because it is easy to determine the recipient of a P2PKH output; you can just extract the address and compare it to a known address. For example, if you want to get the TsengCoin balance for `2LuJkN1xDRRM2R2h2H4qnSspy4qmwoZfor`, you can just search the UTXO database for any UTXOs where the output script is P2PKH and the address is `5686215dbe4915045db3def6ab7172a1bdf3e6e4` (this is the base58check decoded raw address for `2LuJkN1xDRRM2R2h2H4qnSspy4qmwoZfor`).

Each UTXO in the database is a "transaction index" that points to either an unconfirmed or a confirmed transaction. In most cases, the UTXO database will contain a logical and valid transaction order, meaning that every transaction in the UTXO database is valid and all UTXOs are in the correct order, with confirmed UTXOs first and unconfirmed last. The exception to this is the verification function `verify_block`, which resets the global UTXO database during execution so that it can verify transactions in a block. This function takes care not to leave the UTXO database in an intermediate state though, so that any other functions can expect the UTXO database to be valid.

## Coinbase

Each block includes one transaction which is unlike the others. This is called the coinbase transaction, and it is the transaction that grants miners their reward. There are a few conditions that the coinbase transaction must satisfy. These are enforced by the `verify_block` function (instead of the usual `verify_transaction`) because coinbase transactions are not relayed over the network.

- The coinbase transaction must be the first and only coinbase transaction in a block
- It must have one input and one output
- The input hash must be zero, and the index must be 0xFFFF_FFFF. No other transaction will ever have this many outputs, so the presence of an input pointing to output index 0xFFFF_FFFF is a clear indicator of a coinbase transaction.
- The amount listed in the output must be the block reward plus block fees.

## Verification Rules

When a node receives a transaction from a peer, the transaction is verified according to the following rules:

- The transaction must have at least 1 input
- The transaction must have at least 1 output
- The transaction cannot be bigger than the max block size (16kb)
- The transaction cannot produce outputs totaling a sum greater than 1 billion TsengCoin
- The transaction cannot contain empty outputs (outputs with zero TsengCoin)
- None of the transaction's input hashes can be zero. This would indicate a coinbase transaction, which should not be relayed.
- The transaction's hash must be valid
- Every input must point to a valid UTXO
- Every input must successfully unlock the corresponding output
- The sum of transaction inputs cannot be greater than the sum of transaction outputs
- The total transaction input amount cannot be more than 1 billion TsengCoin
- The transaction fee cannot be zero
