{
  description = "hlsd - serve live HLS (and optional MPEG-DASH) from a raw PCM audio stream";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    # Current crane doesn't expose a `nixpkgs` input, so we don't follow it.
    crane.url = "github:ipetkov/crane";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };

    flake-utils.url = "github:numtide/flake-utils";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, crane, fenix, flake-utils, advisory-db, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };

        inherit (pkgs) lib;

        craneLib = crane.mkLib pkgs;

        src = craneLib.cleanCargoSource ./.;

        # hlsd is pure-Rust and has no runtime system dependencies: it reads
        # PCM on stdin and serves HLS/DASH over HTTP. There is no mpv/audio
        # device involvement, so no runtime wrapper is needed.
        commonArgs = {
          inherit src;

          pname = "hlsd";
          version = "0.1.0";
          strictDeps = true;

          # We build with all-codecs enabled. The optional codec crates
          # (fdk-aac, mp3lame-encoder, audiopus) compile vendored C sources
          # with the `cc` crate, so a C compiler must be on PATH — `stdenv.cc`
          # is the toolchain for this platform (clang on Darwin, gcc on Linux).
          # audiopus_sys builds its bundled libopus with autotools, so
          # autoconf/automake/libtool are required too. pkg-config lets the
          # -sys crates probe for system libraries (they fall back to their
          # vendored copies otherwise).
          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.stdenv.cc
            pkgs.autoconf
            pkgs.automake
            pkgs.libtool
          ];

          buildInputs = lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];

          # Build just the one bin target, with every codec enabled.
          cargoExtraArgs = "--locked --features all-codecs --bin hlsd";
        };

        craneLibLLvmTools = craneLib.overrideToolchain
          (fenix.packages.${system}.complete.withComponents [
            "cargo"
            "llvm-tools"
            "rustc"
          ]);

        # Cache the dependency graph separately from the crate source.
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        hlsd = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          doCheck = false;

          meta = {
            description = "Serve live HLS (and optional MPEG-DASH) from a raw PCM audio stream";
            homepage = "https://github.com/tsirysndr/hlsd";
            license = lib.licenses.mit;
            mainProgram = "hlsd";
            platforms = lib.platforms.unix;
          };
        });

      in
      {
        checks = {
          inherit hlsd;

          hlsd-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

          hlsd-doc = craneLib.cargoDoc (commonArgs // {
            inherit cargoArtifacts;
          });

          hlsd-fmt = craneLib.cargoFmt {
            inherit src;
          };

          hlsd-audit = craneLib.cargoAudit {
            inherit src advisory-db;
          };

          hlsd-nextest = craneLib.cargoNextest (commonArgs // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
          });
        } // lib.optionalAttrs (system == "x86_64-linux") {
          hlsd-coverage = craneLib.cargoTarpaulin (commonArgs // {
            inherit cargoArtifacts;
          });
        };

        packages = {
          default = hlsd;
          hlsd = hlsd;

          hlsd-llvm-coverage = craneLibLLvmTools.cargoLlvmCov (commonArgs // {
            inherit cargoArtifacts;
          });
        };

        apps.default = flake-utils.lib.mkApp {
          drv = hlsd;
          name = "hlsd";
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = builtins.attrValues self.checks.${system};

          # Build-time tools. pkg-config + stdenv.cc + autotools are needed to
          # compile the vendored C codec sources (see commonArgs above).
          nativeBuildInputs = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            rust-analyzer
            pkg-config
            stdenv.cc
            autoconf
            automake
            libtool
          ];

          buildInputs = with pkgs; lib.optionals stdenv.isDarwin [
            libiconv
          ];

          shellHook = ''
            echo "⚡ hlsd dev shell — cargo $(cargo --version | cut -d' ' -f2) ready"
          '';
        };
      });
}
