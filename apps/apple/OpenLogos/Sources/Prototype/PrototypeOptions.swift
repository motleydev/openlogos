// B004 reading-interactions prototype: what the debug switches and launch arguments select.
//
// This is a fidelity prototype of interactions and typography (specs/21-reading-interactions.md,
// specs/16-design-guide.md), not the production reader (B062). Every piece of data it shows is a
// hard-coded fixture and is labelled as such on screen. No network, account, store or model call.
//
// Launch arguments (UserDefaults reads `-Key value`), all optional:
//   -OPENLOGOS_VIEW    shell|read|three|collection|selection|search|margin|handoff
//   -OPENLOGOS_LAYOUT  A|B                 (specs/16 "two compact layouts")
//   -OPENLOGOS_STATE   normal|loading|empty|offline|unavailable|invalidQuery|readOnly|conflict
//   -OPENLOGOS_WIDTH   390                 simulated viewport width in points (height optional)
//   -OPENLOGOS_HEIGHT  844
//   -OPENLOGOS_METRICS 1                   show the measured reading-area overlay
//   -OPENLOGOS_FOCUS   1                   start in focus mode
//   -OPENLOGOS_DEBUGBAR 0                  hide the debug bar (screenshots of the frame alone)
//   B005 (view `anchors`, Sources/Reader):
//   -OPENLOGOS_CASE cross-verse            drive one AnchorCase through the surface at launch (or `all`, one after another)
//   -OPENLOGOS_PERF 1                      pane 1 is a synthetic 1,000-verse set; the scroll and selection measurement runs
//   -OPENLOGOS_SYNTH 1000                  verses in the synthetic set
//   -OPENLOGOS_LOG 1                       append round trips and measurements to Documents/b005-*.jsonl
//   -OPENLOGOS_APPEARANCE light|dark       macOS only: fix the app appearance for a capture
// Dynamic Type comes from the system: -UIPreferredContentSizeCategoryName (scripts/sim.sh).
import SwiftUI

enum PrototypeView: String, CaseIterable, Identifiable {
    case shell, read, three, collection, selection, search, margin, handoff, anchors
    var id: String { rawValue }

    /// The names specs/16 "Prototype and usability gate" uses.
    var title: String {
        switch self {
        case .shell: return "B063 shell"
        case .read: return "Read"
        case .three: return "Three texts, drawers closed"
        case .collection: return "Three texts + collection"
        case .selection: return "Selection and word study"
        case .search: return "Search results"
        case .margin: return "Margin capture"
        case .handoff: return "Obsidian handoff"
        case .anchors: return "B005 anchors (3 texts + note)"
        }
    }
}

enum LayoutVariant: String, CaseIterable, Identifiable {
    case a = "A", b = "B"
    var id: String { rawValue }
    var title: String {
        switch self {
        case .a: return "A · shared bar, inline labels, transient drawers"
        case .b: return "B · edge launcher, passage field in active pane, local headers"
        }
    }
}

/// The required states of specs/16 "Recovery and states"; each view reaches the ones that
/// apply to it (specs/21 §11). `normal` is the fixture's happy path.
enum PrototypeState: String, CaseIterable, Identifiable {
    case normal, loading, empty, offline, unavailable, invalidQuery, readOnly, conflict
    var id: String { rawValue }
}

@MainActor
final class PrototypeOptions: ObservableObject {
    @Published var view: PrototypeView
    @Published var layout: LayoutVariant
    @Published var state: PrototypeState
    /// A simulated viewport, so compact and desktop widths can be rendered on the one iPad
    /// simulator this machine may boot. nil means "the window".
    @Published var simulatedSize: CGSize?
    @Published var showMetrics: Bool
    @Published var focusMode: Bool
    @Published var showDebugBar: Bool
    /// B005 launch-time drivers: a named AnchorCase (or `all`) and the measurement.
    let launchCase: String?
    let launchMeasurement: Bool

    init(defaults: UserDefaults = .standard) {
        launchCase = defaults.string(forKey: "OPENLOGOS_CASE")
        launchMeasurement = defaults.bool(forKey: "OPENLOGOS_PERF")
        view = PrototypeView(rawValue: defaults.string(forKey: "OPENLOGOS_VIEW") ?? "") ?? .read
        layout = LayoutVariant(rawValue: defaults.string(forKey: "OPENLOGOS_LAYOUT") ?? "") ?? .a
        state = PrototypeState(rawValue: defaults.string(forKey: "OPENLOGOS_STATE") ?? "") ?? .normal
        let width = defaults.double(forKey: "OPENLOGOS_WIDTH")
        let height = defaults.double(forKey: "OPENLOGOS_HEIGHT")
        if width > 0 {
            simulatedSize = CGSize(width: width, height: height > 0 ? height : Self.defaultHeight(forWidth: width))
        } else {
            simulatedSize = nil
        }
        showMetrics = defaults.bool(forKey: "OPENLOGOS_METRICS")
        focusMode = defaults.bool(forKey: "OPENLOGOS_FOCUS")
        showDebugBar = defaults.object(forKey: "OPENLOGOS_DEBUGBAR") == nil ? true : defaults.bool(forKey: "OPENLOGOS_DEBUGBAR")
    }

    /// The four measurement widths of specs/16 with the heights docs/evidence/B004.md uses.
    static let measurementSizes: [CGSize] = [
        CGSize(width: 1440, height: 900), // desktop window
        CGSize(width: 1024, height: 768), // iPad landscape, split view, small desktop
        CGSize(width: 768, height: 1024), // iPad portrait
        CGSize(width: 390, height: 844), // phone-class width
    ]

    static func defaultHeight(forWidth width: Double) -> Double {
        measurementSizes.first { Double($0.width) == width }.map { Double($0.height) } ?? (width < 700 ? 844 : 900)
    }
}

/// Whether a width is "compact" for the pane model (specs/21 §10: one pane plus tabs under
/// about 700 points, two plus a tab under about 1000, three above).
enum WidthClass {
    case compact, medium, regular
    init(width: CGFloat) {
        if width < 700 { self = .compact } else if width < 1000 { self = .medium } else { self = .regular }
    }
    var visiblePanes: Int {
        switch self {
        case .compact: return 1
        case .medium: return 2
        case .regular: return 3
        }
    }
}
