use std::{fs, process::Command};

#[test]
fn check_accepts_valid_backup_and_rejects_bad_size_and_settings() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("pedal.fcp");
    let mut bytes = vec![0; 23646];
    bytes[0] = 3;
    fs::write(&path, &bytes)?;
    let check = || {
        Command::new(env!("CARGO_BIN_EXE_mwave-rust"))
            .arg("--check")
            .arg(&path)
            .output()
    };
    let valid = check()?;
    assert!(
        valid.status.success(),
        "{}",
        String::from_utf8_lossy(&valid.stderr)
    );
    assert!(String::from_utf8(valid.stdout)?.contains("Advanced custom"));
    bytes[2] = 255;
    fs::write(&path, &bytes)?;
    let invalid = check()?;
    assert!(!invalid.status.success());
    assert!(String::from_utf8(invalid.stderr)?.contains("Global MIDI channel"));
    fs::write(&path, [0; 20])?;
    assert!(!check()?.status.success());
    Ok(())
}
