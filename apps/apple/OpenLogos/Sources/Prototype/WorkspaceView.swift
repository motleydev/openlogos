// Composes a scenario at a viewport: chrome for layout A or B, the visible panes for the width
// class (three, two plus a tab, one plus tabs), drawers as a trailing column or a sheet, the
// destination views, focus mode, and the measurement overlay (specs/21 §1, §8, §10).
import SwiftUI

struct WorkspaceView: View {
    @ObservedObject var workspace: Workspace
    @EnvironmentObject var options: PrototypeOptions
    let size: CGSize
    @State private var rendered: [String: CGSize] = [:]

    private var layout: LayoutVariant { options.layout }
    private var widthClass: WidthClass { WidthClass(width: size.width) }
    private var compact: Bool { widthClass == .compact }

    /// A transient (unpinned) drawer in regular or medium width takes the last visible pane's
    /// column; that pane is named in the drawer header and listed in the tab row (specs/21 §5).
    private var drawerCoversPane: Bool { !compact && workspace.drawer != .none && !workspace.drawerPinned && !isDestination }

    /// The panes on screen: the active one in compact width, the first n otherwise, minus the
    /// one a transient drawer covers.
    private var visiblePanes: [Pane] {
        var count = min(workspace.panes.count, widthClass.visiblePanes)
        if drawerCoversPane { count = max(count - 1, 1) }
        if workspace.panes.count <= count { return Array(workspace.panes) }
        if widthClass == .regular && !drawerCoversPane { return Array(workspace.panes.prefix(count)) }
        // Fewer slots than panes: a window of `count` panes that contains the active one.
        let active = workspace.compactIndex
        let start = min(active, workspace.panes.count - count)
        return Array(workspace.panes[start..<(start + count)])
    }

    private var coveredPaneName: String? {
        guard drawerCoversPane else { return nil }
        let shown = Set(visiblePanes.map(\.id))
        return workspace.panes.first { !shown.contains($0.id) }?.content.shortName
    }

    private var isDestination: Bool { [.search, .margin, .handoff].contains(options.view) }

    var body: some View {
        ZStack(alignment: .bottom) {
            if options.focusMode {
                focusBody
            } else if layout == .a {
                VStack(spacing: 0) {
                    SharedBar(workspace: workspace, width: size.width)
                    panesAndDrawer
                }
            } else {
                if compact {
                    VStack(spacing: 0) {
                        panesAndDrawer
                        EdgeLauncher(workspace: workspace, width: size.width)
                    }
                } else {
                    HStack(spacing: 0) {
                        EdgeLauncher(workspace: workspace, width: size.width)
                        panesAndDrawer
                    }
                }
            }
            ConfirmationLine(workspace: workspace)
                .padding(.bottom, compact && layout == .b ? ChromeMetrics.launcherStrip + 8 : 8)
                .animation(.default, value: workspace.confirmation)
        }
        .overlay(alignment: .bottomLeading) {
            if options.showMetrics && !isDestination {
                MetricsOverlay(size: size, layout: layout, panes: workspace.panes.count, rendered: rendered)
            }
        }
        .onPreferenceChange(ReadingAreaKey.self) { rendered = $0 }
        .background(.background)
    }

    // MARK: Panes

    @ViewBuilder private var panesAndDrawer: some View {
        if isDestination {
            destination
        } else {
            VStack(spacing: 0) {
                if visiblePanes.count < workspace.panes.count {
                    PaneTabs(workspace: workspace, visible: Set(visiblePanes.map(\.id)))
                }
                HStack(spacing: 0) {
                    ForEach(Array(visiblePanes.enumerated()), id: \.element.id) { index, pane in
                        if index > 0 { Divider() }
                        paneColumn(pane)
                    }
                    if !compact, workspace.drawer != .none {
                        Divider()
                        drawerContent(covering: workspace.drawerPinned ? nil : coveredPaneName)
                            .frame(width: drawerWidth)
                    }
                }
                // Compact width: the drawer is a sheet at medium height over the pane, drawn
                // inside the viewport so the launcher strip and the selection stay reachable.
                .overlay(alignment: .bottom) {
                    if compact, workspace.drawer != .none {
                        CompactSheet { drawerContent(covering: nil) }
                            .frame(height: size.height * 0.55)
                            .transition(.move(edge: .bottom))
                    }
                }
            }
        }
    }

