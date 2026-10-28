// The prototype's root: a debug bar (view, layout A/B, state, simulated width, metrics) above a
// simulated viewport, so every view can be rendered at the four measurement widths on the one
// iPad simulator this machine boots. A viewport that does not fit the window is rotated (a
// 1024 × 768 landscape frame fits a portrait iPad) or scaled down, and the debug bar says so; the
// layout engine still lays out at the true size, so the measured numbers are unaffected.
import SwiftUI

struct PrototypeRoot: View {
    @StateObject private var options = PrototypeOptions()

    var body: some View {
        GeometryReader { geometry in
            VStack(spacing: 0) {
                if options.showDebugBar {
                    DebugBar(window: geometry.size)
                }
                SimulatedViewport(window: geometry.size)
            }
        }
        .environmentObject(options)
    }
}

/// The scenario at the chosen size, rebuilt when the view or state changes.
private struct SimulatedViewport: View {
    @EnvironmentObject var options: PrototypeOptions
    let window: CGSize

    var body: some View {
        let available = CGSize(width: window.width, height: window.height - (options.showDebugBar ? DebugBar.height : 0))
        let size = options.simulatedSize ?? available
        let fits = size.width <= available.width && size.height <= available.height
        let fitsRotated = size.height <= available.width && size.width <= available.height
        let rotate = !fits && fitsRotated
        let scale: CGFloat = fits || rotate ? 1 : min(available.width / size.width, available.height / size.height)

        ScenarioView(size: size)
            .id("\(options.view.rawValue)-\(options.state.rawValue)-\(Int(size.width))")
            .frame(width: size.width, height: size.height)
            .environment(\.horizontalSizeClass, WidthClass(width: size.width) == .compact ? .compact : .regular)
            .clipped()
            .overlay(Rectangle().strokeBorder(options.simulatedSize == nil ? .clear : .secondary, lineWidth: 1))
            .rotationEffect(rotate ? .degrees(-90) : .zero)
            .scaleEffect(scale)
            .frame(width: rotate ? size.height : size.width * scale, height: rotate ? size.width : size.height * scale)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            .background(Color.secondary.opacity(options.simulatedSize == nil ? 0 : 0.12))
    }
}

/// Builds the workspace for the selected view and state, once per combination.
private struct ScenarioView: View {
    @EnvironmentObject var options: PrototypeOptions
    let size: CGSize
    @StateObject private var holder = WorkspaceHolder()

    var body: some View {
        Group {
            if options.view == .shell {
                ReadingShellView()
            } else {
                WorkspaceView(workspace: holder.workspace(for: options.view, state: options.state), size: size)
            }
        }
    }
}

@MainActor
private final class WorkspaceHolder: ObservableObject {
    private var key = ""
    private var cached: Workspace?

    func workspace(for view: PrototypeView, state: PrototypeState) -> Workspace {
        let wanted = "\(view.rawValue)-\(state.rawValue)"
        if let cached, key == wanted { return cached }
        let made = Workspace.make(view: view, state: state)
        key = wanted
        cached = made
        return made
    }
}

/// Not part of any layout: the switches the evidence screenshots were taken with.
struct DebugBar: View {
    @EnvironmentObject var options: PrototypeOptions
    @Environment(\.dynamicTypeSize) private var typeSize
    let window: CGSize
    static let height: CGFloat = 44

    var body: some View {
        HStack(spacing: 8) {
            Picker("View", selection: $options.view) {
                ForEach(PrototypeView.allCases) { Text($0.title).tag($0) }
            }
            .pickerStyle(.menu)
            Picker("Layout", selection: $options.layout) {
                ForEach(LayoutVariant.allCases) { Text("Layout \($0.rawValue)").tag($0) }
            }
            .pickerStyle(.segmented)
            .frame(maxWidth: 180)
            Picker("State", selection: $options.state) {
                ForEach(PrototypeState.allCases) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.menu)
            Picker("Width", selection: Binding(
                get: { options.simulatedSize.map { Int($0.width) } ?? 0 },
                set: { width in
                    options.simulatedSize = width == 0 ? nil
                        : PrototypeOptions.measurementSizes.first { Int($0.width) == width }
                        ?? CGSize(width: CGFloat(width), height: PrototypeOptions.defaultHeight(forWidth: Double(width)))
                }
            )) {
                Text("Window").tag(0)
                ForEach(PrototypeOptions.measurementSizes, id: \.width) { Text("\(Int($0.width))×\(Int($0.height))").tag(Int($0.width)) }
            }
            .pickerStyle(.menu)
            Toggle("Metrics", isOn: $options.showMetrics).toggleStyle(.button)
            Spacer(minLength: 0)
            Text(caption).font(.caption2).foregroundStyle(.secondary).lineLimit(2)
        }
        .font(.caption)
        .padding(.horizontal, 8)
        .frame(height: Self.height)
        .background(Color.secondary.opacity(0.15))
        .overlay(alignment: .bottom) { Divider() }
        .accessibilityLabel("Prototype debug bar")
    }

    private var caption: String {
        let size = options.simulatedSize.map { "\(Int($0.width))×\(Int($0.height)) pt" } ?? "window \(Int(window.width))×\(Int(window.height - Self.height)) pt"
        return "B004 fixture prototype · \(size) · type \(typeSize.isAccessibilitySize ? "accessibility" : "default")"
    }
}

#Preview {
    PrototypeRoot()
}
