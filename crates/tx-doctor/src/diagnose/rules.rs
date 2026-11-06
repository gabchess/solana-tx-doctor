use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::frame::failing_frame;
use super::{known_program, Cause, Diagnosis, TxMeta};

static RX_FAILED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Program (\w+) failed: (.+)$").unwrap());
static RX_CUSTOM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"custom program error: 0x([0-9a-fA-F]+)").unwrap());
static RX_ANCHOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"AnchorError.*?Error Code: (\w+)\. Error Number: (\d+)\.").unwrap()
});
static RX_COMPUTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"exceeded CUs meter|Computational budget exceeded").unwrap());
static RX_LAMPORTS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)insufficient lamports").unwrap());
static RX_ERROR_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)error").unwrap());
static RX_SLIPPAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)slippage|AmountOutBelowMinimum|MinimumAmountOut|MinAmountOut|TooLittle\w*Received|TooMuch\w*Required|BelowMinTokensOut").unwrap()
});

/// Builtin InstructionError variants (anza-xyz/solana-sdk instruction-error/src/lib.rs).
fn builtin_cause(name: &str) -> Option<Cause> {
    Some(match name {
        "InsufficientFunds" => Cause::InsufficientLamports,
        "UninitializedAccount" => Cause::AccountNotInitialized,
        "InvalidInstructionData" => Cause::InstructionRejected,
        "IllegalOwner" | "InvalidAccountOwner" | "IncorrectProgramId" | "InvalidAccountData" => {
            Cause::WrongAccountPassed
        }
        "MissingRequiredSignature" => Cause::MissingSignature,
        "ComputationalBudgetExceeded" => Cause::ComputeBudgetExceeded,
        _ => return None,
    })
}

/// Anchor framework codes (anchor lang/error/src/lib.rs), counted only with an AnchorError line.
fn anchor_cause(code: u64) -> Option<Cause> {
    match code {
        100..=103 | 3005 => Some(Cause::InstructionRejected),
        // RequireViolated = 2500 through RequireGteViolated = 2506: a require! check in the program's own logic, not an account constraint.
        2500..=2506 => Some(Cause::ProgramCustomError),
        2000..=2999 => Some(Cause::AnchorConstraintViolation),
        3001 | 3012 => Some(Cause::AccountNotInitialized),
        3002 | 3007 | 3008 | 3014 => Some(Cause::WrongAccountPassed),
        3010 => Some(Cause::MissingSignature),
        _ => None,
    }
}

fn instruction_error(err: &Value) -> (Option<u64>, Option<&Value>) {
    match err.get("InstructionError").and_then(Value::as_array) {
        Some(pair) if pair.len() == 2 => (pair[0].as_u64(), Some(&pair[1])),
        _ => (None, None),
    }
}