    /// A transient drawer is as wide as the pane column it took; a pinned one is a narrower
    /// column beside all the panes.
    private var drawerWidth: CGFloat {
        let side = layout == .b && !compact ? ChromeMetrics.launcherWidth : 0
        let slots = CGFloat(min(workspace.panes.count, widthClass.visiblePanes))
        let paneWidth = (size.width - side) / slots
        return workspace.drawerPinned ? min(ChromeMetrics.drawerWidth, paneWidth) : paneWidth
    }

    private func paneColumn(_ pane: Pane) -> some View {
        VStack(spacing: 0) {
            PaneIdentityRow(workspace: workspace, pane: pane, layout: layout, isActive: pane.id == workspace.activePaneID, width: size.width)
            PaneTextView(
                workspace: workspace, pane: pane, state: options.state, width: size.width,
                selectable: options.view == .selection && pane.id == "p1",
                onCollect: { text, locator in workspace.collect(text: text, resource: pane.content, locator: locator) },
                onStudy: { workspace.drawer = .study }
            )
        }
        .frame(maxWidth: .infinity)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Pane \(pane.content.shortName)")
    }

    @ViewBuilder private func drawerContent(covering: String?) -> some View {
        switch workspace.drawer {
        case .study: StudyDrawer(workspace: workspace, state: options.state, covering: covering)
        case .collection: CollectionDrawer(workspace: workspace, state: options.state, covering: covering)
        case .notes: NotesDock(workspace: workspace, state: options.state, covering: covering)
        case .none: EmptyView()
        }
    }

    @ViewBuilder private var destination: some View {
        switch options.view {
        case .search: SearchResultsView(workspace: workspace, state: options.state, width: size.width)
        case .margin: MarginCaptureView(workspace: workspace, state: options.state, width: size.width)
        case .handoff: HandoffView(workspace: workspace, state: options.state, width: size.width)
        default: EmptyView()
        }
    }

    // MARK: Focus mode

    /// Chrome hidden, the active pane centred at the prose measure, one visible way out.
    private var focusBody: some View {
        ZStack(alignment: .topTrailing) {
            if let pane = workspace.activePane {
                PaneTextView(workspace: workspace, pane: pane, state: options.state, width: size.width)
                    .frame(maxWidth: 680)
                    .frame(maxWidth: .infinity)
            }
            Button {
                options.focusMode = false
            } label: {
                Label("Exit focus", systemImage: "arrow.down.right.and.arrow.up.left")
                    .font(.footnote)
                    .padding(.horizontal, 10)
                    .frame(minHeight: ChromeMetrics.minimumTarget)
                    .background(Capsule().fill(.thickMaterial))
                    .contentShape(Capsule())
            }
            .buttonStyle(.plain)
            .keyboardShortcut(.escape, modifiers: [])
            .padding(12)
            .accessibilityHint("Restores the bar, pane labels and dock exactly as they were")
        }
    }
}

/// A medium-detent sheet drawn inside the viewport (compact width). The grab handle is also a
/// 44-point close target; the production reader uses the system sheet with the same detents.
struct CompactSheet<Content: View>: View {
    @ViewBuilder let content: () -> Content

    var body: some View {
        VStack(spacing: 0) {
            Capsule().fill(.secondary).frame(width: 36, height: 5).padding(.vertical, 6)
                .frame(maxWidth: .infinity, minHeight: 20)
                .accessibilityLabel("Sheet handle")
            content()
        }
        .background(.background)
        .clipShape(UnevenRoundedRectangle(topLeadingRadius: 12, topTrailingRadius: 12))
        .shadow(color: .black.opacity(0.18), radius: 10, y: -2)
        .accessibilityAddTraits(.isModal)
    }
}
