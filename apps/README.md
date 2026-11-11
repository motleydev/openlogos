# apps

Thin native interfaces over the Rust core (docs/adr/0001-rust-core-native-ui.md). Each app
directory has its own entry document naming its build and test commands. `just ci-hosts` runs
every app's and binding's tests in turn; specs/backlog-status.json says what is merged.

| Directory | Platform | Status |
|---|---|---|
| `apple/` | SwiftUI + TextKit 2 for iPadOS, macOS, iOS | the workspace shell on the Mac (specs/22, B072–B078); iPad runs the earlier reading prototypes in the simulator, the shell on iPad is B075 |
| `android/` | Kotlin + Jetpack Compose | the Kotlin binding and its JVM test host (B061); no app |
| `web/` | DOM reader, static build, offline shell, core as WebAssembly in a worker | the workspace shell in `web/shell` in lockstep with the Mac (B072–B078), over the wasm binding in `web/core-host`; the reader and ink spikes it reuses |
| `desktop/` | Tauri shell for Windows and Linux over the web interface | not started |
| `review/` | evidence and reviewer decisions | not started |
