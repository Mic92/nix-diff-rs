use anyhow::{Context, Result, anyhow, bail};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use tinyjson::JsonValue;

use crate::parser::parse_derivation;
use crate::types::Derivation;

/// Instantiate a .nix file, flake, or expression and parse the resulting .drv file
pub fn instantiate_and_parse(input: &str) -> Result<Derivation> {
    let temp_dir = TempDir::new().context("Failed to create temporary directory")?;
    let gcroot_path = temp_dir.path().join("result");

    let drv_path = if input.contains('#') {
        // Treat as flake reference if it contains #
        instantiate_flake(input, &gcroot_path)?
    } else if input.ends_with(".nix") {
        // Treat as regular Nix file
        instantiate_file(input, &gcroot_path)?
    } else {
        // Try as store path first
        return Err(anyhow!(
            "Input must be a .drv file, .nix file, or flake reference"
        ));
    };

    // Parse the resulting .drv file
    parse_derivation(&drv_path)
}

/// Instantiate a flake reference
fn instantiate_flake(flake_ref: &str, gcroot_path: &Path) -> Result<String> {
    // Extract attribute from flake reference
    let (flake_path, attr) = flake_ref
        .split_once('#')
        .ok_or_else(|| anyhow!("Invalid flake reference: missing #"))?;

    // First get flake metadata to resolve to store path and narHash
    let metadata_output = Command::new("nix")
        .args([
            "--extra-experimental-features",
            "nix-command flakes",
            "flake",
            "metadata",
            "--json",
            flake_path,
        ])
        .output()
        .context("Failed to run nix flake metadata")?;

    if !metadata_output.status.success() {
        bail!(
            "nix flake metadata failed: {}",
            String::from_utf8_lossy(&metadata_output.stderr)
        );
    }

    let metadata_str = String::from_utf8(metadata_output.stdout)
        .context("Failed to parse metadata output as UTF-8")?;

    let (store_path, nar_hash) = extract_flake_fields(&metadata_str)?;

    // Create expression to evaluate the flake with narHash for pure evaluation
    let expression = format!("(builtins.getFlake \"path:{store_path}?narHash={nar_hash}\").{attr}");

    instantiate_expression(&expression, gcroot_path)
}

/// Safely extract `path` and `locked.narHash` from flake metadata JSON.
/// tinyjson's `[]` indexing panics on missing keys, so walk the object
/// manually and return a proper error.
fn extract_flake_fields(json: &str) -> Result<(String, String)> {
    use std::collections::HashMap;

    let metadata: JsonValue = json
        .parse()
        .context("Failed to parse flake metadata JSON")?;

    let root: &HashMap<String, JsonValue> = metadata
        .get()
        .ok_or_else(|| anyhow!("flake metadata is not a JSON object"))?;

    let store_path = root
        .get("path")
        .and_then(|v| v.get::<String>())
        .ok_or_else(|| anyhow!("No path found in flake metadata"))?
        .clone();

    let nar_hash = root
        .get("locked")
        .and_then(|v| v.get::<HashMap<String, JsonValue>>())
        .and_then(|l| l.get("narHash"))
        .and_then(|v| v.get::<String>())
        .ok_or_else(|| anyhow!("No locked.narHash found in flake metadata"))?
        .clone();

    Ok((store_path, nar_hash))
}

/// Instantiate a Nix expression
fn instantiate_expression(expr: &str, gcroot_path: &Path) -> Result<String> {
    let mut cmd = Command::new("nix-instantiate");
    cmd.args(["--expr", expr]);
    run_nix_instantiate(cmd, gcroot_path)
}

/// Instantiate a Nix file
fn instantiate_file(file_path: &str, gcroot_path: &Path) -> Result<String> {
    let mut cmd = Command::new("nix-instantiate");
    cmd.arg(file_path);
    run_nix_instantiate(cmd, gcroot_path)
}

/// Common function to instantiate and process nix-instantiate output
fn run_nix_instantiate(mut cmd: Command, gcroot_path: &Path) -> Result<String> {
    cmd.args(["--extra-experimental-features", "nix-command flakes"]);
    cmd.args(["--add-root", &gcroot_path.to_string_lossy(), "--indirect"]);
    let output = cmd.output().context("Failed to run nix-instantiate")?;

    if !output.status.success() {
        bail!(
            "nix-instantiate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let gcroot_result = String::from_utf8(output.stdout)?.trim().to_string();

    // Read the symlink to get the actual .drv path
    // --add-root --indirect always creates a symlink pointing to the store
    let drv_path = fs::read_link(&gcroot_result)
        .with_context(|| format!("Failed to read gcroot symlink: {gcroot_result}"))?
        .to_string_lossy()
        .to_string();

    if !Path::new(&drv_path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("drv"))
    {
        bail!("nix-instantiate did not return a .drv file: {drv_path}");
    }

    Ok(drv_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flake_metadata_missing_locked_returns_error() {
        // Older nix or unusual refs may omit "locked". Indexing with []
        // panics in tinyjson; we must return a proper error instead.
        let json = r#"{"path":"/nix/store/x"}"#;
        let err = extract_flake_fields(json).unwrap_err();
        assert!(
            err.to_string().contains("narHash"),
            "expected narHash error, got: {err}"
        );
    }

    #[test]
    fn flake_metadata_missing_path_returns_error() {
        let json = r#"{"locked":{"narHash":"sha256-x"}}"#;
        let err = extract_flake_fields(json).unwrap_err();
        assert!(err.to_string().contains("path"));
    }

    #[test]
    fn flake_metadata_happy_path() {
        let json = r#"{"path":"/nix/store/x","locked":{"narHash":"sha256-abc"}}"#;
        let (p, h) = extract_flake_fields(json).unwrap();
        assert_eq!(p, "/nix/store/x");
        assert_eq!(h, "sha256-abc");
    }
}
