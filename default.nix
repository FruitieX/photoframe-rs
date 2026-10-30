let
  unstable = import (fetchTarball https://nixos.org/channels/nixos-unstable/nixexprs.tar.xz) { };
  rust_overlay = import (builtins.fetchTarball https://github.com/oxalica/rust-overlay/archive/master.tar.gz );
  nixpkgs = import <nixpkgs> { overlays = [ rust_overlay ]; };
  rustToolchain = (nixpkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
    extensions = [
      "rust-src"
      "rust-analysis"
      "rustfmt-preview"
      "clippy-preview"
    ];
    targets = [
      "x86_64-unknown-linux-gnu"
      "x86_64-unknown-linux-musl"
    ];
  };
in
  with nixpkgs;
  stdenv.mkDerivation {
    name = "env";
    # Build C dependencies (aws-lc-sys) against musl headers, like musl-tools in CI.
    CC_x86_64_unknown_linux_musl = "${musl.dev}/bin/musl-gcc";
    buildInputs = [
      rustToolchain
      nodejs_26
    ];
  }