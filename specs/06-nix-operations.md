# Nix development and self-hosting contract

This blueprint specifies desired outputs. It does not contain a tested flake, pinned application dependencies or production modules. The infrastructure phase creates and verifies them; no placeholder lock hashes are accepted.

## Intended implementation repository

```text
crates/core/              references, versification mappings, anchors, typed claims
crates/text/              render-ready attributed runs, normalization, offset adapters
crates/search/            corpus query engine and prose full-text (specs/19)
crates/retrieval/         optional vector and hybrid retrieval (specs/19)
crates/package/           safe archive, hashes, signature verification, index layers
crates/collections/       study collections, export receipts, Markdown/JSON export
crates/workspaces/        pane trees, link groups, layout profiles
crates/ink/               portable stroke geometry and snapshot contracts
crates/notes/             app/vault note authority and merge contracts
crates/sync/              encrypted operation log and conflict resolution
crates/ffi/               UniFFI and wasm-bindgen surfaces, versioned API
crates/conformance/       cross-platform fixtures and expected results
apps/apple/               SwiftUI app for iPadOS, macOS and iOS
apps/android/             Kotlin + Compose app
apps/web/                 DOM reader, static build and offline shell
apps/desktop/             Tauri shell for Windows/Linux
apps/review/              evidence and reviewer decisions
contracts/                JSON Schema, OpenAPI and generated types
design/tokens/            single-source design tokens generated per platform
plugins/obsidian/         native block embedding and vault bridge
services/hub/             optional identity-aware encrypted sync API (Rust)
services/companion/       optional scoped vault/research runtime (Rust)
services/catalog/         optional static catalog builder
pipelines/build/          Rust acquisition-to-package build (specs/04, specs/19)
pipelines/ml/             Python experiments: OCR, alignment, embeddings, evaluation
corpus/registry/          metadata/rights records; no large copyrighted dumps
fixtures/                 small licensed or synthetic fixtures
nix/modules/              NixOS services and profiles
nix/checks/               VM integration tests
nix/darwin/               macOS developer/CI environment
ops/                      runbooks, apple-build entry point, host profiles
docs/adr/                 architecture decision records
specs/                    inert product and agent data
flake.nix                 explicit packages/devShells/checks/modules
flake.lock                actual pinned revisions generated and reviewed
```

## Required outputs and commands

- `nix develop .#core`, `.#web`, `.#android`, `.#apple`, `.#pipeline`, `.#ops`: pinned Rust toolchains and targets, UniFFI/wasm-bindgen tooling, package-manager versions, code generators, OCR dependencies and test clients. `.#apple` verifies a recorded Xcode version rather than claiming hermeticity. Build-host topology is in specs/20-orchestration-and-build-hosts.md.
- `nix flake check`: schema validation, formatting, lint/type tests, unit/property tests, relevant deterministic fixtures and NixOS VM tests where platform-supported.
- `nix build .#web`, `.#hub`, `.#corpus-tools`: immutable deliverables with provenance and dependency/SBOM output. Distribution of the corpus is separate from the build code.
- Android release/debug packages through a pinned SDK/NDK/JDK/Gradle path with declared SDK license acceptance [S19]. No unrecorded downloads during a purportedly hermetic build; explicit dependency prefetch/lock phases precede offline checks.
- macOS developer shell and CI wrappers pin the open-source toolchain, Swift package resolution, JS package locks and selected Xcode path/version. Apple SDK/Xcode, device provisioning and signing remain external licensed prerequisites; do not claim iOS release archives build on Linux or are fully Nix-hermetic.
- `nixosModules.open-bible-web`, `open-bible-hub`, `open-bible-worker`, plus composable personal-hub and maintainer profiles. Names are desired outputs, not existing module claims.

Choose current compatible versions at implementation time and commit real lockfiles. Support x86_64-linux/aarch64-linux for servers and appropriate Darwin development; skip impossible platform checks explicitly rather than reporting them passed. Nix store paths never contain tokens, private notes, signing keys, license keys or unpublished private corpora.

## Service defaults

NixOS units run as separate unprivileged users with writable state directories, read-only system paths and minimal network permissions. Parser workers have no outbound network; acquisition is a separate allowlisted stage. Use systemd hardening, resource limits, schema-version startup checks and health endpoints without personal data.

Secrets come from operator-managed encrypted secret files (e.g. sops-nix/agenix after verification) or runtime credential injection. Public config exports contain no values. All ports, origins, issuer URLs and storage locations are explicit module options. TLS and trust roots work over a private mesh without a public internet dependency after provisioning; avoid relying silently on a public ACME service in the fully isolated profile.

## Host profiles

1. Static reader: bundled assets/catalog, explicit offline caching, security headers and optional network restriction.
2. Personal hub: static reader + hub + PostgreSQL + self-hosted OIDC. Co-locate initially; no mandatory Redis/Kafka/object-storage cloud account.
3. Mesh add-on: Headscale + optional self-operated DERP, tested public-relay exclusion, DNS and access policy. Bootstrap access to identity and coordination cannot depend on a not-yet-enrolled tunnel; document initial administrative path and failure recovery.
4. Maintainer: acquisition, OCR/enrichment worker, review UI and publishing job. Start with filesystem object store; add S3-compatible storage only if measured need warrants it.

## CI and release

Separate untrusted PR checks from trusted release/signing. No signing secrets on fork/agent code execution. Lockfile changes are reviewable; dependency license checks and vulnerability triage accompany build checks. Generate checksums, SBOM, provenance and reproducibility notes for every release. Code signatures and corpus signatures have separate keys and owners.

First deploy to a disposable NixOS VM. Test identity enrollment, unauthorized access, offline reader, sync convergence, key recovery, interrupted transfer, database migration, rollback and restore. A service rollback after schema change may require restore or forward-fix; do not assume an old binary can read a migrated database.

Backups include database, package objects, rights evidence, review events and server key material under separate encrypted policies. Client recovery material remains user-held. Restore into a fresh host and compare counts/hashes; document recovery objectives measured in that test. Operator logs omit reading history, note text, tokens and raw document content by default.

## Paperclip runtime environment

Pin Paperclip itself and CLI adapters in the operations environment after inspecting the installation. Runtime model tokens are injected only into approved jobs. Worktrees isolate writes; shared dependency/Nix caches may be read concurrently with appropriate cache locking. Allocate RAM/CPU ceilings so OCR, mobile builds and multiple agents do not exhaust the same machine. Run-policy caps do not replace OS isolation.

## Revision 2 environments and hosting

Add `nix develop .#obsidian` and `.#research` with pinned plugin tooling, conformance fixtures and provider runtime compatibility probes. Add a separate optional `open-bible-companion` module with per-user state/credential directories, explicit listen address/origin checks, HTTPS and mesh access. No provider CLI is mandatory in the core build. Proprietary external runtimes (where selected) remain optional, unmodified and subject to their actual distribution terms; installation/packaging choices must not silently redistribute them under the project's code license. Provider credentials are provisioned at runtime only.

Test browser/mobile access to a personal host, token renewal/revocation, per-user isolation, no silent paid fallback, and shutdown during an active chat. CLI tools run in isolated empty workspaces; vault/corpus content cannot supply auto-loaded configuration. Local models and OCR resources have separate CPU/GPU/RAM caps so they cannot stall reading or the sync service.
