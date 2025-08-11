use insta::assert_snapshot;
use std::path::PathBuf;
use std::process::Command;

mod common;
use common::setup_nix_env;

// Normalize store paths and hashes for consistent snapshots
fn normalize_nix_output(output: &str, store_dir: &str) -> String {
    // Replace custom store path with /nix/store
    let normalized = output.replace(store_dir, "/nix/store");

    // Replace all hashes with "HASH"
    let re = regex::Regex::new(r"/nix/store/[a-z0-9]{32}-").unwrap();
    re.replace_all(&normalized, "/nix/store/HASH-").to_string()
}

fn run_nix_diff(file1: &str, file2: &str) -> String {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let (nix_root, env_vars) = setup_nix_env();
    let nix_store_dir = nix_root.path().join("store").to_string_lossy().to_string();

    // Generate derivations
    let mut cmd1 = Command::new("nix-instantiate");
    cmd1.args(["--extra-experimental-features", "nix-command flakes"])
        .arg(tests_dir.join(file1));
    for (key, value) in &env_vars {
        cmd1.env(key, value);
    }
    let output1 = cmd1
        .output()
        .unwrap_or_else(|_| panic!("Failed to instantiate {file1}"));

    let mut cmd2 = Command::new("nix-instantiate");
    cmd2.args(["--extra-experimental-features", "nix-command flakes"])
        .arg(tests_dir.join(file2));
    for (key, value) in &env_vars {
        cmd2.env(key, value);
    }
    let output2 = cmd2
        .output()
        .unwrap_or_else(|_| panic!("Failed to instantiate {file2}"));

    let drv1 = String::from_utf8_lossy(&output1.stdout).trim().to_string();
    let drv2 = String::from_utf8_lossy(&output2.stdout).trim().to_string();

    // Run nix-diff with NO_COLOR to get consistent output
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_nix-diff"));
    cmd.args([&drv1, &drv2]).env("NO_COLOR", "1");
    for (key, value) in &env_vars {
        cmd.env(key, value);
    }
    let output = cmd.output().expect("Failed to run nix-diff");

    assert!(
        output.status.success(),
        "nix-diff failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    // Normalize the temporary store path and hashes for consistent snapshots
    normalize_nix_output(&stdout, &nix_store_dir)
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
    let (nix_root, env_vars) = setup_nix_env();
    let nix_store_dir = nix_root.path().join("store").to_string_lossy().to_string();

    // Generate derivations
    let mut cmd1 = Command::new("nix-instantiate");
    cmd1.args(["--extra-experimental-features", "nix-command flakes"])
        .arg(tests_dir.join("hello-flake-v1/default.nix"));
    for (key, value) in &env_vars {
        cmd1.env(key, value);
    }
    let output1 = cmd1
        .output()
        .expect("Failed to instantiate hello-flake-v1/default.nix");

    let mut cmd2 = Command::new("nix-instantiate");
    cmd2.args(["--extra-experimental-features", "nix-command flakes"])
        .arg(tests_dir.join("hello-flake-v2/default.nix"));
    for (key, value) in &env_vars {
        cmd2.env(key, value);
    }
    let output2 = cmd2
        .output()
        .expect("Failed to instantiate hello-flake-v2/default.nix");

    let drv1 = String::from_utf8_lossy(&output1.stdout).trim().to_string();
    let drv2 = String::from_utf8_lossy(&output2.stdout).trim().to_string();

    // Run with different context settings
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_nix-diff"));
    cmd.args(["--context", "5", &drv1, &drv2])
        .env("NO_COLOR", "1");
    for (key, value) in &env_vars {
        cmd.env(key, value);
    }
    let output = cmd.output().expect("Failed to run nix-diff");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    // Normalize the temporary store path and hashes for consistent snapshots
    let normalized = normalize_nix_output(&stdout, &nix_store_dir);
    assert_snapshot!(normalized);
}

#[test]
fn test_word_diff_orientation() {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let (nix_root, env_vars) = setup_nix_env();
    let nix_store_dir = nix_root.path().join("store").to_string_lossy().to_string();

    // Generate derivations
    let mut cmd1 = Command::new("nix-instantiate");
    cmd1.args(["--extra-experimental-features", "nix-command flakes"])
        .arg(tests_dir.join("hello-flake-v1/default.nix"));
    for (key, value) in &env_vars {
        cmd1.env(key, value);
    }
    let output1 = cmd1
        .output()
        .expect("Failed to instantiate hello-flake-v1/default.nix");

    let mut cmd2 = Command::new("nix-instantiate");
    cmd2.args(["--extra-experimental-features", "nix-command flakes"])
        .arg(tests_dir.join("hello-flake-v2/default.nix"));
    for (key, value) in &env_vars {
        cmd2.env(key, value);
    }
    let output2 = cmd2
        .output()
        .expect("Failed to instantiate hello-flake-v2/default.nix");

    let drv1 = String::from_utf8_lossy(&output1.stdout).trim().to_string();
    let drv2 = String::from_utf8_lossy(&output2.stdout).trim().to_string();

    // Run with word orientation
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_nix-diff"));
    cmd.args(["--orientation", "word", &drv1, &drv2])
        .env("NO_COLOR", "1");
    for (key, value) in &env_vars {
        cmd.env(key, value);
    }
    let output = cmd.output().expect("Failed to run nix-diff");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    // Normalize the temporary store path and hashes for consistent snapshots
    let normalized = normalize_nix_output(&stdout, &nix_store_dir);
    assert_snapshot!(normalized);
}
