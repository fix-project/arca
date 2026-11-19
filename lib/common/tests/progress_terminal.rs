use std::process::Command;

#[test]
fn progress_logs_named_events_and_measured_transfer_rates() {
    let output = Command::new(env!("PROBE")).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"result\n");
    let stderr = String::from_utf8(output.stderr).unwrap();
    let lines: Vec<_> = stderr.lines().collect();
    assert_eq!(
        &lines[..4],
        [
            "START Evaluate named batch",
            "START local: Load transferred input (from large)",
            "unnamed diagnostic",
            "END local: Load transferred input (from large) (0.50s, 1.0 MiB, 2.0 MiB/s)",
        ],
    );
    assert!(lines[4].starts_with("END Evaluate named batch ("));
    let rows: Vec<_> = lines[7..]
        .iter()
        .map(|row| row.split_whitespace().collect::<Vec<_>>())
        .collect();
    assert_eq!(rows.len(), 1, "{stderr}");
    assert_eq!(&rows[0][..2], ["large", "3"]);
}
