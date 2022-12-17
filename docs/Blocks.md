# Blocks

A block is simply a group of transactions that has been accepted and confirmed by the network. Blocks are identified by their SHA256 hash - in the core client you can do `getblock <hash>` and print information about the block with the given hash. Blocks contain a block header and a group of transactions. The block header contains some information about the block:

- `version`: What version of the TsengCoin protocol this block obeys. Currently just 1
- `prev_hash`: The SHA256 hash of the previous block
- `merkle_root`: The Merkle root of the transactions in the block. See [Merkle Root](#merkle-root) below
- `timestamp`: The approximate creation time of this block, give or take two hours. Unit is seconds since Unix epoch
- `difficulty_target`: The difficulty target of the network when this block was created
- `nonce`: A number used by miners to meet the proof of work requirement. More info in [Mining](./Mining.md)
- `hash`: The SHA256 hash of this block. Used to identify the block in the blockchain

Generally, the blockchain will be arranged in a sequential, linked-list type of way (although this is not how it's represented in code). Together, the list of blocks is called a blockchain, and the blockchain implements a mostly immutable database. This is not always the case though - in some rare cases, the blockchain can become forked.

## Forks

A fork occurs when two nodes propose valid blocks at almost the same time, and both blocks are accepted by different parts of the network. Some nodes will accept one block, and the other nodes will accept the other. Depending on the size of the network, the distribution of miners, and the frequency with which blocks are found, forks can range from being nearly impossible to only somewhat rare. In a small network like ours, forks are extremely rare because valid blocks propagate quickly throughout the network. In a large network like Bitcoin or in a network with high fragmentation, forks can be much more common (but still rare). In a network like this it's possible for two distant miners to propose valid blocks at nearly the same time. Before one miner's block has time to propagate to all nodes, the remaining nodes have already received the other miner's block and added it to the blockchain. This is a huge problem because now we don't have consensus.

TsengCoin solves this in a similar way to Bitcoin. When we have a fork, we create a fork chain and add the fork block to the chain. When a new block comes in, we add it to the corresponding chain and then try to resolve the fork. A fork is resolved when one chain has a higher cumulative difficulty than all the others. The chain with the highest difficulty is the valid chain, and the other blocks must be removed from the chain. (In practice you can just sum up the difficulty targets for each chain - this is easy, but it isn't efficient because you're adding potentially thousands of 256-bit integers. We did it because our network is small.) The winning chain is made into the main chain, and the blocks in any rejected chains are removed. Their transactions (except coinbase transactions) are then added back into the pending transaction pool to be included in future blocks. Note that some of these transactions may no longer be valid. If for example the same UTXO is spent in two separate chains, then the losing chain will contain an invalid transaction when it is unwound because you can't spend the same UTXO twice. This may happen if the miners who produced a fork included the same transaction in their blocks. Because of this problem, the core client will validate each transaction before adding it back to the pending pool.

## Merkle Root

In Bitcoin, Merkle trees generally serve two purposes:

1. If you have transaction data, they allow you to verify that a block contains the transactions it claims to contain
2. If you don't have transaction data, they allow you to verify that a given transaction is in a block by means of a Merkle path proof. We don't have SPV nodes or any non-full nodes in TsengCoin, so we didn't implement this.

[Chapter 7](https://www.oreilly.com/library/view/mastering-bitcoin/9781491902639/ch07.html) of the Mastering Bitcoin book by Andreas M. Antonopoulos explains Merkle trees very well. Our core client computes the Merkle root of the transactions in a block and uses this to verify that the block does indeed contain the transactions it claims to. Merkle roots also guarantee that the order of transactions in a block can't be tampered with. This is important, because an attacker might want to reorder transactions to invalidate one or more of them and therefore invalidate the whole block.

## Verification Rules

When a block is received from a peer, the block is validated according to the following rules:

- The block cannot be more than 16kb in size
- The block cannot have zero transactions
- The previous block _should_ exist (if it doesn't, then the new block is an orphan - it's still accepted)
- The current difficulty in the block header must be the actual current difficulty
- The block hash must be less than the current difficulty target
- The block header's hash must be correct
- The timestamp on the block cannot be more than +/- 2 hours off from the current time
- Every transaction in the block must be valid
- The first transaction in the block must be the coinbase transaction
- The amount in the coinbase transaction must be the block reward plus fees.
- The block's Merkle root must be correct
