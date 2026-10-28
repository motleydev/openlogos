// A pane's reading surface: monochrome Scripture in the system serif with Dynamic Type, scaled
// leading, subordinate verse numbers, the resource's own layout direction (Hebrew right to left
// inside a left-to-right workspace), native text selection, and the loading, unavailable and
// read-only states (specs/21 §1, §7, §11). The word-level selection used by the Selection view
// is a flow of tappable words because SwiftUI Text does not expose its selected range; the
// production reader (B062) extends the native edit menu instead (specs/21 §3).
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
    /// Word-level selection on verse 1 of this pane (Selection view only).
    var selectable = false
    var onCollect: ((String, String) -> Void)?
    var onStudy: (() -> Void)?

    @ScaledMetric(relativeTo: .title3) private var leading: CGFloat = 9

    var body: some View {
        GeometryReader { geometry in
            ScrollView {
                content
                    .padding(.horizontal, ChromeMetrics.panePadding(width))
                    .padding(.vertical, 20)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .preference(key: ReadingAreaKey.self, value: [pane.id: geometry.size])
        }
        .environment(\.layoutDirection, pane.content.isRightToLeft ? .rightToLeft : .leftToRight)
        .background(Color(white: 1, opacity: 0.001)) // a hit area for pane activation
        .onTapGesture { workspace.activate(pane.id) }
    }

    @ViewBuilder private var content: some View {
        if !pane.content.isInstalled {
            UnavailableSourceView(content: pane.content)
        } else if state == .loading && pane.id == workspace.activePaneID {
            LoadingLines()
        } else {
            VStack(alignment: .leading, spacing: 16) {
                if pane.content.isReferenceOnly {
                    StateNotice(symbol: "lock", text: "Reference only: this edition can be read and cited, not quoted. Collect adds the reference without text.")
                }
                if selectable, let first = pane.content.verses.first {
                    SelectableVerse(workspace: workspace, pane: pane, verse: first, compact: WidthClass(width: width) == .compact, onCollect: onCollect, onStudy: onStudy)
                    paragraph(Array(pane.content.verses.dropFirst()))
                } else {
                    paragraph(pane.content.verses)
                }
                Text("Fixture: \(pane.content.fullTitle). \(pane.content.license)")
                    .font(.system(.caption, design: .serif))
                    .foregroundStyle(.secondary)
                    .padding(.top, 8)
            }
            .textSelection(.enabled)
        }
    }

    private func paragraph(_ verses: [Passage.Verse]) -> some View {
        verses.enumerated().reduce(Text("")) { paragraph, element in
            let (index, verse) = element
            let separator = index == 0 ? Text("") : Text(" ")
            let body = Text(verse.text)
            guard !verse.number.isEmpty else { return paragraph + separator + body }
            let number = Text(verse.number)
                .font(.system(.caption2, design: .serif))
                .foregroundStyle(.secondary)
                .baselineOffset(7)
            return paragraph + separator + number + Text("\u{2009}") + body
        }
        .font(.system(.title3, design: .serif))
        .lineSpacing(leading)
        .multilineTextAlignment(.leading)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// One verse as tappable words: tap selects, tap another extends, and the project actions appear
/// under the verse rather than over it (specs/21 §3; native Copy stays on the paragraph above).
struct SelectableVerse: View {
    @ObservedObject var workspace: Workspace
    let pane: Pane
    let verse: Passage.Verse
    var compact = false
    var onCollect: ((String, String) -> Void)?
    var onStudy: (() -> Void)?
    @ScaledMetric(relativeTo: .title3) private var leading: CGFloat = 9

    private var wordList: [String] { words(of: verse.text) }

    private var selectedText: String {
        guard let range = workspace.selectedWordRange else { return "" }
        return wordList[range.clamped(to: 0...(wordList.count - 1))].joined(separator: " ")
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            WordFlow(spacing: 4, lineSpacing: leading) {
                Text(verse.number)
                    .font(.system(.caption2, design: .serif))
                    .foregroundStyle(.secondary)
                    .baselineOffset(7)
                    .accessibilityLabel("verse \(verse.number)")
                ForEach(Array(wordList.enumerated()), id: \.offset) { index, word in
                    let selected = workspace.selectedWordRange?.contains(index) ?? false
                    Text(word)
                        .font(.system(.title3, design: .serif))
                        .padding(.horizontal, 2)
                        .background(
                            RoundedRectangle(cornerRadius: 3)
                                .fill(selected ? Color.primary.opacity(0.14) : .clear)
                        )
                        .overlay(alignment: .bottom) {
                            Rectangle().fill(selected ? Color.primary : .clear).frame(height: 1.5)
                        }
                        .contentShape(Rectangle())
                        .onTapGesture { tap(index) }
                        .accessibilityAddTraits(selected ? .isSelected : [])
                        .accessibilityHint("Selects this word; tap another word to extend")
                }
            }
            if workspace.selectedWordRange != nil {
                SelectionActions(
                    selectedText: selectedText,
                    compact: compact,
                    onHighlight: {
                        workspace.confirmation = .init(message: "Highlighted \"\(selectedText)\" with Emphasis", detail: "Undo · Change highlight")
                    },
                    onCollect: { onCollect?(selectedText, "\(pane.content.passage.split(separator: "–").first ?? ""), \"\(selectedText)\"") },
                    onStudy: { onStudy?() }
                )
            }
        }
    }

    private func tap(_ index: Int) {
        if let range = workspace.selectedWordRange, range.count == 1 || !range.contains(index) {
            workspace.selectedWordRange = min(range.lowerBound, index)...max(range.upperBound, index)
        } else {
            workspace.selectedWordRange = index...index
        }
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
                Button("Copy with reference", systemImage: "doc.on.doc") {}
                Button("Open in pane", systemImage: "rectangle.split.2x1") {}
                Button("Search for this", systemImage: "magnifyingglass") {}
                Button("Short observation…", systemImage: "text.bubble") {}
                Button("Compare renderings", systemImage: "rectangle.split.3x1") {}
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

/// A left-to-right wrapping flow for the tappable words.
struct WordFlow: Layout {
    var spacing: CGFloat = 4
    var lineSpacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let maxWidth = proposal.width ?? .infinity
        var x: CGFloat = 0, y: CGFloat = 0, lineHeight: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x + size.width > maxWidth, x > 0 {
                x = 0
                y += lineHeight + lineSpacing
                lineHeight = 0
            }
            x += size.width + spacing
            lineHeight = max(lineHeight, size.height)
        }
        return CGSize(width: maxWidth == .infinity ? x : maxWidth, height: y + lineHeight)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX, y = bounds.minY, lineHeight: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x + size.width > bounds.maxX, x > bounds.minX {
                x = bounds.minX
                y += lineHeight + lineSpacing
                lineHeight = 0
            }
            subview.place(at: CGPoint(x: x, y: y), proposal: .unspecified)
            x += size.width + spacing
            lineHeight = max(lineHeight, size.height)
        }
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
