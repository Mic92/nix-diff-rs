use insta::assert_snapshot;
use std::path::PathBuf;
use std::process::Command;

fn run_nix_diff(file1: &str, file2: &str) -> String {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");

    // Generate derivations
    let output1 = Command::new("nix-instantiate")
        .arg(tests_dir.join(file1))
        .output()
        .unwrap_or_else(|_| panic!("Failed to instantiate {file1}"));

    let output2 = Command::new("nix-instantiate")
        .arg(tests_dir.join(file2))
        .output()
        .unwrap_or_else(|_| panic!("Failed to instantiate {file2}"));

    let drv1 = String::from_utf8_lossy(&output1.stdout).trim().to_string();
    let drv2 = String::from_utf8_lossy(&output2.stdout).trim().to_string();

    // Run nix-diff with NO_COLOR to get consistent output
    let output = Command::new(env!("CARGO_BIN_EXE_nix-diff"))
        .args([&drv1, &drv2])
        .env("NO_COLOR", "1")
        .output()
        .expect("Failed to run nix-diff");

    assert!(
        output.status.success(),
        "nix-diff failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn test_hello_diff_snapshot() {
    let output = run_nix_diff("hello-flake-v1/default.nix", "hello-flake-v2/default.nix");
    assert_snapshot!(output);
}

#[test]
fn test_identical_derivations() {
    let output = run_nix_diff("hello-flake-v1/default.nix", "hello-flake-v1/default.nix");
    assert_snapshot!(output);
}

#[test]
fn test_hello_diff_with_context() {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");

    // Generate derivations
    let output1 = Command::new("nix-instantiate")
        .arg(tests_dir.join("hello-flake-v1/default.nix"))
        .output()
        .expect("Failed to instantiate hello-flake-v1/default.nix");

    let output2 = Command::new("nix-instantiate")
        .arg(tests_dir.join("hello-flake-v2/default.nix"))
        .output()
        .expect("Failed to instantiate hello-flake-v2/default.nix");

    let drv1 = String::from_utf8_lossy(&output1.stdout).trim().to_string();
    let drv2 = String::from_utf8_lossy(&output2.stdout).trim().to_string();

    // Run with different context settings
    let output = Command::new(env!("CARGO_BIN_EXE_nix-diff"))
        .args(["--context", "5", &drv1, &drv2])
        .env("NO_COLOR", "1")
        .output()
        .expect("Failed to run nix-diff");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert_snapshot!(stdout);
}

#[test]
fn test_word_diff_orientation() {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");

    // Generate derivations
    let output1 = Command::new("nix-instantiate")
        .arg(tests_dir.join("hello-flake-v1/default.nix"))
        .output()
        .expect("Failed to instantiate hello-flake-v1/default.nix");

    let output2 = Command::new("nix-instantiate")
        .arg(tests_dir.join("hello-flake-v2/default.nix"))
        .output()
        .expect("Failed to instantiate hello-flake-v2/default.nix");

    let drv1 = String::from_utf8_lossy(&output1.stdout).trim().to_string();
    let drv2 = String::from_utf8_lossy(&output2.stdout).trim().to_string();

    // Run with word orientation
    let output = Command::new(env!("CARGO_BIN_EXE_nix-diff"))
        .args(["--orientation", "word", &drv1, &drv2])
        .env("NO_COLOR", "1")
        .output()
        .expect("Failed to run nix-diff");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert_snapshot!(stdout);
}
