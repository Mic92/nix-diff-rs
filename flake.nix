{
  description = "Explain why two Nix derivations differ";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    treefmt-nix.url = "github:numtide/treefmt-nix";
    treefmt-nix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"];

      imports = [
        inputs.treefmt-nix.flakeModule
      ];

      perSystem = {
        config,
        self',
        inputs',
        pkgs,
        system,
        ...
      }: {
        packages = {
          default = self'.packages.nix-diff;
          nix-diff = pkgs.callPackage ./package.nix {};
        };

        checks = let
          packages = pkgs.lib.mapAttrs' (n: pkgs.lib.nameValuePair "package-${n}") self'.packages;
          devShells = pkgs.lib.mapAttrs' (n: pkgs.lib.nameValuePair "devShell-${n}") self'.devShells;
        in
          packages
          // devShells
          // {
            clippy = pkgs.callPackage ./package.nix {enableClippy = true;};
            tests = pkgs.callPackage ./package.nix {enableChecks = true;};
          };

        devShells.default = pkgs.mkShell {
          inputsFrom = [self'.packages.default];
          NIX_CFLAGS_COMPILE = "-Wno-error";
          packages = with pkgs; [
            cargo
            rustc
            rust-analyzer
            rustfmt
            clippy
            cargo-watch
            cargo-criterion
            cargo-insta
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
