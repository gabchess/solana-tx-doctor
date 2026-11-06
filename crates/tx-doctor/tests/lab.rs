//! The failure-lab program, run in LiteSVM by lab/record.test.mjs, fails each
//! instruction on purpose and records the logs. Each recording must decode to the
//! cause and the named error the program defines.

use std::path::PathBuf;

use serde_json::Value;
use tx_doctor::cli::report_from_file;
use tx_doctor::diagnose::{Cause, FAILURE_LAB_ID};

fn recording(name: &str) -> Value {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/lab/{name}.json"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{}: {e}. Run `npm test --prefix lab` first.",
            path.display()
        )
    });
    serde_json::from_str(&text).unwrap()
}

fn assert_decodes(name: &str, cause: Cause, code: u64, error_name: &str) {
    let value = recording(name);
    assert_eq!(value["expected_cause"], cause.as_str());
    assert_eq!(value["expected_error_code"], code);
    assert_eq!(value["expected_error_name"], error_name);

    let d = report_from_file(&value).unwrap().diagnosis;
    assert_eq!(d.cause, cause, "{d:#?}");
    assert_eq!(d.error_code, Some(code), "{d:#?}");
    assert_eq!(d.error_name.as_deref(), Some(error_name), "{d:#?}");
    assert_eq!(d.program_id.as_deref(), Some(FAILURE_LAB_ID));
    assert_eq!(d.program_name, Some("failure-lab"));
    assert_eq!(d.instruction_index, Some(0));
    assert!(
        d.evidence.contains(error_name),
        "proof line names the error: {}",
        d.evidence
    );
}

#[test]
fn require_with_a_custom_error_decodes_to_the_named_error() {
    assert_decodes(
        "withdraw_over_limit",
        Cause::ProgramCustomError,
        6000,
        "AmountOverLimit",
    );
}

#[test]
fn a_has_one_violation_decodes_to_an_anchor_constraint_failure() {
    assert_decodes(
        "set_limit_wrong_authority",
        Cause::AnchorConstraintViolation,
        2001,
        "ConstraintHasOne",
    );
}

#[test]
fn an_account_that_was_never_created_decodes_to_account_not_initialized() {
    assert_decodes(
        "read_ledger_not_initialized",
        Cause::AccountNotInitialized,
        3012,
        "AccountNotInitialized",
    );
}
