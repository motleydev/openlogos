// The measurement behind docs/evidence/B004.md: useful reading area for layouts A and B at a
// viewport size, computed from the same ChromeMetrics the views lay out with, and checked
// against what the rendered panes report (ReadingAreaKey). "Useful reading area" is the sum of
// the pane text regions (the scroll areas inside each pane's identity row and the shared chrome,
// before text padding) with drawers closed; dividers, bars, launcher, tab rows and the debug
// bar are chrome. Command duplication and interaction counts are counted by hand in the
// evidence file from the rendered views, not here.
import Foundation
import SwiftUI

struct LayoutMetrics: Equatable {
    let size: CGSize
    let layout: LayoutVariant
    let panes: Int
    let visiblePanes: Int
    let chromeHeight: CGFloat // vertical chrome above/below the text region of one pane
    let chromeWidth: CGFloat // horizontal chrome beside the panes (launcher)
    let paneTextRegions: [CGSize]
    var readingArea: CGFloat { paneTextRegions.reduce(0) { $0 + $1.width * $1.height } }
    var readingFraction: CGFloat { readingArea / (size.width * size.height) }

    /// The designed values at normal type with every drawer closed.
    static func designed(size: CGSize, layout: LayoutVariant, panes: Int) -> LayoutMetrics {
        let widthClass = WidthClass(width: size.width)
        let visible = min(panes, widthClass.visiblePanes)
        let tabs: CGFloat = visible < panes ? ChromeMetrics.tabsRow : 0
        var top: CGFloat = 0, bottom: CGFloat = 0, side: CGFloat = 0
        switch layout {
        case .a:
            top = ChromeMetrics.sharedBar + tabs + ChromeMetrics.identityRowA
        case .b:
            if widthClass == .compact {
                bottom = ChromeMetrics.launcherStrip
                // The one visible pane is the active one and carries the passage field row.
                top = tabs + ChromeMetrics.headerB + ChromeMetrics.passageRowB
            } else {
                side = ChromeMetrics.launcherWidth
                top = tabs + ChromeMetrics.headerB
            }
        }
        let dividers = CGFloat(max(visible - 1, 0)) * ChromeMetrics.divider
        let paneWidth = (size.width - side - dividers) / CGFloat(visible)
        let paneHeight = size.height - top - bottom
        return LayoutMetrics(
            size: size, layout: layout, panes: panes, visiblePanes: visible,
            chromeHeight: top + bottom, chromeWidth: side,
            paneTextRegions: Array(repeating: CGSize(width: paneWidth, height: paneHeight), count: visible)
        )
    }

    /// The table docs/evidence/B004.md prints, for three panes.
    static func table(panes: Int = 3) -> String {
        var lines = ["| Width × height | Layout | Visible panes | Pane text region (pt) | Reading area (pt²) | Share of viewport |", "|---|---|---|---|---|---|"]
        for size in PrototypeOptions.measurementSizes {
            for layout in LayoutVariant.allCases {
                let m = designed(size: size, layout: layout, panes: panes)
                let region = m.paneTextRegions.first ?? .zero
                lines.append("| \(Int(size.width)) × \(Int(size.height)) | \(layout.rawValue) | \(m.visiblePanes) | \(m.visiblePanes) × \(format(region.width)) × \(format(region.height)) | \(format(m.readingArea)) | \(Int((m.readingFraction * 100).rounded()))% |")
            }
        }
        return lines.joined(separator: "\n")
    }

    static func format(_ value: CGFloat) -> String {
        let formatter = NumberFormatter()
        formatter.numberStyle = .decimal
        formatter.maximumFractionDigits = value < 100 ? 1 : 0
        return formatter.string(from: NSNumber(value: Double(value))) ?? "\(value)"
    }
}

/// The on-screen overlay: designed versus rendered reading area, so a screenshot carries its
/// own check. Rendered sizes come from the panes' GeometryReaders.
struct MetricsOverlay: View {
    let size: CGSize
    let layout: LayoutVariant
    let panes: Int
    let rendered: [String: CGSize]

    var body: some View {
        let designed = LayoutMetrics.designed(size: size, layout: layout, panes: panes)
        let renderedArea = rendered.values.reduce(CGFloat(0)) { $0 + $1.width * $1.height }
        VStack(alignment: .leading, spacing: 2) {
            Text("\(Int(size.width))×\(Int(size.height)) · layout \(layout.rawValue) · \(designed.visiblePanes)/\(panes) panes")
            Text("designed \(LayoutMetrics.format(designed.readingArea)) pt² (\(Int((designed.readingFraction * 100).rounded()))%)")
            Text("rendered \(LayoutMetrics.format(renderedArea)) pt² (\(Int((renderedArea / (size.width * size.height) * 100).rounded()))%) · \(rendered.count) regions")
        }
        .font(.system(.caption2, design: .monospaced))
        .padding(6)
        .background(RoundedRectangle(cornerRadius: 6).fill(.thickMaterial))
        .padding(8)
        .accessibilityHidden(true)
        .onAppear { MetricsLog.write(size: size, layout: layout, designed: designed, renderedArea: renderedArea, regions: rendered) }
        .onChange(of: rendered) { _, new in
            let area = new.values.reduce(CGFloat(0)) { $0 + $1.width * $1.height }
            MetricsLog.write(size: size, layout: layout, designed: designed, renderedArea: area, regions: new)
        }
    }
}

/// Appends one JSON line per measurement to Documents/b004-metrics.jsonl so a script can read
/// the rendered numbers back out of the simulator container.
enum MetricsLog {
    static func write(size: CGSize, layout: LayoutVariant, designed: LayoutMetrics, renderedArea: CGFloat, regions: [String: CGSize]) {
        guard let directory = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first else { return }
        let url = directory.appendingPathComponent("b004-metrics.jsonl")
        let record: [String: Any] = [
            "width": Int(size.width), "height": Int(size.height), "layout": layout.rawValue,
            "visiblePanes": designed.visiblePanes,
            "designedArea": Int(designed.readingArea.rounded()),
            "renderedArea": Int(renderedArea.rounded()),
            "regions": regions.mapValues { [Int($0.width.rounded()), Int($0.height.rounded())] },
            "view": UserDefaults.standard.string(forKey: "OPENLOGOS_VIEW") ?? "read",
            "typeSize": UserDefaults.standard.string(forKey: "UIPreferredContentSizeCategoryName") ?? "default",
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: record), var line = String(data: data, encoding: .utf8) else { return }
        line += "\n"
        if let handle = try? FileHandle(forWritingTo: url) {
            handle.seekToEndOfFile()
            handle.write(Data(line.utf8))
            try? handle.close()
        } else {
            try? line.write(to: url, atomically: true, encoding: .utf8)
        }
    }
}
