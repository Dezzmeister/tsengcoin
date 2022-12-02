use chrono::{Utc};

use super::block::{RawBlockHeader, hash_block_header, MAX_BLOCK_SIZE, BLOCK_TIMESTAMP_TOLERANCE, make_merkle_root};
use super::transaction::{BLOCK_REWARD, UnhashedTransaction, hash_txn, Transaction, build_utxos_from_confirmed, compute_input_sum};
use super::txn_verify::{verify_transaction, check_pending_and_orphans};
use super::{block_verify_error::BlockVerifyResult, block::Block, state::State};

use super::block_verify_error::ErrorKind::IncorrectDifficulty;
use super::block_verify_error::ErrorKind::FailedProofOfWork;
use super::block_verify_error::ErrorKind::InvalidHeaderHash;
use super::block_verify_error::ErrorKind::OldBlock;
use super::block_verify_error::ErrorKind::TooLarge;
use super::block_verify_error::ErrorKind::EmptyBlock;
use super::block_verify_error::ErrorKind::TxnError;
use super::block_verify_error::ErrorKind::OrphanTxn;
use super::block_verify_error::ErrorKind::InvalidCoinbase;
use super::block_verify_error::ErrorKind::InvalidCoinbaseAmount;
use super::block_verify_error::ErrorKind::InvalidMerkleRoot;

