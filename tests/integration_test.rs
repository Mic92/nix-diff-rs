use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_basic_derivation_diff() {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");

    // Generate derivations
    let output1 = Command::new("nix-instantiate")
        .arg(tests_dir.join("hello-v1.nix"))
        .output()
        .expect("Failed to instantiate hello-v1.nix");

    let output2 = Command::new("nix-instantiate")
        .arg(tests_dir.join("hello-v2.nix"))
        .output()
        .expect("Failed to instantiate hello-v2.nix");

    let drv1 = String::from_utf8_lossy(&output1.stdout).trim().to_string();
    let drv2 = String::from_utf8_lossy(&output2.stdout).trim().to_string();

    // Run nix-diff
    let output = Command::new(env!("CARGO_BIN_EXE_nix-diff"))
        .args(&[&drv1, &drv2])
        .env("NO_COLOR", "1")
        .output()
        .expect("Failed to run nix-diff");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that we see expected differences
    assert!(stdout.contains("hello-v1"));
    assert!(stdout.contains("hello-v2"));
    assert!(stdout.contains("version"));
    assert!(stdout.contains("1.0"));
    assert!(stdout.contains("2.0"));
    assert!(stdout.contains("newFeature"));
}

#[test]
fn test_color_modes() {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");

    let output1 = Command::new("nix-instantiate")
        .arg(tests_dir.join("hello-v1.nix"))
        .output()
        .expect("Failed to instantiate hello-v1.nix");

    let output2 = Command::new("nix-instantiate")
        .arg(tests_dir.join("hello-v2.nix"))
        .output()
        .expect("Failed to instantiate hello-v2.nix");

    let drv1 = String::from_utf8_lossy(&output1.stdout).trim().to_string();
    let drv2 = String::from_utf8_lossy(&output2.stdout).trim().to_string();

    // Test with color never
    let output = Command::new(env!("CARGO_BIN_EXE_nix-diff"))
        .args(&["--color", "never", &drv1, &drv2])
        .output()
        .expect("Failed to run nix-diff");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("\x1b["));

    // Test with color always
    let output = Command::new(env!("CARGO_BIN_EXE_nix-diff"))
        .args(&["--color", "always", &drv1, &drv2])
        .output()
        .expect("Failed to run nix-diff");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\x1b["));
}

#[test]
fn test_no_color_env_var() {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");

    let output1 = Command::new("nix-instantiate")
        .arg(tests_dir.join("hello-v1.nix"))
        .output()
        .expect("Failed to instantiate hello-v1.nix");

    let output2 = Command::new("nix-instantiate")
        .arg(tests_dir.join("hello-v2.nix"))
        .output()
        .expect("Failed to instantiate hello-v2.nix");

    let drv1 = String::from_utf8_lossy(&output1.stdout).trim().to_string();
    let drv2 = String::from_utf8_lossy(&output2.stdout).trim().to_string();

    // Test NO_COLOR environment variable
    let output = Command::new(env!("CARGO_BIN_EXE_nix-diff"))
        .args(&["--color", "always", &drv1, &drv2])
        .env("NO_COLOR", "1")
        .output()
        .expect("Failed to run nix-diff");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("\x1b["));
}
