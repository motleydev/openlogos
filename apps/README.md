# apps

Thin native interfaces over the Rust core (docs/adr/0001-rust-core-native-ui.md). Each app
directory has its own entry document naming its build and test commands once it exists.

| Directory | Platform | Status |
|---|---|---|
| `apple/` | SwiftUI + TextKit 2 for iPadOS, macOS, iOS | not started; B005 and B061 gate it |
| `android/` | Kotlin + Jetpack Compose | not started |
| `web/` | DOM reader, static build, offline shell, core as WebAssembly in a worker | not started |
| `desktop/` | Tauri shell for Windows and Linux over the web interface | not started |
| `review/` | evidence and reviewer decisions | not started |
