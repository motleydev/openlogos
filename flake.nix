{
  description = "OpenLogos: one Rust core, native readers and an embedded retrieval stack for open Bible study";

  inputs = {
    # Fetched from the official NixOS channel CDN rather than GitHub: no API rate
    # limits on fresh machines or CI, and flake.lock still pins an exact revision.
    # `just nix-update` moves it to the newest 26.05 release.
    nixpkgs.url = "https://channels.nixos.org/nixos-26.05/nixexprs.tar.xz";
    # Prebuilt Rust toolchains with cross-compilation targets (iOS, Android, wasm)
    # that nixpkgs' own rustc does not ship.
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      ...
    }:
    let
      inherit (nixpkgs) lib;

      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      forEachSystem =
        f:
        lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              overlays = [ rust-overlay.overlays.default ];
            }
          )
        );

      # One stable toolchain for the whole workspace. Cargo.toml's `rust-version` and
      # scripts/doctor.sh follow this value; bump all three together.
      rustVersion = "1.99.1";

      # Cross targets the core is built for on every host. Apple targets need Xcode's
      # linker and SDKs, so only Darwin shells carry them (see shells.apple).
      portableTargets = [
        "wasm32-unknown-unknown"
        "aarch64-linux-android"
      ];
      appleTargets = [
        "aarch64-apple-darwin"
        "aarch64-apple-ios"
        "aarch64-apple-ios-sim"
      ];

      toolchainFor =
        pkgs:
        pkgs.rust-bin.stable.${rustVersion}.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
          ];
          targets = portableTargets ++ lib.optionals pkgs.stdenv.isDarwin appleTargets;
        };

      # Only what cargo needs; a documentation edit must not rebuild the Rust checks.
      rustSource = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          ./Cargo.toml
          ./Cargo.lock
          ./rustfmt.toml
          ./.cargo # build-script environment for the C sources (sqlite-vec NEON)
          ./crates
        ];
      };

      # Validator dependencies, as VALIDATION.md records them.
      pythonFor = pkgs: pkgs.python3.withPackages (ps: [ ps.jsonschema ]);
    in
    {
      devShells = forEachSystem (
        pkgs:
        let
          toolchain = toolchainFor pkgs;
          python = pythonFor pkgs;

          # Tools every shell shares so `just lint` and the git hooks work in any of them.
          repoTools = [
            pkgs.just # task runner: `just` lists every workflow
            pkgs.lefthook # git hooks (`just setup` installs them)
            pkgs.git
            pkgs.jq
            pkgs.nixfmt # formats this file
            pkgs.shellcheck
            pkgs.actionlint # lints .github/workflows
          ];
          rustTools = [
            toolchain
            pkgs.cargo-nextest # `just test`: parallel runner with JUnit output for evidence
          ];

          # Rust links through `cc`. On Linux that compiler comes from nixpkgs so a fresh
          # host needs nothing else. On macOS the shells deliberately carry no compiler:
          # Xcode's clang and linker must be the ones that see Apple SDKs, and a Nix
          # clang in front of them breaks iOS and simulator builds. Xcode is the one
          # declared external prerequisite (specs/06-nix-operations.md); the shells
          # verify it rather than claim hermeticity. mkShellNoCC is not enough on its
          # own: rust-overlay's rustc propagates a Nix clang wrapper and cctools, so
          # `darwinHook` below takes them off PATH again (B063).
          mkShell = if pkgs.stdenv.isDarwin then pkgs.mkShellNoCC else pkgs.mkShell;

          shell =
            {
              name,
              packages,
              env ? { },
              hook ? "",
            }:
            mkShell (
              {
                name = "openlogos-${name}";
                packages = repoTools ++ packages;
                # Marker that scripts/doctor.sh uses to tell which dev shell is active.
                OPENLOGOS_DEVSHELL = name;
                # C sources compiled for wasm32 (SQLite inside crates/retrieval-sqlite, B064)
                # need a clang without the Nix wrapper: the wrapper adds hardening flags the
                # wasm32 target rejects (-fzero-call-used-regs) and Xcode's clang has no
                # wasm32 back end. The `cc` crate reads this variable for that target only.
                CC_wasm32_unknown_unknown = "${pkgs.llvmPackages.clang-unwrapped}/bin/clang";
                shellHook = ''
                  ${lib.optionalString pkgs.stdenv.isDarwin darwinHook}
                  ${hook}
                  if [ -z "''${CI:-}" ]; then
                    echo "openlogos ${name} shell ready. Run 'just' to see every workflow, 'just doctor' to check your machine."
                  fi
                '';
              }
              // env
            );

          # Every Darwin shell: no Nix compiler, linker or SDK in front of Xcode's.
          # - rust-overlay's rustc propagates a Nix clang wrapper and cctools (`cc`,
          #   `clang`, `ld`, `lipo`, ...) so cargo can link on a bare host. They link Apple
          #   targets against Nix's macOS SDK, which the iOS simulator rejects, so they are
          #   removed from PATH; /usr/bin's Xcode shims take over. scripts/apple-bindings.sh
          #   still names Xcode's clang per target explicitly, so it works either way.
          # - A Nix shell may point DEVELOPER_DIR/SDKROOT at Nix's SDK; Xcode needs its own.
          #   Same guard as sudoku-color's build scripts.
          darwinHook = ''
            kept=""
            IFS=: read -r -a entries <<<"$PATH"
            for entry in "''${entries[@]}"; do
              case "$entry" in
                /nix/store/*-clang-wrapper-*/bin | /nix/store/*-clang-[0-9]*/bin | /nix/store/*-cctools-*/bin) continue ;;
              esac
              kept="''${kept:+$kept:}$entry"
            done
            PATH="$kept"
            export PATH
            unset kept entries entry
            # The wrappers' setup hooks also export tool names and flags. xcodebuild reads
            # environment variables as build settings (LD=ld makes it link with ld
            # directly and fail; MACOSX_DEPLOYMENT_TARGET=14.0 would override the project's),
            # and cargo's cc crate reads CC. None of them belong in a shell without a Nix
            # compiler.
            for variable in CC CXX LD AR AS NM OBJCOPY OBJDUMP RANLIB READELF SIZE STRINGS STRIP \
              MACOSX_DEPLOYMENT_TARGET LD_DYLD_PATH ZERO_AR_DATE NIX_CC NIX_BINTOOLS \
              NIX_CFLAGS_COMPILE NIX_LDFLAGS NIX_HARDENING_ENABLE NIX_ENFORCE_NO_NATIVE \
              NIX_IGNORE_LD_THROUGH_GCC NIX_DONT_SET_RPATH NIX_NO_SELF_RPATH; do
              unset "$variable" "''${variable}_FOR_BUILD"
            done
            for variable in DEVELOPER_DIR SDKROOT; do
              if [[ "''${!variable:-}" == /nix/store/* ]]; then unset "$variable"; fi
            done
          '';

          shells = {
            # Also carries the validator's Python so `just ci` (which includes `just
            # validate`) runs in the default shell.
            core = shell {
              name = "core";
              packages = rustTools ++ [ python ];
            };

            # Also carries the validator's Python, like `core`, so `just ci` runs here too
            # (B061: the web bindings are built and tested from this shell).
            web = shell {
              name = "web";
              packages = rustTools ++ [
                python
                pkgs.nodejs_24 # current LTS line for apps/web tooling and tests
                pkgs.wasm-bindgen-cli # the wasm-bindgen crate in crates/ffi is pinned to this exact version
                pkgs.wasm-pack
                pkgs.binaryen # wasm-opt, for the size budgets
              ];
            };

            # The Android SDK and NDK are not provided here: on this host there is no room
            # for nixpkgs' androidenv closure, and the SDK carries its own license terms.
            # The shell pins JDK, Gradle and cargo-ndk and expects ANDROID_HOME and
            # ANDROID_NDK_HOME from the machine; apps/android/README.md says how.
            android = shell {
              name = "android";
              packages = rustTools ++ [
                pkgs.jdk21
                pkgs.gradle
                pkgs.cargo-ndk
              ];
              env.JAVA_HOME = "${pkgs.jdk21.home}";
              hook = ''
                if [ -z "''${ANDROID_HOME:-}" ] || [ -z "''${ANDROID_NDK_HOME:-}" ]; then
                  echo "warning: ANDROID_HOME or ANDROID_NDK_HOME is not set; Android builds need the SDK and NDK installed outside Nix (apps/android/README.md)." >&2
                fi
              '';
            };

            # Deterministic Rust pipeline (pipelines/build) plus the Python used for
            # hermetic oracles and evaluation (pipelines/ml) and the blueprint validator.
            pipeline = shell {
              name = "pipeline";
              packages = rustTools ++ [ python ];
            };

            # Runbooks, validation and host profiles; no compiler, no toolchain download.
            ops = shell {
              name = "ops";
              packages = [ python ];
            };
          }
          // lib.optionalAttrs pkgs.stdenv.isDarwin {
            # Apple builds: Rust with iOS/macOS targets and a verified Xcode. Xcode, Apple
            # SDKs, simulator runtimes, provisioning and signing stay outside Nix
            # (specs/20-orchestration-and-build-hosts.md).
            apple = shell {
              name = "apple";
              packages = rustTools ++ [
                pkgs.xcodegen # apps/apple/OpenLogos/project.yml -> OpenLogos.xcodeproj (gitignored)
              ];
              hook = ''
                # Warns on mismatch; the shell still opens so the problem can be fixed from inside it.
                scripts/xcode-check.sh || true
              '';
            };
          };
        in
        shells // { default = shells.core; }
      );

      checks = forEachSystem (
        pkgs:
        let
          system = pkgs.stdenv.hostPlatform.system;
          toolchain = toolchainFor pkgs;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };

          # One shape for every cargo-driven check: the workspace source, dependencies
          # vendored from Cargo.lock, a custom build phase and an empty output. Plain
          # makeRustPlatform rather than crane: the workspace has no third-party
          # dependencies yet, so there is nothing for crane's artifact caching to save,
          # and it keeps the flake to one extra input.
          cargoCheck =
            name: phase:
            rustPlatform.buildRustPackage {
              pname = "openlogos-${name}";
              version = "0";
              src = rustSource;
              cargoLock.lockFile = ./Cargo.lock;
              RUSTFLAGS = "-D warnings";
              buildPhase = ''
                runHook preBuild
                ${phase}
                runHook postBuild
              '';
              doCheck = false;
              installPhase = "mkdir -p $out";
              dontFixup = true;
            };

          # Lint checks that only need the repository and one tool.
          toolCheck =
            name: tools: script:
            pkgs.runCommandNoCC "openlogos-${name}" { nativeBuildInputs = tools; } ''
              cd ${self}
              ${script}
              mkdir -p $out
            '';

          common = {
            rustfmt = cargoCheck "rustfmt" "cargo fmt --all --check";
            clippy = cargoCheck "clippy" "cargo clippy --workspace --all-targets --locked -- -D warnings";
            tests = cargoCheck "tests" "cargo test --workspace --locked";
            # The crates that run in the browser worker must stay free of native-only code.
            # openlogos-retrieval-sqlite builds for wasm32 too (B064) but compiles SQLite with
            # an unwrapped clang (see CC_wasm32_unknown_unknown in the shells); inside this
            # sandboxed build that compiler is not wired yet, so `just check-wasm` covers it
            # and this check does not.
            wasm = cargoCheck "wasm" "cargo check --locked --target wasm32-unknown-unknown -p openlogos-core -p openlogos-text -p openlogos-workspaces -p openlogos-conformance -p openlogos-ffi";
            nixfmt = toolCheck "nixfmt" [
              pkgs.nixfmt
              pkgs.findutils
            ] "find . -name '*.nix' -print0 | xargs -0 nixfmt --check";
            shellcheck = toolCheck "shellcheck" [ pkgs.shellcheck ] "shellcheck scripts/*.sh";
            actionlint = toolCheck "actionlint" [ pkgs.actionlint ] "actionlint .github/workflows/*.yml";
            justfmt = toolCheck "justfmt" [ pkgs.just ] "just --fmt --check --unstable";
            blueprint = toolCheck "blueprint" [ (pythonFor pkgs) ] "python3 tools/validate.py";
          };

          # NixOS VM tests and other checks that cannot run on macOS live in nix/checks.
          # On Darwin they are left out of the set and named in a warning, never reported
          # as passed (specs/06-nix-operations.md).
          linuxOnly = import ./nix/checks { inherit pkgs; };
          skipped =
            lib.warnIf (linuxOnly != { })
              "openlogos: skipping Linux-only checks on ${system}: ${lib.concatStringsSep ", " (lib.attrNames linuxOnly)}"
              { };
        in
        common // (if pkgs.stdenv.isLinux then linuxOnly else skipped)
      );

      formatter = forEachSystem (pkgs: pkgs.nixfmt-tree);
    };
}
