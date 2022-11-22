use serde::{Serialize, Deserialize};

use crate::wallet::Hash256;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    /// Protocol version
    pub version: u32,
    /// Input transactions
    pub inputs: Vec<TxnInput>,
    /// Recipients
    pub outputs: Vec<TxnOutput>,
    /// Some metadata
    pub meta: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TxnOutput {
    amount: u64,
    lock_script: Script
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TxnInput {
    txn_hash: Hash256,
    block_hash: Hash256,
    unlock_script: Script,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ScriptType {
    TsengScript
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Script {
    code: String,
    script_type: ScriptType
}
