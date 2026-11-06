use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    SlippageExceeded,
    InsufficientTokenBalance,
    InsufficientLamports,
    ComputeBudgetExceeded,
    AccountNotInitialized,
    WrongAccountPassed,
    MissingSignature,
    InstructionRejected,
    AnchorConstraintViolation,
    ProgramCustomError,
    Unknown,
}

impl Cause {
    pub const ALL: [Cause; 11] = [
        Cause::SlippageExceeded,
        Cause::InsufficientTokenBalance,
        Cause::InsufficientLamports,
        Cause::ComputeBudgetExceeded,
        Cause::AccountNotInitialized,
        Cause::WrongAccountPassed,
        Cause::MissingSignature,
        Cause::InstructionRejected,
        Cause::AnchorConstraintViolation,
        Cause::ProgramCustomError,
        Cause::Unknown,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Cause::SlippageExceeded => "slippage_exceeded",
            Cause::InsufficientTokenBalance => "insufficient_token_balance",
            Cause::InsufficientLamports => "insufficient_lamports",
            Cause::ComputeBudgetExceeded => "compute_budget_exceeded",
            Cause::AccountNotInitialized => "account_not_initialized",
            Cause::WrongAccountPassed => "wrong_account_passed",
            Cause::MissingSignature => "missing_signature",
            Cause::InstructionRejected => "instruction_rejected",
            Cause::AnchorConstraintViolation => "anchor_constraint_violation",
            Cause::ProgramCustomError => "program_custom_error",
            Cause::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Option<Cause> {
        Cause::ALL.into_iter().find(|c| c.as_str() == s)
    }

    pub fn title(self) -> &'static str {
        match self {
            Cause::SlippageExceeded => "Slippage tolerance exceeded",
            Cause::InsufficientTokenBalance => "Not enough tokens in the source account",
            Cause::InsufficientLamports => "Not enough SOL",
            Cause::ComputeBudgetExceeded => "Ran out of compute units",
            Cause::AccountNotInitialized => "An account was not initialized",
            Cause::WrongAccountPassed => "Wrong account passed",
            Cause::MissingSignature => "A required signer did not sign",
            Cause::InstructionRejected => "The program did not accept this instruction",
            Cause::AnchorConstraintViolation => "An Anchor account constraint failed",
            Cause::ProgramCustomError => "The program returned its own error",
            Cause::Unknown => "Cause not recognized",
        }
    }

    pub fn fix(self) -> &'static str {
        match self {
            Cause::SlippageExceeded => "The price moved past your limit before the swap landed. Retry with a fresh quote, raise slippage a little, or add a priority fee so the transaction lands sooner.",
            Cause::InsufficientTokenBalance => "The token account had fewer tokens than the transfer or swap amount. Re-read the balance right before signing, and leave room for fees and rounding.",
            Cause::InsufficientLamports => "An account did not have enough SOL for the transfer, the fee or the rent of a new account. Top up the wallet, or lower the amount and keep about 0.002 SOL for rent.",
            Cause::ComputeBudgetExceeded => "The transaction used its whole compute budget. Simulate it, then set a ComputeBudget limit about 10 to 20 percent above the units it used, or split the work into smaller transactions.",
            Cause::AccountNotInitialized => "The program expected an account that does not exist yet or holds no data. Create it first (for example the associated token account), or check that the client passes the right address.",
            Cause::WrongAccountPassed => "An account had the wrong owner or type for this instruction. Check the account order and addresses against the program's IDL, and that token accounts belong to the right mint and token program.",
            Cause::MissingSignature => "The program needed an account to sign, and it did not. Add that keypair or wallet as a signer, or pass the account the program expects as the authority.",
            Cause::InstructionRejected => "The instruction data did not match what the deployed program expects. This usually means a stale SDK or IDL after a program upgrade, or the wrong program id. Update the client and rebuild the instruction.",
            Cause::AnchorConstraintViolation => "An account did not match the program's #[account(...)] rules, such as seeds, owner, has_one or mut. Compare the accounts the client passes with the program's IDL.",
            Cause::ProgramCustomError => "Look up the error code in the program's IDL or source. The proof line names the program and the code.",
            Cause::Unknown => "No known pattern matched. Read the program logs from the top; the first 'failed' line names the program that stopped the transaction.",
        }
    }
}
