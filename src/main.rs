mod diff;
mod parser;
mod render;
mod types;

use anyhow::{anyhow, Context, Result};
use std::env;
use std::path::PathBuf;
use types::{ColorMode, DiffOrientation};

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

    let path1 = parser::get_derivation_path(&paths[0])?;
    let path2 = parser::get_derivation_path(&paths[1])?;

    let drv1 = parser::parse_derivation(&path1)
        .with_context(|| format!("Failed to parse first derivation: {}", path1.display()))?;
    let drv2 = parser::parse_derivation(&path2)
        .with_context(|| format!("Failed to parse second derivation: {}", path2.display()))?;

    let mut diff_context = diff::DiffContext::new(orientation, context_lines);
    let diff = diff_context.diff_derivations(&path1, &path2, &drv1, &drv2)?;

    let renderer = render::Renderer::new(color_mode, context_lines);
    renderer.render(&diff)?;

    Ok(())
}

fn print_help() {
    eprintln!("nix-diff - Explain why two Nix derivations differ");
    eprintln!();
    eprintln!("Usage: nix-diff [OPTIONS] <PATH1> <PATH2>");
    eprintln!();
    eprintln!("Arguments:");
    eprintln!("  <PATH1>    First derivation path (.drv file or store path)");
    eprintln!("  <PATH2>    Second derivation path (.drv file or store path)");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --color <MODE>         Color mode: always, auto, never (default: auto)");
    eprintln!("  --orientation <MODE>   Diff orientation: line, word, character (default: line)");
    eprintln!("  --context <LINES>      Number of context lines (default: 3)");
    eprintln!("  -h, --help             Show this help message");
}
