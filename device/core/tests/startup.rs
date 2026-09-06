use std::{os::unix::fs::PermissionsExt, process::Command};

#[test]
fn startup_reports_safe_stage_diagnostics_without_configuration_values() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.json");
    let marker = "secret-marker-must-not-appear";
    std::fs::write(&config, format!(r#"{{"private":"{marker}"}}"#)).unwrap();
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o664)).unwrap();
    let unsafe_mode = Command::new(env!("CARGO_BIN_EXE_dwdesktop-core"))
        .arg("--config")
        .arg(&config)
        .output()
        .unwrap();
    assert!(!unsafe_mode.status.success());
    let diagnostic = String::from_utf8(unsafe_mode.stderr).unwrap();
    assert!(diagnostic.contains("no group/world write"));
    assert!(!diagnostic.contains(marker));

    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(
        &config,
        format!(r#"{{"private":"{marker}","private":null}}"#),
    )
    .unwrap();
    let duplicate = Command::new(env!("CARGO_BIN_EXE_dwdesktop-core"))
        .arg("--config")
        .arg(&config)
        .output()
        .unwrap();
    assert!(!duplicate.status.success());
    let diagnostic = String::from_utf8(duplicate.stderr).unwrap();
    assert!(diagnostic.contains("duplicate keys are forbidden"));
    assert!(!diagnostic.contains(marker));
}
