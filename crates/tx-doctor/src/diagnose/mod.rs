//! Maps one failed transaction to one root cause, with the log line that proves it.
//! Plain code, no model. Error codes are program-specific (0x1 means "insufficient
//! funds" in the Token program and something else elsewhere), so codes are looked up
//! by the failing program id first, and Anchor framework codes count only when that
//! program logged an AnchorError.

mod causes;
mod frame;
mod programs;
mod rules;

pub use causes::Cause;
pub use programs::{known_program, KnownProgram, ProgramError, FAILURE_LAB_ID};
pub use rules::diagnose;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The `meta` object of a `getTransaction` result, reduced to what the rules read.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxMeta {
    #[serde(default)]
    pub err: Value,
    #[serde(default)]
    pub fee: u64,
    #[serde(default)]
    pub compute_units_consumed: Option<u64>,
    #[serde(default)]
    pub log_messages: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Diagnosis {
    pub cause: Cause,
    pub title: &'static str,
    pub fix: &'static str,
    /// The log line or error field that proves the cause.
    pub evidence: String,
    /// Zero-based, as in meta.err. Solana Explorer shows this plus one.
    pub instruction_index: Option<u64>,
    pub program_id: Option<String>,
    pub program_name: Option<&'static str>,
    pub error_code: Option<u64>,
    pub error_name: Option<String>,
    pub fee_lamports: u64,
    pub compute_units: Option<u64>,
}