pub fn diagnose(meta: &TxMeta) -> Diagnosis {
    let logs: &[String] = meta.log_messages.as_deref().unwrap_or(&[]);
    let (index, inner) = instruction_error(&meta.err);
    // The innermost program fails first; callers then re-report the same code, so the first "failed" line is the origin.
    let failed_idx = logs.iter().position(|l| RX_FAILED.is_match(l));
    let failed_line = failed_idx.map(|i| logs[i].as_str());
    let program_id = failed_line
        .and_then(|l| RX_FAILED.captures(l))
        .map(|c| c[1].to_string());
    let program = program_id.as_deref().and_then(known_program);
    let frame: &[String] = match (failed_idx, program_id.as_deref()) {
        (Some(i), Some(id)) => failing_frame(logs, i, id),
        _ => &[],
    };
    let anchor_line = frame
        .iter()
        .find(|l| RX_ANCHOR.is_match(l))
        .map(String::as_str);
    let anchor = anchor_line.and_then(|l| RX_ANCHOR.captures(l));
    let anchor_name = anchor.as_ref().map(|c| c[1].to_string());
    let anchor_code = anchor.as_ref().and_then(|c| c[2].parse::<u64>().ok());
    let custom = inner.and_then(|v| v.get("Custom")).and_then(Value::as_u64);
    let hex_code = failed_line
        .and_then(|l| RX_CUSTOM.captures(l))
        .and_then(|c| u64::from_str_radix(&c[1], 16).ok());
    let code = custom.or(hex_code).or(anchor_code);
    let known = code.and_then(|c| program.and_then(|p| p.error(c)));
    let builtin = inner.and_then(Value::as_str);

    let err_text = serde_json::to_string(&meta.err).unwrap_or_default();
    let proof = anchor_line.or(failed_line).unwrap_or(&err_text).to_string();
    let result = |cause: Cause, evidence: &str| Diagnosis {
        cause,
        title: cause.title(),
        fix: cause.fix(),
        evidence: evidence.to_string(),
        instruction_index: index,
        program_id: program_id.clone(),
        program_name: program.map(|p| p.name),
        error_code: code,
        error_name: known
            .map(|k| k.name.to_string())
            .or_else(|| anchor_name.clone())
            .or_else(|| builtin.map(str::to_string)),
        fee_lamports: meta.fee,
        compute_units: meta.compute_units_consumed,
    };
    let in_frame = |rx: &Regex| frame.iter().find(|l| rx.is_match(l)).map(String::as_str);

    // Only the failing program's own lines count, so a message from an earlier, successful program cannot set the cause.
    let compute_line = in_frame(&RX_COMPUTE);
    if builtin == Some("ComputationalBudgetExceeded") || compute_line.is_some() {
        return result(
            Cause::ComputeBudgetExceeded,
            compute_line.unwrap_or(&err_text),
        );
    }

    if err_text.contains("InsufficientFundsForFee") || err_text.contains("InsufficientFundsForRent")
    {
        return result(Cause::InsufficientLamports, &err_text);
    }
    if let Some(line) = in_frame(&RX_LAMPORTS) {
        return result(Cause::InsufficientLamports, line);
    }

    if let Some(cause) = builtin.and_then(builtin_cause) {
        return result(cause, failed_line.unwrap_or(&err_text));
    }

    if let Some(k) = known {
        return result(k.cause, &proof);
    }

    let slippage_line = frame
        .iter()
        .find(|l| RX_ERROR_LINE.is_match(l) && RX_SLIPPAGE.is_match(l));
    if let Some(line) = slippage_line {
        return result(Cause::SlippageExceeded, line);
    }

    if let (Some(line), Some(cause)) = (anchor_line, anchor_code.and_then(anchor_cause)) {
        return result(cause, line);
    }
    if code.is_some() {
        return result(Cause::ProgramCustomError, &proof);
    }

    result(Cause::Unknown, failed_line.unwrap_or(&err_text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const P: &str = "Prog1111111111111111111111111111111111111111";

    fn meta(err: Value, logs: Vec<String>) -> TxMeta {
        TxMeta {
            err,
            fee: 5000,
            compute_units_consumed: None,
            log_messages: Some(logs),
        }
    }

    fn frame(inner: &[&str], failed: &str) -> Vec<String> {
        let mut v = vec![format!("Program {P} invoke [1]")];
        v.extend(inner.iter().map(|s| s.to_string()));
        v.push(format!("Program {P} failed: {failed}"));
        v
    }

    fn custom(index: u64, code: u64) -> Value {
        json!({ "InstructionError": [index, { "Custom": code }] })
    }

    fn cause_of(err: Value, logs: Vec<String>) -> Cause {
        diagnose(&meta(err, logs)).cause
    }

    #[test]
    fn a_non_anchor_program_returning_3012_is_its_own_error() {
        let logs = frame(&[], "custom program error: 0xbc4");
        assert_eq!(cause_of(custom(0, 3012), logs), Cause::ProgramCustomError);
    }

    #[test]
    fn a_non_anchor_program_returning_2006_is_not_an_anchor_constraint() {
        let logs = frame(&[], "custom program error: 0x7d6");
        assert_eq!(cause_of(custom(0, 2006), logs), Cause::ProgramCustomError);
    }

    #[test]
    fn logging_a_slippage_setting_then_failing_otherwise_is_not_slippage() {
        let logs = frame(
            &["Program log: slippage_bps=50"],
            "custom program error: 0x1770",
        );
        assert_eq!(cause_of(custom(0, 6000), logs), Cause::ProgramCustomError);
    }

    #[test]
    fn a_slippage_error_from_an_earlier_successful_program_does_not_count() {
        let mut logs = vec![
            "Program Other111111111111111111111111111111111111 invoke [1]".to_string(),
            "Program log: Error: slippage check skipped".to_string(),
            "Program Other111111111111111111111111111111111111 success".to_string(),
        ];
        logs.extend(frame(&[], "custom program error: 0x1770"));
        assert_eq!(cause_of(custom(1, 6000), logs), Cause::ProgramCustomError);
    }

    #[test]
    fn compute_or_lamport_messages_from_an_earlier_program_do_not_set_the_cause() {
        let mut logs = vec![
            "Program Other111111111111111111111111111111111111 invoke [1]".to_string(),
            "Program log: insufficient lamports, retrying with a smaller amount".to_string(),
            "Program log: exceeded CUs meter in a dry run".to_string(),
            "Program Other111111111111111111111111111111111111 success".to_string(),
        ];
        logs.extend(frame(&[], "custom program error: 0x1770"));
        assert_eq!(cause_of(custom(1, 6000), logs), Cause::ProgramCustomError);
    }

    #[test]
    fn builtin_insufficient_funds_means_not_enough_sol() {
        let d = diagnose(&meta(
            json!({ "InstructionError": [2, "InsufficientFunds"] }),
            frame(&[], "insufficient funds for instruction"),
        ));
        assert_eq!(d.cause, Cause::InsufficientLamports);
        assert_eq!(d.error_name.as_deref(), Some("InsufficientFunds"));
        assert_eq!(d.instruction_index, Some(2));
    }

    #[test]
    fn builtin_illegal_owner_means_the_wrong_account_was_passed() {
        let logs = frame(&[], "Provided owner is not allowed");
        let err = json!({ "InstructionError": [0, "IllegalOwner"] });
        assert_eq!(cause_of(err, logs), Cause::WrongAccountPassed);
    }

    #[test]
    fn a_non_anchor_slippage_error_from_the_failing_program_is_slippage() {
        let logs = frame(
            &["Program log: Error: exceeds desired slippage limit"],
            "custom program error: 0x1e",
        );
        assert_eq!(cause_of(custom(0, 30), logs), Cause::SlippageExceeded);
    }

    #[test]
    fn anchor_101_means_the_program_does_not_accept_the_instruction() {
        let logs = frame(
            &["Program log: AnchorError occurred. Error Code: InstructionFallbackNotFound. Error Number: 101. Error Message: x."],
            "custom program error: 0x65",
        );
        assert_eq!(cause_of(custom(0, 101), logs), Cause::InstructionRejected);
    }

    #[test]
    fn anchor_2500_is_a_failed_require_check_and_2001_is_a_failed_constraint() {
        let require = frame(
            &["Program log: AnchorError thrown in programs/x/src/lib.rs:21. Error Code: RequireViolated. Error Number: 2500. Error Message: A require expression was violated."],
            "custom program error: 0x9c4",
        );
        let has_one = frame(
            &["Program log: AnchorError caused by account: vault. Error Code: ConstraintHasOne. Error Number: 2001. Error Message: A has one constraint was violated."],
            "custom program error: 0x7d1",
        );
        assert_eq!(
            cause_of(custom(0, 2500), require),
            Cause::ProgramCustomError
        );
        assert_eq!(
            cause_of(custom(0, 2001), has_one),
            Cause::AnchorConstraintViolation
        );
    }

    #[test]
    fn a_transaction_level_fee_shortfall_is_not_enough_sol() {
        assert_eq!(
            cause_of(json!("InsufficientFundsForFee"), vec![]),
            Cause::InsufficientLamports
        );
    }

    #[test]
    fn a_known_program_code_takes_its_name_from_the_table() {
        let id = crate::diagnose::FAILURE_LAB_ID;
        let logs = vec![
            format!("Program {id} invoke [1]"),
            format!("Program {id} failed: custom program error: 0x1770"),
        ];
        let d = diagnose(&meta(custom(0, 6000), logs));
        assert_eq!(d.cause, Cause::ProgramCustomError);
        assert_eq!(d.error_name.as_deref(), Some("AmountOverLimit"));
        assert_eq!(d.program_name, Some("failure-lab"));
    }
}
