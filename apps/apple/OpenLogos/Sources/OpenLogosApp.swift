// OpenLogos app shell (B063). This is a shell, not the reader: it exists so the Mac mini lanes
// (scripts/sim.sh, scripts/ship.sh) have a real SwiftUI app that links the Rust core through
// the UniFFI bindings. B062 replaces ReadingShellView with the TextKit 2 reading surface
// (docs/adr/0001-rust-core-native-ui.md, specs/18-platform-capabilities.md).
import SwiftUI

@main
struct OpenLogosApp: App {
    #if os(macOS)
    /// B072: the workspace shell is the Mac app's view; the B004 prototype stays one Debug
    /// menu item away, and opens directly when a prototype view is named at launch
    /// (`-OPENLOGOS_VIEW anchors`, as scripts/b005-shots.sh and b039-shots.sh do).
    @State private var showPrototype = UserDefaults.standard.string(forKey: "OPENLOGOS_VIEW") != nil
    #endif

    var body: some Scene {
        WindowGroup {
            #if os(macOS)
            if showPrototype {
                PrototypeRoot()
            } else {
                ShellView(model: ShellLaunch.measurementPath == nil ? ShellModel() : ShellModel(sessionJSON: ShellSeed.measurementJSON))
            }
            #else
            // B004: the reading-interactions prototype (Sources/Prototype/) is the root on
            // iPad until the shell lands there (B075); the B063 shell stays reachable as the
            // "B063 shell" view in its debug bar, or with `-OPENLOGOS_VIEW shell`.
            PrototypeRoot()
            #endif
        }
        #if os(macOS)
        .commands {
            CommandMenu("Debug") {
                // No shortcut: the shell binds only what the core registry lists.
                Toggle("B004 prototype", isOn: $showPrototype)
            }
        }
        #endif
    }
}
