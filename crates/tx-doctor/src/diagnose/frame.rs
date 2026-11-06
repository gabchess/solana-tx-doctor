/// Log lines of the failing program's own frame: from its last invoke up to its failure.
pub fn failing_frame<'a>(logs: &'a [String], failed_idx: usize, program_id: &str) -> &'a [String] {
    let invoke = format!("Program {program_id} invoke [");
    let mut start = failed_idx;
    while start > 0 && !logs[start - 1].starts_with(&invoke) {
        start -= 1;
    }
    &logs[start.saturating_sub(1)..=failed_idx]
}

#[cfg(test)]
mod tests {
    use super::failing_frame;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn frame_starts_at_the_failing_programs_own_invoke() {
        let logs = lines(&[
            "Program A invoke [1]",
            "Program log: from A",
            "Program A success",
            "Program B invoke [1]",
            "Program log: from B",
            "Program B failed: boom",
        ]);
        assert_eq!(failing_frame(&logs, 5, "B"), &logs[3..6]);
    }

    #[test]
    fn without_an_invoke_line_the_frame_runs_from_the_top() {
        let logs = lines(&["Program log: x", "Program B failed: boom"]);
        assert_eq!(failing_frame(&logs, 1, "B"), &logs[..]);
    }
}
