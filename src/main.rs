use anyhow::{Context, Result, anyhow};
use nix_diff::{diff, instantiate, parser, render, types};
use std::env;
use std::path::{Path, PathBuf};
use types::{ColorMode, Derivation, RenderOptions};

fn main() {
    // Follow diff(1) exit code convention: 0 = identical, 1 = differ, 2 = error.
    std::process::exit(match run() {
        Ok(false) => 0,
        Ok(true) => 1,
        Err(e) => {
            eprintln!("Error: {e:#}");
            2
        }
    });
}

fn run() -> Result<bool> {
    let args: Vec<String> = env::args().collect();

    let mut opts = RenderOptions::default();
    let mut paths = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--color" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("--color requires an argument"));
                }
                opts.color_mode = match args[i].as_str() {
                    "always" => ColorMode::Always,
                    "auto" => ColorMode::Auto,
                    "never" => ColorMode::Never,
                    _ => return Err(anyhow!("Invalid color mode: {}", args[i])),
                };
            }
            "--no-inline-highlight" => {
                opts.inline_highlight = false;
            }
            "--context" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("--context requires an argument"));
                }
                opts.context_lines = args[i]
                    .parse()
                    .with_context(|| format!("Invalid context lines: {}", args[i]))?;
            }
            "--depth" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("--depth requires an argument"));
                }
                opts.max_depth = Some(
                    args[i]
                        .parse()
                        .with_context(|| format!("Invalid depth: {}", args[i]))?,
                );
            }
            "-v" | "--verbose" => {
                opts.verbose = true;
            }
            "--input-list-limit" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("--input-list-limit requires an argument"));
                }
                opts.input_list_limit = args[i]
                    .parse()
                    .with_context(|| format!("Invalid input-list-limit: {}", args[i]))?;
            }
            "-h" | "--help" => {
                print_help();
                return Ok(false);
            }
            arg => {
                if arg.starts_with('-') {
                    return Err(anyhow!("Unknown option: {arg}"));
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
        std::process::exit(2);
    }
    if paths[0].as_os_str().is_empty() || paths[1].as_os_str().is_empty() {
        eprintln!("Error: Derivation paths cannot be empty");
        std::process::exit(2);
    }

    let (drv1, path1) = load_derivation(&paths[0])?;
    let (drv2, path2) = load_derivation(&paths[1])?;

    let mut diff_context = diff::DiffContext::new();
    let diff = diff_context.diff_derivations(&path1, &path2, &drv1, &drv2)?;

    let renderer = render::Renderer::new(opts);
    let differs = renderer.render(&diff, &path1, &path2)?;

    Ok(differs)
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
    eprintln!("  --no-inline-highlight  Disable word-level highlighting within changed lines");
    eprintln!("  --context <LINES>      Number of context lines (default: 3)");
    eprintln!("  --input-list-limit <N> Max added/removed inputs to list (default: 10)");
    eprintln!("  --depth <N>            Max recursion depth into input derivations");
    eprintln!("  -v, --verbose          Show output-path changes and full input lists");
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
