// B072/B076: the floating tool well (specs/22 §4, §4.1): a 40 pt vertical well 12 pt from the
// leading edge over the columns. Its contents come from the core (`toolWell(context:)`) for
// what has focus: the context's tools, then the ink group (two overlapping swatch wells, X
// swaps, and the width dot) on Bible text, then the selection group while text is selected.
// The well's top stays put when the context or the selection changes; groups grow downward.
// A grip at the top drags it along the edge; a double-click on the grip (or Toggle tool
// palette) collapses it to the grip. The tool in effect is a filled button with an
// accessibility value; disabled items are dimmed with the reason in the tooltip.
import SwiftUI

struct ToolWell: View {
    @ObservedObject var model: ShellModel
    @State private var measuredHeight: CGFloat = 0

    /// The core's neutral icon names as SF Symbols (the web shell draws the same meanings as
    /// inline SVG; docs/evidence/B076-mac.md has the table). The ink items are drawn, not icons.
    static let symbols: [String: String] = [
        "select": "cursorarrow",
        "word-study": "character.book.closed",
        "highlight": "highlighter",
        "pen": "pencil.tip",
        "highlighter-pen": "paintbrush.pointed",
        "eraser": "eraser",
        "collect": "tray.and.arrow.down",
        "type": "character.cursor.ibeam",
        "bold": "bold",
        "italic": "italic",
        "heading": "textformat.size",
        "list": "list.bullet",
        "link-passage": "link",
        "highlight-now": "highlighter",
        "note": "note.text.badge.plus",
        "search": "magnifyingglass",
        "copy-reference": "doc.on.doc",
        "study": "text.book.closed",
    ]

    static func symbol(_ icon: String) -> String { symbols[icon] ?? "questionmark.square.dashed" }

    /// B072's well (six tools and the ink group): the height the well's top is placed by, so
    /// the top does not move when a context has more or fewer tools or a selection adds its
    /// group.
    static var referenceHeight: CGFloat {
        ShellTokens.toolGripSide + 6 * (ShellTokens.toolButtonSide + 2) + 1 + 2 * 8 + 44 + 28
    }

    var body: some View {
        GeometryReader { geometry in
            let height = model.toolWellCollapsed ? ShellTokens.toolGripSide : max(measuredHeight, ShellTokens.toolGripSide)
            let top = Self.top(position: model.toolWellPosition, in: geometry.size.height, height: height, collapsed: model.toolWellCollapsed)
            well
                .frame(width: model.toolWellWidth)
                .fixedSize(horizontal: false, vertical: true)
                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { measuredHeight = $0 }
                .padding(.leading, ShellTokens.toolWellInset)
                .padding(.top, top)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .environment(\.wellGeometryHeight, geometry.size.height)
        }
    }

    /// The well's top: where B072's well would start for this position, kept on screen.
    static func top(position: CGFloat, in available: CGFloat, height: CGFloat, collapsed: Bool) -> CGFloat {
        let reference = collapsed ? ShellTokens.toolGripSide : referenceHeight
        let wanted = position * available - reference / 2
        return max(8, min(wanted, available - height - 8))
    }

    private var well: some View {
        VStack(spacing: 2) {
            Grip(model: model)
            if !model.toolWellCollapsed {
                ForEach(model.well.groups, id: \.kind) { group in
                    WellGroup(model: model, group: group)
                }
            }
        }
        .padding(.bottom, model.toolWellCollapsed ? 0 : 6)
        .background(
            RoundedRectangle(cornerRadius: ShellTokens.Radius.toolWell)
                .fill(model.theme.wellColor)
                .shadow(color: .black.opacity(model.theme.isDark ? 0.45 : 0.12), radius: 6, y: 2)
        )
        .overlay(RoundedRectangle(cornerRadius: ShellTokens.Radius.toolWell).strokeBorder(model.theme.rulesColor, lineWidth: 1))
        // A context change swaps buttons in place; only the selection group fades, briefly.
        .animation(.easeOut(duration: 0.1), value: model.well.groups.map(\.kind))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Tools")
        .accessibilityValue(model.well.context)
        .accessibilityIdentifier("shell-tool-well")
    }
}

private struct WellHeightKey: EnvironmentKey { static let defaultValue: CGFloat = 600 }
extension EnvironmentValues {
    var wellGeometryHeight: CGFloat {
        get { self[WellHeightKey.self] }
        set { self[WellHeightKey.self] = newValue }
    }
}

/// One group: the tools at the top; the ink and selection groups each under a divider.
private struct WellGroup: View {
    @ObservedObject var model: ShellModel
    let group: ShellToolWellGroup

    var body: some View {
        VStack(spacing: 2) {
            if group.kind != "tools" {
                Rectangle().fill(model.theme.rulesColor).frame(width: 24, height: 1).padding(.vertical, 8)
            }
            if group.kind == "ink" {
                SwatchWells(model: model, items: group.items)
                if let width = group.items.first(where: { $0.commandId == "tool.width" }) {
                    WidthDot(model: model, item: width)
                }
            } else {
                ForEach(group.items, id: \.commandId) { item in
                    ToolButton(model: model, item: item, group: group.kind)
                }
            }
        }
        .transition(group.kind == "selection" ? .opacity : .identity)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Self.title(group.kind))
        .accessibilityIdentifier("shell-well-\(group.kind)")
    }

    static func title(_ kind: String) -> String {
        switch kind {
        case "ink": return "Ink"
        case "selection": return "Selection"
        default: return "Tools"
        }
    }
}

