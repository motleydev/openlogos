// B072: the floating tool well (specs/22 §4): a 40 pt vertical well 12 pt from the leading
// edge, vertically centred over the columns, with Select, Highlight, Pen, Highlighter pen,
// Eraser and Collect, two overlapping swatch wells (X swaps) and the ink width dot. A grip at
// the top drags it along the edge; a double-click on the grip (or Toggle tool palette)
// collapses it to the grip. The current tool is a filled button with an accessibility value.
import SwiftUI

struct ToolWell: View {
    @ObservedObject var model: ShellModel
    @State private var dragStart: CGFloat?

    struct Tool: Identifiable {
        let id: String
        let command: String
        let symbol: String
        let name: String
        let key: String
    }

    static let tools: [Tool] = [
        Tool(id: "select", command: "tool.select", symbol: "cursorarrow", name: "Select", key: "V"),
        Tool(id: "highlight", command: "tool.highlight", symbol: "highlighter", name: "Highlight", key: "H"),
        Tool(id: "pen", command: "tool.pen", symbol: "pencil.tip", name: "Pen", key: "P"),
        Tool(id: "highlighter_pen", command: "tool.highlighter_pen", symbol: "paintbrush.pointed", name: "Highlighter pen", key: "M"),
        Tool(id: "eraser", command: "tool.eraser", symbol: "eraser", name: "Eraser", key: "E"),
        Tool(id: "collect", command: "tool.collect", symbol: "tray.and.arrow.down", name: "Collect", key: "C"),
    ]

    var body: some View {
        GeometryReader { geometry in
            let height = wellHeight
            let centre = model.toolWellPosition * geometry.size.height
            let y = max(height / 2 + 8, min(geometry.size.height - height / 2 - 8, centre))
            well
                .frame(width: model.toolWellWidth)
                .position(x: ShellTokens.toolWellInset + model.toolWellWidth / 2, y: y)
                .environment(\.wellGeometryHeight, geometry.size.height)
        }
    }

    private var wellHeight: CGFloat {
        model.toolWellCollapsed ? 22 : 22 + CGFloat(Self.tools.count) * (ShellTokens.toolButtonSide + 2) + 1 + 2 * 8 + 44 + 28
    }

    private var well: some View {
        VStack(spacing: 2) {
            Grip(model: model)
            if !model.toolWellCollapsed {
                ForEach(Self.tools) { tool in
                    ToolButton(model: model, tool: tool)
                }
                Rectangle().fill(model.theme.rulesColor).frame(width: 24, height: 1).padding(.vertical, 8)
                SwatchWells(model: model)
                WidthDot(model: model)
            }
        }
        .padding(.bottom, model.toolWellCollapsed ? 0 : 6)
        .background(
            RoundedRectangle(cornerRadius: ShellTokens.Radius.toolWell)
                .fill(model.theme.wellColor)
                .shadow(color: .black.opacity(model.theme.isDark ? 0.45 : 0.12), radius: 6, y: 2)
        )
        .overlay(RoundedRectangle(cornerRadius: ShellTokens.Radius.toolWell).strokeBorder(model.theme.rulesColor, lineWidth: 1))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Tools")
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
    let tool: ToolWell.Tool

    var body: some View {
        let current = model.tool == tool.id
        Button {
            model.run(tool.command)
        } label: {
            Image(systemName: tool.symbol)
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
        .help("\(tool.name) (\(tool.key))")
        .accessibilityLabel(tool.name)
        .accessibilityValue(current ? "current tool" : "")
        .accessibilityAddTraits(current ? .isSelected : [])
        .accessibilityIdentifier("shell-tool-\(tool.id)")
    }
}

private struct SwatchWells: View {
    @ObservedObject var model: ShellModel

    var body: some View {
        ZStack(alignment: .topLeading) {
            RoundedRectangle(cornerRadius: 4)
                .fill(model.alternateSwatch.solid)
                .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(model.theme.pageColor, lineWidth: 1.5))
                .frame(width: 18, height: 18)
                .offset(x: 10, y: 10)
            RoundedRectangle(cornerRadius: 4)
                .fill(model.swatch.solid)
                .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(model.theme.pageColor, lineWidth: 1.5))
                .frame(width: 18, height: 18)
        }
        .frame(width: 28, height: 28, alignment: .topLeading)
        .padding(.vertical, 4)
        .contentShape(Rectangle())
        .onTapGesture { model.run("tool.swap_swatches") }
        .help("Swatches: \(model.swatch.title), alternate \(model.alternateSwatch.title) (X swaps)")
        .accessibilityElement()
        .accessibilityLabel("Swatches")
        .accessibilityValue("\(model.swatch.title), alternate \(model.alternateSwatch.title)")
        .accessibilityAddTraits(.isButton)
        .accessibilityHint("Swaps the swatches")
        .accessibilityIdentifier("shell-swatches")
    }
}

private struct WidthDot: View {
    @ObservedObject var model: ShellModel

    var body: some View {
        Circle()
            .fill(model.theme.textColor)
            .frame(width: CGFloat(model.inkWidth) + 2, height: CGFloat(model.inkWidth) + 2)
            .frame(width: 28, height: 22)
            .contentShape(Rectangle())
            .onTapGesture { model.cycleInkWidth() }
            .help("Ink width \(Int(model.inkWidth)) pt")
            .accessibilityElement()
            .accessibilityLabel("Ink width")
            .accessibilityValue("\(Int(model.inkWidth)) points")
            .accessibilityAddTraits(.isButton)
            .accessibilityIdentifier("shell-width")
    }
}
