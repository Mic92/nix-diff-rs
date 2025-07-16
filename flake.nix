{
  description = "Explain why two Nix derivations differ";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    treefmt-nix.url = "github:numtide/treefmt-nix";
    treefmt-nix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];

      imports = [
        inputs.treefmt-nix.flakeModule
      ];

      perSystem = { config, self', inputs', pkgs, system, ... }: {
        packages = {
          default = self'.packages.nix-diff;
          nix-diff = pkgs.rustPlatform.buildRustPackage {
            pname = "nix-diff";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            meta = with pkgs.lib; {
              description = "Explain why two Nix derivations differ";
              homepage = "https://github.com/nix-community/nix-diff-rs";
              license = licenses.bsd3;
              maintainers = with maintainers; [ ];
              mainProgram = "nix-diff";
            };
          };
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ self'.packages.default ];
          NIX_CFLAGS_COMPILE = "-Wno-error";
          packages = with pkgs; [
            cargo
            rustc
            rust-analyzer
            rustfmt
            clippy
            cargo-watch
            cargo-criterion
          ];
        };

        treefmt = {
          projectRootFile = "flake.nix";
          programs = {
            rustfmt.enable = true;
            alejandra.enable = true;
          };
        };
      };
    };
}