private struct Grip: View {
    @ObservedObject var model: ShellModel
    @Environment(\.wellGeometryHeight) private var height
    @State private var start: CGFloat?

    var body: some View {
        Image(systemName: "line.3.horizontal")
            .font(.system(size: 9, weight: .semibold))
            .foregroundStyle(model.theme.chromeColor)
            .frame(width: model.toolWellWidth, height: ShellTokens.toolGripSide)
            .contentShape(Rectangle())
            .onTapGesture(count: 2) { model.run("view.tool_palette") }
            .gesture(
                DragGesture(minimumDistance: 2, coordinateSpace: .global)
                    .onChanged { value in
                        let origin = start ?? model.toolWellPosition
                        if start == nil { start = origin }
                        model.toolWellPosition = max(0, min(1, origin + value.translation.height / max(1, height)))
                    }
                    .onEnded { _ in start = nil }
            )
            .help("Drag to move; double-click to \(model.toolWellCollapsed ? "expand" : "collapse")")
            .accessibilityLabel("Tool palette grip")
            .accessibilityValue(model.toolWellCollapsed ? "collapsed" : "expanded")
            .accessibilityAction(named: model.toolWellCollapsed ? "Expand" : "Collapse") { model.run("view.tool_palette") }
            .accessibilityIdentifier("shell-tool-grip")
    }
}

private struct ToolButton: View {
    @ObservedObject var model: ShellModel
    let item: ShellToolWellItem
    let group: String

    var body: some View {
        let current = item.active
        let disabled = item.disabledText != nil
        Button {
            model.runWellItem(item)
        } label: {
            Image(systemName: ToolWell.symbol(item.icon))
                .font(.system(size: 14, weight: current ? .semibold : .regular))
                .frame(width: ShellTokens.toolButtonSide, height: ShellTokens.toolButtonSide)
                .foregroundStyle(current ? model.theme.pageColor : model.theme.textColor)
                .background(
                    RoundedRectangle(cornerRadius: ShellTokens.Radius.control)
                        .fill(current ? model.theme.accentColor : .clear)
                )
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .opacity(disabled ? 0.4 : 1)
        .disabled(disabled)
        .help(ToolWell.tooltip(item))
        .accessibilityLabel(item.label)
        .accessibilityValue(current ? "current tool" : (item.disabledText ?? ""))
        .accessibilityAddTraits(current ? .isSelected : [])
        .accessibilityIdentifier("shell-\(group == "selection" ? "action" : "tool")-\(item.icon)")
    }
}

extension ToolWell {
    /// `Bold (⌘B)`, `Heading`, `Pen (P) — Pen works on Bible text`.
    static func tooltip(_ item: ShellToolWellItem) -> String {
        let key = item.key.map { " (\(ShellModel.symbols($0)))" } ?? ""
        return "\(item.label)\(key)\(item.disabledText.map { " — \($0)" } ?? "")"
    }
}

private struct SwatchWells: View {
    @ObservedObject var model: ShellModel
    let items: [ShellToolWellItem]

    var body: some View {
        let current = ShellTokens.swatch(hex: items.first { $0.commandId == "tool.swatch" }?.value ?? model.session.ink.swatch)
        let alternate = ShellTokens.swatch(hex: items.first { $0.commandId == "tool.swap_swatches" }?.value ?? model.session.ink.alternateSwatch)
        ZStack(alignment: .topLeading) {
            RoundedRectangle(cornerRadius: 4)
                .fill(alternate.solid)
                .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(model.theme.pageColor, lineWidth: 1.5))
                .frame(width: 18, height: 18)
                .offset(x: 10, y: 10)
            RoundedRectangle(cornerRadius: 4)
                .fill(current.solid)
                .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(model.theme.pageColor, lineWidth: 1.5))
                .frame(width: 18, height: 18)
        }
        .frame(width: 28, height: 28, alignment: .topLeading)
        .padding(.vertical, 4)
        .contentShape(Rectangle())
        .onTapGesture { model.run("tool.swap_swatches") }
        .contextMenu {
            // Swatch takes its colour as an argument: the host offers the tokens' four.
            ForEach(ShellTokens.swatches, id: \.id) { swatch in
                Button(swatch.title) { model.run("tool.swatch", args: ShellCommandArgs(text: ShellTokens.hex(swatch.rgb))) }
            }
        }
        .help("Swatches: \(current.title), alternate \(alternate.title) (X swaps)")
        .accessibilityElement()
        .accessibilityLabel("Swatches")
        .accessibilityValue("\(current.title), alternate \(alternate.title)")
        .accessibilityAddTraits(.isButton)
        .accessibilityHint("Swaps the swatches")
        .accessibilityIdentifier("shell-swatches")
    }
}

private struct WidthDot: View {
    @ObservedObject var model: ShellModel
    let item: ShellToolWellItem

    var body: some View {
        let width = item.value.flatMap(Double.init) ?? model.inkWidth
        Circle()
            .fill(model.theme.textColor)
            .frame(width: CGFloat(width) + 2, height: CGFloat(width) + 2)
            .frame(width: 28, height: 22)
            .contentShape(Rectangle())
            .onTapGesture { model.cycleInkWidth() }
            .help("\(item.label) \(Int(width)) pt")
            .accessibilityElement()
            .accessibilityLabel(item.label)
            .accessibilityValue("\(Int(width)) points")
            .accessibilityAddTraits(.isButton)
            .accessibilityIdentifier("shell-width")
    }
}
