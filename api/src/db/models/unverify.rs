use serde::Deserialize;
use serde_json::Value;

/// Represents an upgrade program instruction
#[allow(dead_code, non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct UpgradeProgramInstruction {
    /// Description of the instruction
    pub description: String,
    /// Type of instruction
    #[serde(rename = "type")]
    pub instruction_type: String,
    /// Source of the instruction
    pub source: String,
    /// Transaction fee
    pub fee: u64,
    /// Fee payer's address
    pub feePayer: String,
    /// Transaction signature
    pub signature: String,
    /// Blockchain slot number
    pub slot: u64,
    /// Transaction timestamp
    pub timestamp: u64,
    /// Token transfer details
    pub tokenTransfers: Vec<Value>,
    /// Native token transfer details
    pub nativeTransfers: Vec<Value>,
    /// Account data changes
    pub accountData: Vec<AccountData>,
    /// Transaction error if any
    pub transactionError: Option<String>,
    /// List of instructions in the transaction
    pub instructions: Vec<Instruction>,
    /// Associated events
    pub events: Value,
}

/// Represents account data changes in a transaction
#[allow(dead_code, non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct AccountData {
    /// Account address
    pub account: String,
    /// Change in native token balance
    pub nativeBalanceChange: i64,
    /// Changes in token balances
    pub tokenBalanceChanges: Vec<Value>,
}

/// Represents an instruction in a transaction
#[allow(dead_code, non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct Instruction {
    /// List of account addresses involved
    pub accounts: Vec<String>,
    /// Instruction data
    pub data: String,
    /// Program ID that processes this instruction
    pub programId: String,
    /// Inner instructions generated during execution
    pub innerInstructions: Vec<Value>,
}
