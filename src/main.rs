mod diff;
mod instantiate;
mod parser;
mod render;
mod types;

use anyhow::{Context, Result, anyhow};
use std::env;
use std::path::{Path, PathBuf};
use types::{ColorMode, Derivation, DiffOrientation};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let mut color_mode = ColorMode::Auto;
    let mut orientation = DiffOrientation::Line;
    let mut context_lines = 3;
    let mut paths = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--color" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("--color requires an argument"));
                }
                color_mode = match args[i].as_str() {
                    "always" => ColorMode::Always,
                    "auto" => ColorMode::Auto,
                    "never" => ColorMode::Never,
                    _ => return Err(anyhow!("Invalid color mode: {}", args[i])),
                };
            }
            "--orientation" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("--orientation requires an argument"));
                }
                orientation = match args[i].as_str() {
                    "line" => DiffOrientation::Line,
                    "word" => DiffOrientation::Word,
                    "character" => DiffOrientation::Character,
                    _ => return Err(anyhow!("Invalid orientation: {}", args[i])),
                };
            }
            "--context" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("--context requires an argument"));
                }
                context_lines = args[i]
                    .parse()
                    .with_context(|| format!("Invalid context lines: {}", args[i]))?;
            }
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            arg => {
                if arg.starts_with('-') {
                    return Err(anyhow!("Unknown option: {}", arg));
                }
                paths.push(PathBuf::from(arg));
            }
        }
        i += 1;
    }

    if paths.len() != 2 {
        eprintln!("Error: Expected exactly 2 derivation paths");
        eprintln!();
        print_help();
        std::process::exit(1);
    }

    let (drv1, path1) = load_derivation(&paths[0])?;
    let (drv2, path2) = load_derivation(&paths[1])?;

    let mut diff_context = diff::DiffContext::new(orientation, context_lines);
    let diff = diff_context.diff_derivations(&path1, &path2, &drv1, &drv2)?;

    let renderer = render::Renderer::new(color_mode, context_lines);
    renderer.render(&diff)?;

    Ok(())
}

fn print_help() {
    eprintln!("nix-diff - Explain why two Nix derivations differ");
    eprintln!();
    eprintln!("Usage: nix-diff [OPTIONS] <INPUT1> <INPUT2>");
    eprintln!();
    eprintln!("Arguments:");
    eprintln!("  <INPUT1>    First input (.drv file, store path, .nix file, or flake#attr)");
    eprintln!("  <INPUT2>    Second input (.drv file, store path, .nix file, or flake#attr)");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --color <MODE>         Color mode: always, auto, never (default: auto)");
    eprintln!("  --orientation <MODE>   Diff orientation: line, word, character (default: line)");
    eprintln!("  --context <LINES>      Number of context lines (default: 3)");
    eprintln!("  -h, --help             Show this help message");
}

fn load_derivation(input: &Path) -> Result<(Derivation, Vec<u8>)> {
    let input_str = input.to_string_lossy();

    if input_str.ends_with(".drv") {
        // Direct .drv file
        let drv = parser::parse_derivation(&input_str)
            .with_context(|| format!("Failed to parse derivation: {}", input.display()))?;
        Ok((drv, input_str.as_bytes().to_vec()))
    } else if input_str.contains('#') || input_str.ends_with(".nix") {
        // Flake reference or .nix file
        let drv = instantiate::instantiate_and_parse(&input_str)
            .with_context(|| format!("Failed to instantiate: {input_str}"))?;
        let path = format!("<instantiated from {input_str}>");
        Ok((drv, path.into_bytes()))
    } else {
        // Try as store path
        let path = parser::get_derivation_path(&input.to_string_lossy())?;
        let drv = parser::parse_derivation(&path)
            .with_context(|| format!("Failed to parse derivation: {path}"))?;
        Ok((drv, path.into_bytes()))
    }
}
