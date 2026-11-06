//! Every recorded mainnet transaction decodes to the cause it was filed under. Offline.

use std::path::PathBuf;

use serde_json::Value;
use tx_doctor::cli::report_from_file;
use tx_doctor::diagnose::Cause;

fn fixture_paths() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tx");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("fixtures folder exists")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn all_fixtures_decode_to_their_expected_cause() {
    let paths = fixture_paths();
    assert_eq!(paths.len(), 13, "expected the 13 recorded fixtures");
    let mut wrong = Vec::new();
    for path in &paths {
        let value: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let expected = value["expected_cause"]
            .as_str()
            .expect("fixture names its cause");
        let expected = Cause::parse(expected).unwrap_or_else(|| panic!("unknown cause {expected}"));
        let report = report_from_file(&value).unwrap();
        if report.diagnosis.cause != expected {
            wrong.push(format!(
                "{}: expected {}, got {} ({})",
                path.file_name().unwrap().to_string_lossy(),
                expected.as_str(),
                report.diagnosis.cause.as_str(),
                report.diagnosis.evidence
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_proof_line_comes_from_the_logs_when_the_logs_have_one() {
    for path in fixture_paths() {
        let value: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let logs: Vec<&str> = value["response"]["meta"]["logMessages"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let evidence = report_from_file(&value).unwrap().diagnosis.evidence;
        if logs.iter().any(|l| l.contains(" failed: ")) {
            assert!(
                logs.contains(&evidence.as_str()),
                "{}: {evidence}",
                path.display()
            );
        }
    }
}
