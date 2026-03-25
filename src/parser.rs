use crate::types::{Derivation, Output};
use anyhow::{anyhow, Context, Result};
use harmonia_store_aterm::parse_derivation_aterm;
use harmonia_store_core::derivation::{DerivationInputs, DerivationOutput};
use harmonia_store_core::store_path::{StoreDir, StorePath, StorePathName};
use harmonia_utils_hash::fmt::CommonHash;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

pub fn parse_derivation(path: &str) -> Result<Derivation> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read derivation file: {path}"))?;

    let store_dir = StoreDir::default();
    let name = extract_drv_name(path, &store_dir);

    let drv = parse_derivation_aterm(&store_dir, &content, name)
        .map_err(|e| anyhow!("Failed to parse ATerm: {e}"))?;

    Ok(convert_derivation(&store_dir, drv))
}

pub fn parse_derivation_string(input: &str) -> Result<Derivation> {
    let store_dir = StoreDir::default();
    let name: StorePathName = "unknown".parse().unwrap();

    let drv = parse_derivation_aterm(&store_dir, input, name)
        .map_err(|e| anyhow!("Failed to parse ATerm: {e}"))?;

    Ok(convert_derivation(&store_dir, drv))
}

/// Extract the derivation name from a store path like `/nix/store/hash-name.drv`.
/// Nix strips the `.drv` suffix to get the derivation name.
fn extract_drv_name(path: &str, store_dir: &StoreDir) -> StorePathName {
    let fallback: StorePathName = "unknown".parse().unwrap();

    let base_name = match store_dir.strip_prefix(path) {
        Ok(b) => b,
        Err(_) => return fallback,
    };

    let sp: StorePath = match base_name.parse() {
        Ok(p) => p,
        Err(_) => return fallback,
    };

    let name_str = sp.name().to_string();
    let drv_name = name_str.strip_suffix(".drv").unwrap_or(&name_str);

    drv_name.parse().unwrap_or(fallback)
}

fn convert_derivation(
    store_dir: &StoreDir,
    drv: harmonia_store_core::derivation::Derivation,
) -> Derivation {
    let outputs = convert_outputs(store_dir, &drv);
    let inputs = DerivationInputs::from(&drv.inputs);

    let input_derivations: BTreeMap<Vec<u8>, BTreeSet<Vec<u8>>> = inputs
        .drvs
        .iter()
        .map(|(sp, oi)| {
            let path = store_dir.display(sp).to_string().into_bytes();
            let outs = oi
                .outputs
                .iter()
                .map(|o| o.to_string().into_bytes())
                .collect();
            (path, outs)
        })
        .collect();

    let input_sources: BTreeSet<Vec<u8>> = inputs
        .srcs
        .iter()
        .map(|sp| store_dir.display(sp).to_string().into_bytes())
        .collect();

    let platform = drv.platform.to_vec();
    let builder = drv.builder.to_vec();
    let args = drv.args.iter().map(|a| a.to_vec()).collect();
    let env = drv
        .env
        .iter()
        .map(|(k, v)| (k.to_vec(), v.to_vec()))
        .collect();

    Derivation {
        outputs,
        input_sources,
        input_derivations,
        platform,
        builder,
        args,
        env,
    }
}

fn convert_outputs(
    store_dir: &StoreDir,
    drv: &harmonia_store_core::derivation::Derivation,
) -> BTreeMap<Vec<u8>, Output> {
    drv.outputs
        .iter()
        .map(|(name, output)| {
            let name_bytes = name.to_string().into_bytes();
            let out = match output {
                DerivationOutput::InputAddressed(sp) => Output {
                    path: store_dir.display(sp).to_string().into_bytes(),
                    hash_algorithm: None,
                    hash: None,
                },
                DerivationOutput::CAFixed(ca) => {
                    let path = output
                        .path(store_dir, &drv.name, name)
                        .ok()
                        .flatten()
                        .map(|sp| store_dir.display(&sp).to_string().into_bytes())
                        .unwrap_or_default();
                    Output {
                        path,
                        hash_algorithm: Some(ca.method_algorithm().to_string().into_bytes()),
                        hash: Some(ca.hash().as_base16().as_bare().to_string().into_bytes()),
                    }
                }
                DerivationOutput::CAFloating(cama) => Output {
                    path: Vec::new(),
                    hash_algorithm: Some(cama.to_string().into_bytes()),
                    hash: None,
                },
                DerivationOutput::Impure(cama) => Output {
                    path: Vec::new(),
                    hash_algorithm: Some(cama.to_string().into_bytes()),
                    hash: Some(b"impure".to_vec()),
                },
                DerivationOutput::Deferred => Output {
                    path: Vec::new(),
                    hash_algorithm: None,
                    hash: None,
                },
            };
            (name_bytes, out)
        })
        .collect()
}

pub fn get_derivation_path(store_path: &str) -> Result<String> {
    // If it's already a .drv file, return it
    if store_path.ends_with(".drv") {
        return Ok(store_path.to_string());
    }

    // Otherwise, query the derivation
    let output = std::process::Command::new("nix-store")
        .arg("--query")
        .arg("--deriver")
        .arg(store_path)
        .output()
        .with_context(|| {
            format!("Failed to run nix-store --query --deriver for path: {store_path}")
        })?;

    if !output.status.success() {
        return Err(anyhow!(
            "Failed to query derivation for {}: {}",
            store_path,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let drv_path = String::from_utf8(output.stdout)?.trim().to_string();

    Ok(drv_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_derivation() {
        let drv = r#"Derive([("out","/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-test","","")],[],[],"/bin/bash","/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-builder",["-c","echo hello"],[("name","test"),("out","/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-test")])"#;
        let result = parse_derivation_string(drv).unwrap();
        assert_eq!(result.outputs.len(), 1);
        assert_eq!(result.platform, b"/bin/bash");
        assert_eq!(result.args, vec![b"-c".to_vec(), b"echo hello".to_vec()]);
    }
}
