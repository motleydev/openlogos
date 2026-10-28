// A pane's reading surface: the TextKit 2 text view (Sources/Reader, B005) showing monochrome
// Scripture in the system serif with Dynamic Type, scaled leading, subordinate verse numbers,
// the resource's own layout direction (Hebrew right to left inside a left-to-right workspace),
// the platform's own selection with the project actions in its edit menu, plus the loading,
// unavailable and read-only states (specs/21 §1, §7, §11). Every selection goes through the
// core and back (AnchorRoundTrip); the pane shows the anchors it got beneath the text, with the
// B004 action strip, so the round trip is visible in every screenshot.
import SwiftUI

/// Reports the text region's size so the reading-area overlay can sum it (LayoutMetrics).
struct ReadingAreaKey: PreferenceKey {
    static var defaultValue: [String: CGSize] = [:]
    static func reduce(value: inout [String: CGSize], nextValue: () -> [String: CGSize]) {
        value.merge(nextValue()) { $1 }
    }
}

struct PaneTextView: View {
    @ObservedObject var workspace: Workspace
    let pane: Pane
    let state: PrototypeState
    let width: CGFloat
    /// Kept for the Selection view: the pane whose selection the study drawer describes.
    var selectable = false
    /// A single pane caps its measure at about 55–75 Latin characters and centres the column
    /// (specs/16 "Prose measure"); multi-pane views are narrower than the cap anyway.
    var capMeasure = false
    var onCollect: ((String, String) -> Void)?
    var onStudy: (() -> Void)?

    @ScaledMetric(relativeTo: .title3) private var measure: CGFloat = ChromeMetrics.measure

    var body: some View {
        GeometryReader { geometry in
            let padding = ChromeMetrics.panePadding(width)
            let columnWidth = capMeasure ? min(geometry.size.width, measure + 2 * padding) : geometry.size.width
            content(paneWidth: columnWidth, padding: padding)
                .frame(width: columnWidth, height: geometry.size.height)
                .frame(maxWidth: .infinity)
                .preference(key: ReadingAreaKey.self, value: [pane.id: CGSize(width: columnWidth, height: geometry.size.height)])
        }
        .background(Color(white: 1, opacity: 0.001)) // a hit area for pane activation
        .simultaneousGesture(TapGesture().onEnded { workspace.activate(pane.id) })
    }

    @ViewBuilder private func content(paneWidth: CGFloat, padding: CGFloat) -> some View {
        if !pane.content.isInstalled {
            ScrollView { UnavailableSourceView(content: pane.content).padding(padding) }
        } else if state == .loading && pane.id == workspace.activePaneID {
            ScrollView { LoadingLines().padding(padding) }
        } else {
            VStack(alignment: .leading, spacing: 0) {
                if pane.content.isReferenceOnly {
                    StateNotice(symbol: "lock", text: "Reference only: this edition can be read and cited, not quoted. Collect adds the reference without text.")
                        .padding(.horizontal, padding)
                        .padding(.top, 12)
                }
                ReaderTextView(
                    paneID: pane.id,
                    content: ReaderContent(content: pane.content),
                    horizontalPadding: padding,
                    command: workspace.commands[pane.id],
                    onSelection: { selection in workspace.setSelection(selection, in: pane.id) },
                    onAction: { action, selection in perform(action, selection) },
                    onMeasurement: { summary in workspace.measurement = summary }
                )
                .accessibilityLabel("\(pane.content.fullTitle), \(pane.content.passage)")
                if let selection = workspace.selection, selection.paneID == pane.id {
                    SelectionActions(
                        selectedText: selection.text,
                        compact: paneWidth < 440,
                        onHighlight: { perform(.highlight, selection) },
                        onCollect: { perform(.collect, selection) },
                        onStudy: { perform(.study, selection) }
                    )
                    .padding(.horizontal, padding)
                    .padding(.top, 6)
                    AnchorReadout(selection: selection)
                        .padding(.horizontal, padding)
                        .padding(.vertical, 8)
                }
                if pane.id == "p1", let measurement = workspace.measurement {
                    Text(measurement)
                        .font(.system(.caption2, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .textSelection(.enabled)
                        .padding(.horizontal, padding)
                        .padding(.vertical, 8)
                        .accessibilityLabel("Measurement")
                }
            }
        }
    }

    /// The project actions, from the edit menu or the strip (specs/21 §3).
    private func perform(_ action: SelectionAction, _ selection: ReaderSelection?) {
        let text = selection?.text ?? ""
        switch action {
        case .highlight:
            workspace.confirmation = .init(message: "Highlighted \"\(text)\" with Emphasis", detail: "Undo · Change highlight")
        case .collect:
            let locator = "\(pane.content.passage.split(separator: "–").first ?? ""), \"\(text)\""
            if let onCollect { onCollect(text, locator) } else { workspace.collect(text: text, resource: pane.content, locator: locator) }
        case .study:
            if let onStudy { onStudy() } else { workspace.drawer = .study }
        default:
            workspace.confirmation = .init(message: "\(action.rawValue): prototype action for \"\(text)\"", detail: nil)
        }
    }
}

/// What the core made of the selection: one line per anchor (segment, scalar range, exact
/// text, token count) and the round-trip verdict. Monospaced, secondary, selectable.
struct AnchorReadout: View {
    let selection: ReaderSelection

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            ForEach(Array(selection.anchors.enumerated()), id: \.offset) { _, anchor in
                Text("\(anchor.segmentId) scalars [\(anchor.range.start), \(anchor.range.end)) → “\(anchor.quote?.exact ?? "")” · \(anchor.tokens.isEmpty ? "no tokens" : anchor.tokens.count == 1 ? anchor.tokens[0] : "\(anchor.tokens[0]) … \(anchor.tokens[anchor.tokens.count - 1]) (\(anchor.tokens.count))")")
            }
            Text("\(selection.identical ? "round trip identical" : "ROUND TRIP DIFFERS") · UTF-16 display [\(selection.displayRange.location), \(selection.displayRange.location + selection.displayRange.length)) → core [\(selection.coreRange.location), \(selection.coreRange.location + selection.coreRange.length)) → \(selection.highlights.count) highlight\(selection.highlights.count == 1 ? "" : "s")\(selection.snapped ? " · widened to grapheme clusters" : "") · \(String(format: "%.2f", selection.elapsedMilliseconds)) ms")
                .foregroundStyle(selection.identical ? Color.secondary : Color.red)
        }
        .font(.system(.caption2, design: .monospaced))
        .foregroundStyle(.secondary)
        .frame(maxWidth: .infinity, alignment: .leading)
        .textSelection(.enabled)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Anchors: \(selection.summary)")
    }
}

