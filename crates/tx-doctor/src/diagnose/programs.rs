use super::Cause;

pub struct ProgramError {
    pub code: u64,
    pub name: &'static str,
    pub cause: Cause,
}

pub struct KnownProgram {
    pub id: &'static str,
    pub name: &'static str,
    pub errors: &'static [ProgramError],
}

impl KnownProgram {
    pub fn error(&self, code: u64) -> Option<&'static ProgramError> {
        self.errors.iter().find(|e| e.code == code)
    }
}

pub const FAILURE_LAB_ID: &str = "3RUGuRGUWq9guvxDz9suK6FxHHbuCm3YjtkT5zsSGtSY";

const TOKEN_ERRORS: &[ProgramError] = &[ProgramError {
    code: 1,
    name: "InsufficientFunds",
    cause: Cause::InsufficientTokenBalance,
}];

const fn no_table(id: &'static str, name: &'static str) -> KnownProgram {
    KnownProgram {
        id,
        name,
        errors: &[],
    }
}

// Sources: SPL Token interface/src/error.rs (InsufficientFunds = 1); Jupiter v6 errors (6001);
// programs/failure-lab/src/lib.rs (LabError starts at Anchor's 6000).
static KNOWN_PROGRAMS: &[KnownProgram] = &[
    KnownProgram {
        id: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
        name: "SPL Token",
        errors: TOKEN_ERRORS,
    },
    KnownProgram {
        id: "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
        name: "Token-2022",
        errors: TOKEN_ERRORS,
    },
    KnownProgram {
        id: "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4",
        name: "Jupiter Aggregator v6",
        errors: &[ProgramError {
            code: 6001,
            name: "SlippageToleranceExceeded",
            cause: Cause::SlippageExceeded,
        }],
    },
    KnownProgram {
        id: FAILURE_LAB_ID,
        name: "failure-lab",
        errors: &[ProgramError {
            code: 6000,
            name: "AmountOverLimit",
            cause: Cause::ProgramCustomError,
        }],
    },
    no_table("11111111111111111111111111111111", "System Program"),
    no_table(
        "ComputeBudget111111111111111111111111111111",
        "Compute Budget",
    ),
    no_table("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P", "pump.fun"),
    no_table(
        "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA",
        "pump.fun AMM",
    ),
    no_table("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD", "Kamino Lend"),
    no_table("dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH", "Drift v2"),
    no_table(
        "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc",
        "Orca Whirlpool",
    ),
    no_table(
        "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK",
        "Raydium CLMM",
    ),
    no_table(
        "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C",
        "Raydium CPMM",
    ),
    no_table(
        "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo",
        "Meteora DLMM",
    ),
];

pub fn known_program(id: &str) -> Option<&'static KnownProgram> {
    KNOWN_PROGRAMS.iter().find(|p| p.id == id)
}
