// OpenLogos app shell (B063). This is a shell, not the reader: it exists so the Mac mini lanes
// (scripts/sim.sh, scripts/ship.sh) have a real SwiftUI app that links the Rust core through
// the UniFFI bindings. B062 replaces ReadingShellView with the TextKit 2 reading surface
// (docs/adr/0001-rust-core-native-ui.md, specs/18-platform-capabilities.md).
import SwiftUI

@main
struct OpenLogosApp: App {
    var body: some Scene {
        WindowGroup {
            ReadingShellView()
        }
    }
}
