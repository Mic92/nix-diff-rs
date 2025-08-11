use std::path::PathBuf;
use std::process::Command;

mod common;
use common::setup_nix_env;

fn assert_diff_output(output: &str) {
    assert!(output.contains("hello-v1"));
    assert!(output.contains("hello-v2"));
    assert!(output.contains("version"));
    assert!(output.contains("1.0"));
    assert!(output.contains("2.0"));
    assert!(output.contains("newFeature"));
}

#[test]
fn test_basic_derivation_diff() {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let (_nix_root, env_vars) = setup_nix_env();

    // Generate derivations
    let mut cmd1 = Command::new("nix-instantiate");
    cmd1.arg(tests_dir.join("hello-flake-v1/default.nix"));
    for (key, value) in &env_vars {
        cmd1.env(key, value);
    }
    let output1 = cmd1
        .output()
        .expect("Failed to instantiate hello-flake-v1/default.nix");

    let mut cmd2 = Command::new("nix-instantiate");
    cmd2.arg(tests_dir.join("hello-flake-v2/default.nix"));
    for (key, value) in &env_vars {
        cmd2.env(key, value);
    }
    let output2 = cmd2
        .output()
        .expect("Failed to instantiate hello-flake-v2/default.nix");

    let drv1 = String::from_utf8_lossy(&output1.stdout).trim().to_string();
    let drv2 = String::from_utf8_lossy(&output2.stdout).trim().to_string();

    // Run nix-diff
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_nix-diff"));
    cmd.args([&drv1, &drv2]).env("NO_COLOR", "1");
    for (key, value) in &env_vars {
        cmd.env(key, value);
    }
    let output = cmd.output().expect("Failed to run nix-diff");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_diff_output(&stdout);
}

#[test]
fn test_nix_file_diff() {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let (_nix_root, env_vars) = setup_nix_env();

    // Run nix-diff directly on .nix files
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_nix-diff"));
    cmd.args([
        tests_dir
            .join("hello-flake-v1/default.nix")
            .to_str()
            .unwrap(),
        tests_dir
            .join("hello-flake-v2/default.nix")
            .to_str()
            .unwrap(),
    ])
    .env("NO_COLOR", "1");
    for (key, value) in &env_vars {
        cmd.env(key, value);
    }
    let output = cmd.output().expect("Failed to run nix-diff");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_diff_output(&stdout);
}

#[test]
fn test_flake_diff() {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let (_nix_root, env_vars) = setup_nix_env();

    // Get current system
    let mut cmd = Command::new("nix");
    cmd.args([
        "--extra-experimental-features",
        "nix-command flakes",
        "eval",
        "--impure",
        "--expr",
        "builtins.currentSystem",
    ]);
    for (key, value) in &env_vars {
        cmd.env(key, value);
    }
    let system_output = cmd.output().expect("Failed to get current system");
    assert!(
        !system_output.stdout.is_empty(),
        "Failed to get current system"
    );

    let system = String::from_utf8_lossy(&system_output.stdout)
        .trim()
        .trim_matches('"')
        .to_string();

    // Run nix-diff on flake references with fully qualified attributes
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_nix-diff"));
    cmd.args([
        &format!(
            "path:{}#packages.{}.default",
            tests_dir.join("hello-flake-v1").to_str().unwrap(),
            system
        ),
        &format!(
            "path:{}#packages.{}.default",
            tests_dir.join("hello-flake-v2").to_str().unwrap(),
            system
        ),
    ])
    .env("NO_COLOR", "1");
    for (key, value) in &env_vars {
        cmd.env(key, value);
    }
    let output = cmd.output().expect("Failed to run nix-diff");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        eprintln!("nix-diff failed with stderr: {stderr}");
        eprintln!("stdout: {stdout}");
        panic!("nix-diff failed");
    }

    assert_diff_output(&stdout);
}
