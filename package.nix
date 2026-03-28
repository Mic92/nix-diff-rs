{
  lib,
  rustPlatform,
  clippy,
  nix,
  enableClippy ? false,
  enableChecks ? false,
}:
rustPlatform.buildRustPackage {
  pname = "nix-diff";
  version = "0.1.0";
  src = ./.;
  cargoLock = {
    lockFile = ./Cargo.lock;
    outputHashes = {
      "harmonia-store-aterm-0.0.0-alpha.0" = "sha256-FDL2xxAFOYw21VhGYake2fFC9S7jK5kBSM4OfU12VmQ=";
    };
  };

  NIX_CFLAGS_COMPILE = "-Wno-error";

  nativeBuildInputs = lib.optional enableClippy clippy ++ lib.optional enableChecks nix;

  nativeCheckInputs = [ nix ];

  doCheck = false;

  buildPhase = lib.optionalString (enableClippy || enableChecks) ''
    runHook preBuild
    ${lib.optionalString enableClippy "cargo clippy --all-targets --all-features -- -D warnings"}
    ${lib.optionalString enableChecks "cargo test"}
    runHook postBuild
  '';

  installPhase = lib.optionalString (enableClippy || enableChecks) ''
    runHook preInstall
    touch $out
    runHook postInstall
  '';

  meta = with lib; {
    description = "Explain why two Nix derivations differ";
    homepage = "https://github.com/nix-community/nix-diff-rs";
    license = licenses.bsd3;
    maintainers = with maintainers; [ ];
    mainProgram = "nix-diff";
  };
}
