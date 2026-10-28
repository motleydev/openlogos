// OpenLogos app shell (B063). This is a shell, not the reader: it exists so the Mac mini lanes
// (scripts/sim.sh, scripts/ship.sh) have a real SwiftUI app that links the Rust core through
// the UniFFI bindings. B062 replaces ReadingShellView with the TextKit 2 reading surface
// (docs/adr/0001-rust-core-native-ui.md, specs/18-platform-capabilities.md).
import SwiftUI

@main
struct OpenLogosApp: App {
    var body: some Scene {
        WindowGroup {
            // B004: the reading-interactions prototype (Sources/Prototype/) is the root; the
            // B063 shell stays reachable as the "B063 shell" view in its debug bar, or with
            // the launch argument `-OPENLOGOS_VIEW shell`.
            PrototypeRoot()
        }
    }
}
