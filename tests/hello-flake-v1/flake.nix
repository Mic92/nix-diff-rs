{
  description = "Hello flake v1";

  outputs = {self}: {
    packages.x86_64-linux.default = import ./default.nix;
    packages.aarch64-linux.default = import ./default.nix;
    packages.x86_64-darwin.default = import ./default.nix;
    packages.aarch64-darwin.default = import ./default.nix;
  };
}