/// Highlight · Collect · Study · More, beneath the selection, each a 44-point target.
struct SelectionActions: View {
    let selectedText: String
    var compact = false
    let onHighlight: () -> Void
    let onCollect: () -> Void
    let onStudy: () -> Void

    var body: some View {
        HStack(spacing: 0) {
            action("Highlight", "highlighter", onHighlight).keyboardShortcut("h", modifiers: [.command, .shift])
            action("Collect", "tray.and.arrow.down", onCollect).keyboardShortcut("c", modifiers: [.command, .shift])
            action("Study", "text.magnifyingglass", onStudy).keyboardShortcut("s", modifiers: [.command, .shift])
            Menu {
                ForEach(SelectionAction.more, id: \.rawValue) { item in
                    Button(item.rawValue, systemImage: item.symbol) {}
                }
            } label: {
                actionLabel("More", "ellipsis")
            }
            .menuStyle(.button)
            .buttonStyle(.plain)
        }
        .background(RoundedRectangle(cornerRadius: 10).fill(.thickMaterial).shadow(radius: 3))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Selection actions for \(selectedText)")
    }

    private func action(_ title: String, _ symbol: String, _ action: @escaping () -> Void) -> some View {
        Button(action: action) { actionLabel(title, symbol) }
            .buttonStyle(.plain)
    }

    /// Icon beside the word in regular widths; icon above the word in compact width so four
    /// labelled actions fit 390 points without wrapping.
    @ViewBuilder private func actionLabel(_ title: String, _ symbol: String) -> some View {
        Group {
            if compact {
                VStack(spacing: 2) {
                    Image(systemName: symbol)
                    Text(title).font(.caption2)
                }
            } else {
                Label(title, systemImage: symbol).labelStyle(.titleAndIcon).font(.subheadline)
            }
        }
        .lineLimit(1)
        .fixedSize()
        .frame(minWidth: ChromeMetrics.minimumTarget, minHeight: ChromeMetrics.minimumTarget)
        .padding(.horizontal, compact ? 8 : 10)
        .contentShape(Rectangle())
    }
}

// MARK: States

/// Skeleton lines at the prose measure; the identity row above is already filled.
struct LoadingLines: View {
    @ScaledMetric(relativeTo: .title3) private var lineHeight: CGFloat = 14
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            ForEach(0..<7, id: \.self) { index in
                RoundedRectangle(cornerRadius: 4)
                    .fill(.quaternary)
                    .frame(maxWidth: index == 6 ? 180 : .infinity)
                    .frame(height: lineHeight)
            }
            HStack {
                Text("Still opening Berean Standard Bible…").font(.footnote).foregroundStyle(.secondary)
                Button("Cancel") {}.font(.footnote).frame(minHeight: ChromeMetrics.minimumTarget)
            }
            .padding(.top, 8)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Loading")
    }
}

/// The unavailable-source placeholder: name, passage and the three actions; no substitute text.
struct UnavailableSourceView: View {
    let content: PaneContent
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Label("Not installed", systemImage: "exclamationmark.triangle")
                .font(.headline)
            Text(content.fullTitle).font(.system(.body, design: .serif))
            Text("Remembered at \(content.passage). The pane keeps its place; no other edition is shown in its stead.")
                .font(.footnote).foregroundStyle(.secondary)
            HStack(spacing: 8) {
                Button("Install") {}.buttonStyle(.bordered)
                Button("Choose another") {}.buttonStyle(.bordered)
                Button("Close pane") {}.buttonStyle(.borderless)
            }
            .frame(minHeight: ChromeMetrics.minimumTarget)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
        .background(RoundedRectangle(cornerRadius: 10).strokeBorder(.secondary.opacity(0.5), style: StrokeStyle(lineWidth: 1, dash: [6])))
    }
}

struct StateNotice: View {
    let symbol: String
    let text: String
    var body: some View {
        Label(text, systemImage: symbol)
            .font(.footnote)
            .foregroundStyle(.secondary)
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 8).fill(.quaternary.opacity(0.5)))
    }
}
