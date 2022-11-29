use chrono::{Utc};

use super::block::{RawBlockHeader, hash_block_header, MAX_BLOCK_SIZE, BLOCK_TIMESTAMP_TOLERANCE, check_orphans};
use super::transaction::{BLOCK_REWARD, UnhashedTransaction, hash_txn, Transaction, build_utxos_from_confirmed};
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

    // The hash must satisy proof of work
    if block.header.hash >= current_difficulty {
        return Err(Box::new(FailedProofOfWork));
    }

    let unhashed: RawBlockHeader = (&block.header).into();
    let hash = hash_block_header(&unhashed);

    // The reported hash must be the actual block hash
    if hash != block.header.hash {
        return Err(Box::new(InvalidHeaderHash));
    }

    let now = Utc::now();

    let time_diff = (now - block.header.timestamp).num_seconds().abs();

    // The block cannot have a timestamp too far in the past or too far in the future
    if time_diff > BLOCK_TIMESTAMP_TOLERANCE.num_seconds() {
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
        state.blockchain.utxo_pool.update_unconfirmed(&txn);

        // Add up the input amounts and output amounts and compute the fee
        let mut input_sum: u64 = 0;
        for input in &txn.inputs {
            // We can just unwrap this because we know it must exist, since we just validated
            // the transaction. For the same reason we can just unwrap all the stuff
            // in this match expression
            let input_utxo = state.blockchain.utxo_pool.utxos.iter().find(|u| u.txn == input.txn_hash).unwrap();
            let input_txn = match input_utxo.block {
                Some(block_hash) => {
                    let (block, _, _) = state.blockchain.get_block(block_hash).unwrap();
                    block.get_txn(input_utxo.txn).unwrap()
                },
                None => {    
                    state.get_pending_txn(input_utxo.txn).unwrap()
                }
            };

            let amount = input_txn.outputs[input.output_idx].amount;

            input_sum += amount;
        }

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

    // TODO: Merkle tree check

    // At this point, the block is valid. Now we just need to do some bookkeeping and update our UTXO
    // database, pending transaction pool, and orphan transaction pool.

    for utxo in &mut state.blockchain.utxo_pool.utxos {
        if utxo.block.is_none() {
            utxo.block = Some(block.header.hash);
        }
    }

    state.pending_txns = old_pending;

    for pos in pending_to_remove {
        state.pending_txns.remove(pos);
    }

    for pos in orphans_to_remove {
        state.orphan_txns.remove(pos);
    }

    check_pending_and_orphans(state);

    // We can't leave the blockchain in an invalid state. We must add the newly verified block to the
    // blockchain before returning
    state.blockchain.add_block(block);

    // We also need to check orphan blocks and see if they can be added too. If so, we will need to re-verify them
    // and add them only if they're valid.
    check_orphans(state);

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

    for pos in pending_to_remove {
        state.pending_txns.remove(pos);
    }
}
