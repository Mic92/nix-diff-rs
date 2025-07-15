# nix-diff-rs

A Rust port of [nix-diff](https://github.com/Gabriel439/nix-diff), a tool to explain why two Nix derivations differ.

## Features

- Compare two Nix derivations and show their differences
- Support for multiple input types:
  - `.drv` files (pre-built derivations)
  - Realized store paths
  - `.nix` files (will be instantiated automatically)
  - Flake references (e.g., `nixpkgs#hello`, `path:/path/to/flake#package`)
- Colored output with support for `NO_COLOR` environment variable
- Configurable diff orientation (line, word, character)
- Minimal dependencies

## Installation

```bash
nix build
```

## Usage

```bash
nix-diff [OPTIONS] <INPUT1> <INPUT2>

Arguments:
  <INPUT1>    First input (.drv file, store path, .nix file, or flake#attr)
  <INPUT2>    Second input (.drv file, store path, .nix file, or flake#attr)

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

Compare Nix files (will be instantiated automatically):
```bash
nix-diff hello.nix goodbye.nix
```

Compare flake outputs:
```bash
# Compare packages from flakes
nix-diff nixpkgs#hello nixpkgs#hello-wayland

# Compare local flake outputs
nix-diff .#packages.x86_64-linux.myapp .#packages.x86_64-linux.myapp-dev

# Compare using flake paths
nix-diff path:/path/to/flake1#package path:/path/to/flake2#package
```

Disable colors:
```bash
nix-diff --color never input1 input2
# or
NO_COLOR=1 nix-diff input1 input2
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