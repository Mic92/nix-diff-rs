# nix-diff-rs

A Rust port of [nix-diff](https://github.com/Gabriel439/nix-diff), a tool to explain why two Nix derivations differ.

## Features

- Compare two Nix derivations and show their differences
- Support for both `.drv` files and realized store paths
- Colored output with support for `NO_COLOR` environment variable
- Configurable diff orientation (line, word, character)
- Minimal dependencies

## Installation

```bash
nix build
```

## Usage

```bash
nix-diff [OPTIONS] <PATH1> <PATH2>

Arguments:
  <PATH1>    First derivation path (.drv file or store path)
  <PATH2>    Second derivation path (.drv file or store path)

Options:
  --color <MODE>         Color mode: always, auto, never (default: auto)
  --orientation <MODE>   Diff orientation: line, word, character (default: line)
  --context <LINES>      Number of context lines (default: 3)
  -h, --help             Show this help message
```

### Examples

Compare two derivations:
```bash
nix-diff /nix/store/abc123-hello.drv /nix/store/def456-hello.drv
```

Compare realized store paths:
```bash
nix-diff /nix/store/abc123-hello /nix/store/def456-hello
```

Disable colors:
```bash
nix-diff --color never path1 path2
# or
NO_COLOR=1 nix-diff path1 path2
```

## Development

```bash
# Enter development shell
nix develop

# Build
cargo build

# Run tests
cargo test

# Run benchmarks
cargo bench

# Format code
nix fmt
```

## License

BSD-3-Clause (same as the original nix-diff)