/// Verifies a new block. Returns true if the block is an orphan. Unlike [verify_transaction],
/// this function will mutate the state. If the block is an orphan, it will add the block to the orphan
/// pool; otherwise it will add the block to the blockchain. It is the caller's job to check the blockchain
/// afterward and try to resolve any forks.
pub fn verify_block(block: Block, state: &mut State) -> BlockVerifyResult<bool> {
    let block_size = block.size();

    // The block cannot be too big
    if block_size > MAX_BLOCK_SIZE {
        return Err(Box::new(TooLarge(MAX_BLOCK_SIZE, block_size)));
    }

    // The block cannot be empty
    if block.transactions.len() == 0 {
        return Err(Box::new(EmptyBlock));
    }

    // Get the previous block in the blockchain. There may not be such a block: if there isn't,
    // then this new block is an orphan.
    let prev_block_opt = state.blockchain.get_block(block.header.prev_hash);
    let (_, chain_idx, pos) = match prev_block_opt {
        None => {
            state.blockchain.orphans.push(block);
            return Ok(true);
        },
        Some(data) => data
    };

    // Get the blocks leading up to where this one should go
    let block_path = state.blockchain.get_blocks_rel(chain_idx, 0, pos + 1);

    let current_difficulty = state.blockchain.current_difficulty();

    // The block must have the correct difficulty
    if current_difficulty != block.header.difficulty_target {
        return Err(Box::new(IncorrectDifficulty));
    }

    let block_hash = block.header.hash;

    // The hash must satisy proof of work
    if block_hash >= current_difficulty {
        return Err(Box::new(FailedProofOfWork));
    }

    let unhashed: RawBlockHeader = (&block.header).into();
    let hash = hash_block_header(&unhashed);

    // The reported hash must be the actual block hash
    if hash != block.header.hash {
        return Err(Box::new(InvalidHeaderHash));
    }

    let now: u64 = Utc::now().timestamp().try_into().unwrap();

    let time_diff = now - block.header.timestamp;

    // The block cannot have a timestamp too far in the past or too far in the future
    if time_diff > BLOCK_TIMESTAMP_TOLERANCE.num_seconds().try_into().unwrap() {
        return Err(Box::new(OldBlock));
    }

    // Unwind pending UTXOs before validating transactions, because a valid block should not contain
    // unconfirmed transactions. We "unwind UTXOs" by rebuilding the entire UTXO database up
    // to the previous block. This means that any UTXOs from pending transactions will be
    // discarded, so we will need to re-validate pending transactions before returning
    // from this function. UTXOs from fork blocks will also be discarded but this isn't such a big issue:
    // forks are rare and at worst a transaction or two gets dropped by our node. It will likely reappear
    // in a future block anyway so we don't really need to worry about this.
    state.blockchain.utxo_pool = build_utxos_from_confirmed(&block_path);

    // A transaction in the new block can only depend on transactions that came before it. This means that
    // when we verify each new transaction, they can't depend on anything in the pending pool. As we verify
    // each transaction in the block we will add it to the pending pool so that future transactions in the block
    // can depend on it.
    let old_pending = state.pending_txns.clone();
    state.pending_txns = vec![];
    let mut pending_to_remove: Vec<usize> = vec![];
    let mut orphans_to_remove: Vec<usize> = vec![];

    let coinbase = &block.transactions[0];
    let mut total_fees: u64 = 0;

    // First add the coinbase transaction as an unconfirmed UTXO. This needs to happen before we
    // actually verify the transaction because future transactions may depend on it.
    state.blockchain.utxo_pool.update_unconfirmed(&coinbase);

    // Verify each transaction separately
    for txn in &block.transactions[1..] {
        let verify_result = verify_transaction(txn.clone(), state);

        // If returning an error, we need to restore the UTXO database to its previous state.
        // We also need to restore any pending transactions we removed from the pending transaction
        // pool.
        match verify_result {
            Ok(true) => {
                restore_utxo_pool(state, &block_path, old_pending);
                return Err(Box::new(OrphanTxn(txn.hash)));
            },
            Err(error) => {
                restore_utxo_pool(state, &block_path, old_pending);
                return Err(Box::new(TxnError(error, txn.hash)));
            },
            _ => ()
        };

        // Now that the transaction is verified, find it in the pending txns pool and add
        // the index to an array (if it exists in the pool). When the block has been validated, 
        // these pending transactions will be removed from the pool. The transaction may also
        // exist in the orphan pool so we want to look there too.
        let pos_in_pending = old_pending.iter().position(|p| p.hash == txn.hash);
        if pos_in_pending.is_some() {
            pending_to_remove.push(pos_in_pending.unwrap());
        }

        let pos_in_orphans = state.orphan_txns.iter().position(|p| p.hash == txn.hash);
        if pos_in_orphans.is_some() {
            orphans_to_remove.push(pos_in_orphans.unwrap());
        }
        // Add it to the pending pool so that validation of future transactions
        // can find it. We can't add it as a confirmed transaction because the block containing
        // it does not yet exist on the blockchain.
        state.pending_txns.push(txn.clone());
        state.blockchain.utxo_pool.update_unconfirmed(txn);

        // Add up the input amounts and output amounts and compute the fee
        let input_sum: u64 = compute_input_sum(txn, state);

        let output_sum = 
            txn.outputs
                .iter()
                .fold(0, |a, e| a + e.amount);
        
        total_fees += input_sum - output_sum;
    }

    // Now verify the coinbase transaction

    // The coinbase transaction must have exactly one input and one output
    if coinbase.inputs.len() != 1 || coinbase.outputs.len() != 1 {
        restore_utxo_pool(state, &block_path, old_pending);
        return Err(Box::new(InvalidCoinbase));
    }

    let input = &coinbase.inputs[0];
    let output = &coinbase.outputs[0];

    // The transaction's sole input hash must be zero
    if input.txn_hash != [0; 32] {
        restore_utxo_pool(state, &block_path, old_pending);
        return Err(Box::new(InvalidCoinbase));
    }

    let expected_amount = BLOCK_REWARD + total_fees;

    // The miner must have claimed the expected amount
    if output.amount != expected_amount {
        return Err(Box::new(InvalidCoinbaseAmount(expected_amount, output.amount)));
    }

    // The reported transaction hash must match the actual hash
    let unhashed: UnhashedTransaction = coinbase.into();
    let expected_hash = match hash_txn(&unhashed) {
        Err(_) => {
            restore_utxo_pool(state, &block_path, old_pending);
            return Err(Box::new(InvalidCoinbase));
        },
        Ok(hash) => hash
    };

    if coinbase.hash != expected_hash {
        restore_utxo_pool(state, &block_path, old_pending);
        return Err(Box::new(InvalidCoinbase));
    }

    // The merkle root needs to match the actual merkle root
    let expected_merkle_root = make_merkle_root(&block.transactions);
    if expected_merkle_root != block.header.merkle_root {
        restore_utxo_pool(state, &block_path, old_pending);
        return Err(Box::new(InvalidMerkleRoot));
    }

    // At this point, the block is valid. Now we just need to do some bookkeeping and update our UTXO
    // database, pending transaction pool, and orphan transaction pool.

    /*
    for utxo in &mut state.blockchain.utxo_pool.utxos {
        if utxo.block.is_none() {
            utxo.block = Some(block.header.hash);
        }
    }
    */

    state.pending_txns = old_pending;

    pending_to_remove.sort();
    orphans_to_remove.sort();

    for i in (0..pending_to_remove.len()).rev() {
        let pos = pending_to_remove[i];
        state.pending_txns.remove(pos);
    }

    for i in (0..orphans_to_remove.len()).rev() {
        let pos = orphans_to_remove[i];
        state.orphan_txns.remove(pos);
    }

    // We can't leave the blockchain in an invalid state. We must add the newly verified block to the
    // blockchain before returning
    state.add_block(block);

    // At this point, all the unconfirmed UTXOs in the UTXO pool are from the block we just verified.
    // Now that the block has been added to the blockchain, we can confirm those and then
    // add the pending transactions again.
    state.blockchain.utxo_pool.confirm(block_hash);

    // Add the pending transactions and check orphans as well
    check_pending_and_orphans(state);

    Ok(false)
}

fn restore_utxo_pool(state: &mut State, utxo_blocks: &Vec<Block>, old_pending: Vec<Transaction>) {
    // First restore the old pending transactions
    state.pending_txns = old_pending;

    // Next rebuild the UTXO pool to undo any transactions that were included
    // in the bad block
    state.blockchain.utxo_pool = build_utxos_from_confirmed(utxo_blocks);

    let mut pending_to_remove: Vec<usize> = vec![];

    for i in 0..state.pending_txns.len() {
        let txn = &state.pending_txns[i];

        let verify_result = verify_transaction(txn.clone(), state);
        match verify_result {
            // We shouldn't have any orphans here
            Ok(true) => {
                println!("Unexpected orphan");
                pending_to_remove.push(i);
            },
            Err(err) => {
                // This is weird and shouldn't happen
                println!("Rejecting a pending transaction that was once valid: {}", err.to_string());
                pending_to_remove.push(i);
            },
            Ok(false) => {
                state.blockchain.utxo_pool.update_unconfirmed(txn);
            }
        }
    }

    pending_to_remove.sort();

    for i in (0..pending_to_remove.len()).rev() {
        let pos = pending_to_remove[i];
        state.pending_txns.remove(pos);
    }
}
