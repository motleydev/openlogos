# Orchestration and build hosts

Revision 5. The owner intends Paperclip [S01] to direct implementation of this repository: Paperclip holds the agents and issues derived from specs/agents.json and specs/backlog.json, and its agents produce reviewable changes here. A Mac Mini acts as the primary agent and build host, because Apple builds, simulators and attached test devices need macOS. This document is a desired topology. It does not provision anything, and specs/00-import-contract.md still governs how records are imported.

## Interim operation without Paperclip (from 27 October 2026)

The owner deferred Paperclip. Until it is adopted, the orchestrator is a **Claude Code session on the Mac mini**, working with the owner, under the same discipline Paperclip would impose:

- **One backlog task per branch or worktree**, named after the task (`b061-core-scaffold`). Subagents implement; the orchestrator audits their trail by reading the diff and running the build and tests itself, never by accepting a summary.
- **Small, frequent commits** with the task ID in the message, so every step is reviewable and revertible. Evidence (test output, measured sizes, screenshots) is recorded in the pull request body or in `docs/evidence/`, not only in chat.
- **Every change lands through a pull request that the owner merges.** No orchestrator or agent pushes to `main`. If pull-request tooling is unavailable, work stops at the pushed branch.
- **Paperclip import stays possible**: specs/agents.json and specs/backlog.json remain the task source of truth and are updated when scope changes, so B001's Paperclip mapping can run later without rework.

**Mac mini status (checked 27 October 2026).** The orchestrator already runs on the Mac mini (`office-hub`, Apple M1, macOS 26.4, Xcode 26.6). It holds a valid Apple Development signing identity in the owner's login keychain and already serves test builds for another project over Tailscale (the `scripts/ship.sh` and shelf pattern in the owner's sudoku-color repository). OpenLogos reuses that pattern: `scripts/sim.sh` for unsigned simulator runs and screenshots, `scripts/ship.sh` for signed ad hoc builds onto the shelf, settings such as the team ID in a git-ignored `.envrc.local`. The isolated signing keychain described below is a hardening step required before any agent runs unattended; it is not yet in place.

## Roles of each system

| System | Responsibility | Never does |
|---|---|---|
| This repository | Source of truth for specs, backlog, code, lockfiles and ADRs | Hold credentials, signing keys or Paperclip deployment state |
| Paperclip | Agents, issues, dependency-aware ready queue, budgets, run leases | Merge to `main`, sign releases or enable a phase without the owner |
| Mac Mini (agent + build host) | Runs Paperclip agent adapters in isolated worktrees; builds and tests every platform; hosts simulators and tethered devices; self-hosted CI runner | Expose signing identities to agent runs |
| GitHub | Pull requests, review, required checks, protected `main` | Run untrusted code with release secrets |
| Owner's laptop / iPad | Review, Paperclip board, device testing | Act as an unattended build host |

## Default topology

1. **Paperclip and its local adapters run on the Mac Mini.** Agents (for example via `claude_local` or `codex_local` adapters) check out one git worktree per leased issue, so Xcode, the Rust toolchains, the Android SDK, Nix and simulators are local to the run. There are no remote hops for ordinary builds.
2. **Every change goes through a pull request.** Agents push branches and open pull requests; `main` is protected and requires checks plus owner or designated-reviewer approval.
3. **The Mac Mini is also a self-hosted CI runner** for jobs that need macOS: Apple builds, UI and conformance tests on simulators and tethered iPad/iPhone, and macOS notarization dry runs. Linux-capable jobs (Rust core tests, WebAssembly, web, Android, NixOS VM tests) may run on hosted runners or a Linux machine. Fork and agent-authored pull requests run on a runner without release secrets.
4. **Trusted release lane.** Signing certificates, provisioning profiles, the App Store Connect key and package-signing keys live in a separate macOS user and keychain (or an equivalent isolated host) used only by the release workflow after human approval. Agent runs cannot read that keychain. Code signatures and corpus signatures keep separate keys and owners (specs/06-nix-operations.md).
5. **Remote build fallback.** If agents later run elsewhere (a Linux host or cloud runtime), the Mac Mini becomes a remote Apple build target, reached through a single checked-in entry point (`ops/apple-build`) that runs a named build or test job over SSH or triggers the corresponding CI workflow. Agents call that entry point; they do not compose ad-hoc remote shell commands.

## Environments on the Mac Mini

- `nix develop .#core`, `.#web`, `.#android`, `.#pipeline`, `.#ops`: fully pinned shells.
- `nix develop .#apple`: pins Rust targets, UniFFI tooling, SwiftLint/format tools and project generators, and selects a recorded Xcode path/version. Xcode, Apple SDKs and simulator runtimes remain external licensed prerequisites; the shell verifies the expected version instead of claiming hermeticity.
- Resource ceilings: at most two concurrent agent coding runs (specs/09-roadmap.md), one simulator-heavy job at a time, and separate CPU/RAM limits for local models and OCR so agents and builds cannot starve each other.

## What agents need from this repository

- A short entry document per area (`crates/README.md`, `apps/apple/README.md`, ...) naming build/test commands. Root auto-loaded instruction files (`AGENTS.md`, `CLAUDE.md`) remain forbidden in this data bundle by tools/validate.py; Paperclip supplies role instructions through its own configuration as described in specs/00-import-contract.md.
- Deterministic test entry points: `nix flake check` for Linux-capable checks and `ops/apple-build test` for Apple checks, each producing machine-readable results that Paperclip can attach to an issue as evidence.
- Small licensed or synthetic fixtures only; no corpus downloads inside agent runs except through the approved acquisition stage (specs/04-ingestion.md).

## Acceptance (B063)

- Paperclip installed version, adapters and Mac Mini toolchain versions recorded in an operations report outside this repository's credentials.
- An agent run in an isolated worktree builds the Rust core, the Apple app for an iPad simulator and the web reader, and attaches test results to its issue.
- A pull request from an agent run cannot access signing material; a negative test demonstrates the keychain is unavailable to that user.
- Two concurrent runs plus one CI job complete without resource exhaustion or worktree collisions.
