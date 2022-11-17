use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    /// Protocol version
    pub version: u32,
    /// Input transactions
    pub inputs: (),
    /// Recipients
    pub outputs: (),
    /// Some metadata
    pub meta: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TxnOutput {
    amount: u64,
    lock_script: Script
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ScriptType {
    TSENGSCRIPT
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Script {
    code: String,
    script_type: ScriptType
}
