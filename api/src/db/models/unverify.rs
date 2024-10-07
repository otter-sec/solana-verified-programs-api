use serde::Deserialize;
use serde_json::Value;

// Structs representing the JSON structure of the payload
#[allow(dead_code, non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct UpgradeProgramInstruction {
    pub description: String,
    #[serde(rename = "type")]
    pub instruction_type: String,
    pub source: String,
    pub fee: u64,
    pub feePayer: String,
    pub signature: String,
    pub slot: u64,
    pub timestamp: u64,
    pub tokenTransfers: Vec<Value>, // Adjust based on the actual structure of token transfers
    pub nativeTransfers: Vec<Value>, // Adjust based on the actual structure of native transfers
    pub accountData: Vec<AccountData>,
    pub transactionError: Option<String>,
    pub instructions: Vec<Instruction>,
    pub events: Value,
}

#[allow(dead_code, non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct AccountData {
    pub account: String,
    pub nativeBalanceChange: i64,
    pub tokenBalanceChanges: Vec<Value>, // Adjust based on the actual structure of token balance changes
}

#[allow(dead_code, non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct Instruction {
    pub accounts: Vec<String>,
    pub data: String,
    pub programId: String,
    pub innerInstructions: Vec<Value>, // Adjust based on the actual structure of inner instructions
}
