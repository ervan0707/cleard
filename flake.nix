{
  description = "cleard — a multi-ecosystem disk reclaimer (npkill, but for everything)";

  # Pull prebuilt outputs from the project's Cachix cache instead of compiling
  # Rust from source. Nix only honours these automatically for trusted users;
  # everyone else can opt in with `cachix use skinnyvans` (see the README).
  nixConfig = {
    extra-substituters = [ "https://skinnyvans.cachix.org" ];
    extra-trusted-public-keys = [
      "skinnyvans.cachix.org-1:sgaZPgRhzsU4YScjc2U5Imc+4E3y9Ov/G/q8p/csX+o="
    ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        # Toolchain pinned via rust-toolchain.toml so `cargo`/`rustc`/`rust-analyzer`
        # are identical inside `nix develop` and in CI.
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        # Build the package with the *same* pinned toolchain as the dev shell,
        # so the package never drifts onto nixpkgs' default rustc (and its MSRV).
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        cleard = rustPlatform.buildRustPackage {
          pname = "cleard";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          # The compiled std bakes its source paths (panic/backtrace location
          # strings) into the binary. rust-overlay keeps that source tree inside
          # the toolchain's store path, so those strings make Nix treat the whole
          # toolchain (rustc + LLVM + cctools, ~2 GB) as a runtime dependency.
          # The strings are diagnostic only — nothing reads them at runtime — so
          # scrub the reference out of the final binary to keep the closure tiny.
          nativeBuildInputs = [ pkgs.removeReferencesTo ];
          postInstall = ''
            remove-references-to -t ${rustToolchain} "$out/bin/cleard"
          '';
          # Pure scanner: no system libs needed beyond libc.
          meta = with pkgs.lib; {
            description = "Interactive, multi-ecosystem build-artifact disk reclaimer";
            license = licenses.mit;
            mainProgram = "cleard";
          };
        };
      in
      {
        packages.default = cleard;
        packages.cleard = cleard;

        # `nix run github:ervan0707/cleard`
        apps.default = flake-utils.lib.mkApp { drv = cleard; };

        devShells.default = pkgs.mkShell {
          packages = [
            rustToolchain
            pkgs.rust-analyzer
            pkgs.cargo-audit # `cargo audit` for dependency CVE scanning
          ];
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          shellHook = ''
            echo "cleard dev shell — run: cargo run -- <path>  |  audit: cargo audit"
          '';
        };
      });
}
