use anyhow::{Context, Result, anyhow, bail};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

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

    let expression = build_flake_expression(&store_path, &nar_hash, attr);
    instantiate_expression(&expression, gcroot_path)
}

/// Build a Nix expression that evaluates a flake attribute, mirroring the
/// packages/legacyPackages resolution that `nix build` applies.
///
/// For a single-component attr like `samba`, this tries (in order):
///   1. top-level output  (`f.samba`)
///   2. `f.packages.<system>.samba`
///   3. `f.legacyPackages.<system>.samba`
///
/// For a dotted attr like `packages.x86_64-linux.samba`, the path is used
/// verbatim so callers can still address any output directly.
fn build_flake_expression(store_path: &str, nar_hash: &str, attr: &str) -> String {
    let flake_expr = format!("builtins.getFlake \"path:{store_path}?narHash={nar_hash}\"");

    if attr.contains('.') {
        // Fully-qualified path — use verbatim, as before.
        format!("({flake_expr}).{attr}")
    } else {
        // Single-component: try top-level, then packages.<s>, then legacyPackages.<s>.
        // The has-attr operator (`?`) with short-circuit `&&` ensures we never throw on
        // a missing intermediate attribute; only the final `else throw` fires.
        format!(
            "let f = {flake_expr}; s = builtins.currentSystem; in \
             if f ? {attr} then f.{attr} \
             else if f ? packages && f.packages ? ${{s}} && f.packages.${{s}} ? {attr} \
             then f.packages.${{s}}.{attr} \
             else if f ? legacyPackages && f.legacyPackages ? ${{s}} && f.legacyPackages.${{s}} ? {attr} \
             then f.legacyPackages.${{s}}.{attr} \
             else throw \"attribute '{attr}' not found in flake\""
        )
    }
}

#[derive(serde::Deserialize)]
struct FlakeMetadata {
    path: Option<String>,
    locked: Option<FlakeLocked>,
}

#[derive(serde::Deserialize)]
struct FlakeLocked {
    #[serde(rename = "narHash")]
    nar_hash: Option<String>,
}

/// Safely extract `path` and `locked.narHash` from flake metadata JSON.
fn extract_flake_fields(json: &str) -> Result<(String, String)> {
    let metadata: FlakeMetadata =
        serde_json::from_str(json).context("Failed to parse flake metadata JSON")?;

    let store_path = metadata
        .path
        .ok_or_else(|| anyhow!("No path found in flake metadata"))?;

    let nar_hash = metadata
        .locked
        .and_then(|l| l.nar_hash)
        .ok_or_else(|| anyhow!("No locked.narHash found in flake metadata"))?;

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

    // nix-instantiate may print multiple paths (one per line) when the
    // expression yields multiple derivations. Take the first and warn
    // rather than failing cryptically in read_link().
    let stdout = String::from_utf8(output.stdout)?;
    let mut lines = stdout.lines().filter(|l| !l.is_empty());
    let gcroot_result = lines
        .next()
        .ok_or_else(|| anyhow!("nix-instantiate produced no output"))?
        .to_string();
    if lines.next().is_some() {
        eprintln!(
            "warning: nix-instantiate produced multiple derivations, using the first: {gcroot_result}"
        );
    }

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
        // Older nix or unusual refs may omit "locked"; we must return
        // a proper error instead of panicking.
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

    #[test]
    fn build_flake_expression_single_attr_has_resolution_logic() {
        let expr = build_flake_expression("/nix/store/x", "sha256-abc", "samba");
        // Must contain all three fallback branches.
        assert!(expr.contains("f ? samba"), "missing top-level check: {expr}");
        assert!(expr.contains("f ? packages"), "missing packages branch: {expr}");
        assert!(
            expr.contains("f ? legacyPackages"),
            "missing legacyPackages branch: {expr}"
        );
        assert!(expr.contains("builtins.currentSystem"), "missing system: {expr}");
    }

    #[test]
    fn build_flake_expression_dotted_attr_is_verbatim() {
        let expr = build_flake_expression("/nix/store/x", "sha256-abc", "packages.x86_64-linux.samba");
        // Fully-qualified path must be embedded verbatim; no resolution wrapper.
        assert!(
            expr.contains("packages.x86_64-linux.samba"),
            "dotted attr not verbatim: {expr}"
        );
        assert!(
            !expr.contains("legacyPackages"),
            "unexpected resolution logic for dotted attr: {expr}"
        );
    }
}
