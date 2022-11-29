use num_bigint::BigUint;

use crate::tsengscript_interpreter::{Token, execute};

use super::{verify_error::{VerifyResult}, transaction::{Transaction, MAX_TXN_AMOUNT, UnsignedTransaction, MIN_TXN_FEE, UnhashedTransaction, hash_txn}, block::MAX_BLOCK_SIZE, state::State};
use super::verify_error::ErrorKind::EmptyInputs;
use super::verify_error::ErrorKind::EmptyOutputs;
use super::verify_error::ErrorKind::TooLarge;
use super::verify_error::ErrorKind::OutOfRange;
use super::verify_error::ErrorKind::Coinbase;
use super::verify_error::ErrorKind::InvalidUTXOIndex;
use super::verify_error::ErrorKind::Script;
use super::verify_error::ErrorKind::BadUnlockScript;
use super::verify_error::ErrorKind::Overspend;
use super::verify_error::ErrorKind::LowFee;
use super::verify_error::ErrorKind::DoubleSpend;
use super::verify_error::ErrorKind::InvalidHash;
use super::verify_error::ErrorKind::ZeroOutput;

/// Verifies the transaction according to an independent set of rules. If there are no errors,
/// returns 'true' if the transaction is an orphan, and false if not. If the transaction is an orphan,
/// it should be added to the pending transactions pool. This function does not mutate the state in any way
/// so adding valid transactions to their respective pools is the caller's responsibility.
pub fn verify_transaction(tx: Transaction, state: &State) -> VerifyResult<bool> {
    let utxos = &state.blockchain.utxo_pool;

    // Transaction must have at least 1 input
    if tx.inputs.len() == 0 {
        return Err(Box::new(EmptyInputs));
    }

    // Transaction must have at least 1 output
    if tx.outputs.len() == 0 {
        return Err(Box::new(EmptyOutputs));
    }

    // Transaction cannot be too big to fit into a block
    if tx.size() > MAX_BLOCK_SIZE {
        return Err(Box::new(TooLarge));
    }

    let output_sum = tx.outputs.iter().fold(0, |a, e| a + e.amount);

    // Total output must be less than the max value
    if output_sum > MAX_TXN_AMOUNT {
        return Err(Box::new(OutOfRange(output_sum)));
    }

    // Transaction outputs must be nonzero
    for output in &tx.outputs {
        if output.amount == 0 {
            return Err(Box::new(ZeroOutput));
        }
    }

    let zero_hash = tx.inputs.iter().find(|i| i.txn_hash == [0; 32]);

    // Only coinbase transactions can have a zero hash. Coinbase transactions should not be
    // relayed
    if zero_hash.is_some() {
        return Err(Box::new(Coinbase));
    }

    // The transaction hash must be valid
    let unhashed_tx: UnhashedTransaction = (&tx).into();
    let hash_res = hash_txn(&unhashed_tx);
    match hash_res {
        Err(_) => return Err(Box::new(InvalidHash)),
        Ok(hash) if hash != tx.hash => return Err(Box::new(InvalidHash)),
        _ => ()
    };

    let unsigned_tx: UnsignedTransaction = (&tx).into();
    let msg_data = bincode::serialize(&unsigned_tx).unwrap();
    let msg_data_bigint = BigUint::from_bytes_be(&msg_data);

    // Initialize a TsengScript stack with the transaction data. This is the same
    // data that the sender would have signed
    let init_stack: Vec<Token> = vec![Token::UByteSeq(msg_data_bigint)];

    let mut input_sum = 0;

    for input in tx.inputs {

        // Each input has to reference a valid UTXO. If not, the transaction is an orphan
        // and must be added to the orphan pool.
        let utxo_opt = utxos.utxos.iter().find(|u| u.txn == input.txn_hash);

        // If the UTXO does not exist, then one of two possibilities is true.
        //  1. The UTXO has already been spent (double spend; reject txn)
        //  2. The UTXO never existed (orphan transaction; goes in orphan pool)
        // We need to check if the input transaction hash exists in order to determine
        // which is true.
        if utxo_opt.is_none() {
            let input_opts = (state.blockchain.find_txn(input.txn_hash), state.get_pending_txn(input.txn_hash));

            match input_opts {
                (None, None) => return Ok(true),
                _ => return Err(Box::new(DoubleSpend(input.txn_hash, input.output_idx)))
            };
        }

        let utxo = utxo_opt.unwrap();

        // UTXO must point to a valid block or to a pending transaction
        let txn = match utxo.block {
            Some(block_hash) => {
                let block_opt = state.blockchain.get_block(block_hash);
        
                if block_opt.is_none() || utxo.txn != input.txn_hash {
                    return Err(Box::new(InvalidUTXOIndex));
                }

                let (block, _, _) = block_opt.unwrap();
                let txn_opt = block.get_txn(utxo.txn);

                // UTXO must point to a transaction in the block
                if txn_opt.is_none() {
                    return Err(Box::new(InvalidUTXOIndex));
                }
            
                txn_opt.unwrap()
            },
            None => {
                let txn_opt = state.get_pending_txn(utxo.txn);

                // UTXO must point to a transaction in the pending pool
                if txn_opt.is_none() {
                    return Err(Box::new(InvalidUTXOIndex));
                }

                txn_opt.unwrap()
            }
        };        

        // Output index must be valid
        if input.output_idx >= txn.outputs.len() {
            return Err(Box::new(InvalidUTXOIndex));
        }

        // If the UTXO output does not exist then there is only one possibility,
        // that the output has already been spent. We already know that our output index is
        // valid, so if the UTXO is missing our valid output index, then it must be because some other
        // transaction has spent it.
        if !utxo.outputs.contains(&input.output_idx) {
            return Err(Box::new(DoubleSpend(input.txn_hash, input.output_idx)));
        }

        let output = &txn.outputs[input.output_idx];
        let lock_script = &output.lock_script;
        let unlock_script = input.unlock_script;
        
        // The unlocking script provided in this transaction has to run first.
        // When it runs, the only item on the stack is the transaction data which was signed by the
        // sender. The unlock script will finish, leaving some data on the stack.
        let unlock_result = execute(&unlock_script.code, &init_stack);
        if unlock_result.is_err() {
            return Err(Box::new(Script(unlock_result.err().unwrap())));
        }

        // The locking script is run with whatever was left on the stack by the unlocking script.
        // When the locking script finishes, the top item on the stack must be TRUE for the
        // input to be valid.
        let next_stack = unlock_result.unwrap().stack;
        let lock_result = execute(&lock_script.code, &next_stack);
        if lock_result.is_err() {
            return Err(Box::new(Script(lock_result.err().unwrap())));
        }

        let script_res = lock_result.unwrap().top;
        match script_res {
            Some(Token::Bool(true)) => (),
            _ => return Err(Box::new(BadUnlockScript(txn.hash, input.output_idx))),
        };

        input_sum += output.amount;
    }

    // Transaction outputs cannot be more than inputs - you can't spend more
    // than you have
    if input_sum < output_sum {
        return Err(Box::new(Overspend(input_sum, output_sum)));
    }

    // Transaction input amount total cannot be more than the max transaction amount
    if input_sum > MAX_TXN_AMOUNT {
        return Err(Box::new(TooLarge));
    }

    let fee = input_sum - output_sum;

    // There is a minimum transaction fee
    if fee < MIN_TXN_FEE {
        return Err(Box::new(LowFee(fee)));
    }

    Ok(false)
}
