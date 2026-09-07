use std::process::Command;

#[test]
fn phase_zero_binary_fails_closed() {
    let output = Command::new(env!("CARGO_BIN_EXE_nli-server"))
        .output()
        .expect("run the Phase 0 bootstrap binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("bootstrap stderr is UTF-8");
    assert!(stderr.contains("Phase 0 bootstrap and exposes no API"));
}
