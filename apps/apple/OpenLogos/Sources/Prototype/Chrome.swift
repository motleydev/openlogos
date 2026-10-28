// The chrome of layouts A and B (specs/21 §10, specs/16 "Hierarchy and navigation"): one shared
// bar or one edge launcher, and one identity row per pane. Heights are the design budget at
// normal type and grow with Dynamic Type; they are the numbers LayoutMetrics uses, so the
// measurement table and the rendered views agree by construction.
import SwiftUI

enum ChromeMetrics {
    static let sharedBar: CGFloat = 44 // layout A, pointer and touch alike
    // specs/16 budgets 32–40 px for a pointer desktop; on touch the row's controls need 44-point
    // targets, so on iPadOS the row is 44 (the first render showed 36 as designed and 44 as
    // rendered; the constant now says what the layout does).
    static let identityRowA: CGFloat = 44 // layout A inline pane label (touch); 36 on pointer desktops
    static let headerB: CGFloat = 44 // layout B explicit local header
    static let passageRowB: CGFloat = 44 // layout B: the active pane's passage field row when its header is too narrow
    static let passageInlineMinimumPaneWidth: CGFloat = 560 // below this, B's passage field takes its own row
    static let launcherWidth: CGFloat = 44 // layout B leading edge, regular widths
    static let launcherStrip: CGFloat = 44 // layout B bottom strip, compact width
    static let tabsRow: CGFloat = 36 // compact and medium: the panes that are not visible
    static let divider: CGFloat = 1
    static let panePaddingRegular: CGFloat = 20
    static let panePaddingCompact: CGFloat = 16
    static let drawerWidth: CGFloat = 360
    static let minimumTarget: CGFloat = 44
    static let accessibilityPaneMinimum: CGFloat = 480 // a pane at accessibility type sizes

    static func panePadding(_ width: CGFloat) -> CGFloat {
        WidthClass(width: width) == .compact ? panePaddingCompact : panePaddingRegular
    }
}

/// A control with the 44-point target the product requires (specs/16), whatever its glyph size.
struct ChromeButton: View {
    let title: String
    let symbol: String
    var showsTitle = false
    var isOn = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Image(systemName: symbol)
                if showsTitle { Text(title).lineLimit(1) }
            }
            .font(.system(.body, design: .default))
            .fontWeight(isOn ? .semibold : .regular)
            .frame(minWidth: ChromeMetrics.minimumTarget, minHeight: ChromeMetrics.minimumTarget)
            .padding(.horizontal, showsTitle ? 6 : 0)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .foregroundStyle(isOn ? .primary : .secondary)
        .accessibilityLabel(title)
        .accessibilityAddTraits(isOn ? .isSelected : [])
    }
}

/// The current collection's destination and count, shown once (specs/15 "Study collections").
struct DestinationIndicator: View {
    @ObservedObject var workspace: Workspace
    var compact = false

    var body: some View {
        Button {
            workspace.toggle(.collection)
        } label: {
            HStack(spacing: 6) {
                Image(systemName: "tray.full")
                if !compact {
                    Text(workspace.collectionName).lineLimit(1)
                }
                Text("\(workspace.collectionItems.count)")
                    .monospacedDigit()
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(Capsule().strokeBorder(.secondary, lineWidth: 1))
            }
            .font(.body)
            .frame(minHeight: ChromeMetrics.minimumTarget)
            .padding(.horizontal, 8)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .foregroundStyle(workspace.drawer == .collection ? .primary : .secondary)
        .accessibilityLabel("Collection \(workspace.collectionName), \(workspace.collectionItems.count) items")
        .accessibilityHint("Opens the collection drawer")
    }
}

/// The passage field: one per workspace in A, in the active pane's header in B.
struct PassageField: View {
    @Binding var text: String
    var compact = false

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "book").foregroundStyle(.secondary)
            TextField("Passage or search", text: $text)
                .textFieldStyle(.plain)
                .font(.body)
                .autocorrectionDisabled()
                #if os(iOS)
                .textInputAutocapitalization(.never)
                #endif
                .accessibilityLabel("Passage")
                .frame(minHeight: ChromeMetrics.minimumTarget)
        }
        .padding(.horizontal, 10)
        .frame(maxWidth: compact ? .infinity : 360)
        .background(RoundedRectangle(cornerRadius: 8).fill(.quaternary.opacity(0.5)))
    }
}

// MARK: Layout A

/// Layout A's one shared bar: workspace menu, passage field, search, collection indicator,
/// overflow. At accessibility sizes it becomes two rows so nothing clips (specs/21 §7).
struct SharedBar: View {
    @ObservedObject var workspace: Workspace
    @EnvironmentObject var options: PrototypeOptions
    @Environment(\.dynamicTypeSize) private var typeSize
    let width: CGFloat

    private var compact: Bool { WidthClass(width: width) == .compact }

    var body: some View {
        Group {
            if typeSize.isAccessibilitySize {
                VStack(spacing: 0) {
                    HStack(spacing: 4) {
                        workspaceMenu
                        Spacer(minLength: 0)
                        searchButton
                        DestinationIndicator(workspace: workspace, compact: true)
                        overflow
                    }
                    PassageField(text: $workspace.passageFieldText, compact: true)
                        .padding(.horizontal, 8)
                        .padding(.bottom, 6)
                }
            } else {
                HStack(spacing: 4) {
                    workspaceMenu
                    PassageField(text: $workspace.passageFieldText, compact: compact)
                    if !compact { Spacer(minLength: 8) }
                    searchButton
                    DestinationIndicator(workspace: workspace, compact: compact)
                    overflow
                }
                .frame(height: ChromeMetrics.sharedBar)
            }
        }
        .padding(.horizontal, 4)
        .background(.bar)
        .overlay(alignment: .bottom) { Divider() }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Workspace bar")
    }

    private var workspaceMenu: some View {
        Menu {
            Section("Workspace") {
                Button("Read", systemImage: "book") { options.view = .read }
                Button("Search", systemImage: "magnifyingglass") { options.view = .search }
                Button("Library", systemImage: "books.vertical") {}
                Button("Notes", systemImage: "note.text") { workspace.toggle(.notes) }
                Button("Collections", systemImage: "tray.full") { options.view = .collection }
            }
            Section("Layouts") {
                Button("Parallel reading", systemImage: "rectangle.split.3x1") { options.view = .three }
                Button("Study and collect", systemImage: "rectangle.split.3x1.fill") { options.view = .collection }
                Button("Margin mode", systemImage: "pencil.and.outline") { options.view = .margin }
            }
            Section {
                Button("Focus mode", systemImage: "arrow.up.left.and.arrow.down.right") { options.focusMode = true }
                    .keyboardShortcut("f", modifiers: [.command, .shift])
                Button("Restore closed pane", systemImage: "arrow.uturn.backward") {}
            }
        } label: {
            HStack(spacing: 4) {
                Image(systemName: "line.3.horizontal")
                if !compact { Text("Read").fontWeight(.semibold) }
            }
            .font(.body)
            .frame(minWidth: ChromeMetrics.minimumTarget, minHeight: ChromeMetrics.minimumTarget)
            .padding(.horizontal, compact ? 0 : 6)
            .contentShape(Rectangle())
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .accessibilityLabel("Workspace menu, Read")
    }

    private var searchButton: some View {
        ChromeButton(title: "Search", symbol: "magnifyingglass", isOn: options.view == .search) {
            options.view = .search
        }
        .keyboardShortcut("f", modifiers: .command)
    }

    private var overflow: some View {
        Menu {
            Button("Appearance", systemImage: "circle.lefthalf.filled") {}
            Button("Typography", systemImage: "textformat.size") {}
            Button("Command launcher ⌘K", systemImage: "command") {}
            Button("Hand off to Obsidian", systemImage: "square.and.arrow.up") { options.view = .handoff }
        } label: {
            Image(systemName: "ellipsis.circle")
                .font(.body)
                .frame(minWidth: ChromeMetrics.minimumTarget, minHeight: ChromeMetrics.minimumTarget)
                .contentShape(Rectangle())
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .foregroundStyle(.secondary)
        .accessibilityLabel("More")
    }
}

// MARK: Layout B

/// Layout B's edge launcher: the same global actions as A's bar, as a leading column in regular
/// widths and a bottom strip in compact width. Labels appear on long press or hover (and always
/// in the compact strip under each icon) so icons are not the only route (specs/16).
struct EdgeLauncher: View {
    @ObservedObject var workspace: Workspace
    @EnvironmentObject var options: PrototypeOptions
    let width: CGFloat

    private var compact: Bool { WidthClass(width: width) == .compact }

    var body: some View {
        Group {
            if compact {
                HStack(spacing: 0) {
                    ForEach(items, id: \.title) { item in
                        launcherButton(item, label: true).frame(maxWidth: .infinity)
                    }
                }
                .padding(.bottom, 2)
                // 44 at normal type; the labels grow with Dynamic Type and the strip with them.
                .frame(minHeight: ChromeMetrics.launcherStrip)
                .background(.bar)
                .overlay(alignment: .top) { Divider() }
            } else {
                VStack(spacing: 2) {
                    ForEach(items, id: \.title) { item in
                        launcherButton(item, label: false)
                    }
                    Spacer()
                }
                .padding(.top, 4)
                .frame(width: ChromeMetrics.launcherWidth)
                .background(.bar)
                .overlay(alignment: .trailing) { Divider() }
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Launcher")
    }

    private struct Item { let title: String; let symbol: String; let action: () -> Void; let isOn: Bool }

    private var items: [Item] {
        [
            Item(title: "Read", symbol: "book", action: { options.view = .read }, isOn: options.view == .read || options.view == .three),
            Item(title: "Search", symbol: "magnifyingglass", action: { options.view = .search }, isOn: options.view == .search),
            Item(title: "Collect \(workspace.collectionItems.count)", symbol: "tray.full", action: { workspace.toggle(.collection) }, isOn: workspace.drawer == .collection),
            Item(title: "Notes", symbol: "note.text", action: { workspace.toggle(.notes) }, isOn: workspace.drawer == .notes),
            Item(title: "Layouts", symbol: "rectangle.split.3x1", action: { options.view = .three }, isOn: false),
            Item(title: "More", symbol: "ellipsis.circle", action: { options.view = .handoff }, isOn: false),
        ]
    }

    private func launcherButton(_ item: Item, label: Bool) -> some View {
        Button(action: item.action) {
            VStack(spacing: 2) {
                Image(systemName: item.symbol).font(.body)
                if label {
                    Text(item.title).font(.caption2).lineLimit(1).minimumScaleFactor(0.8)
                }
            }
            .frame(minWidth: ChromeMetrics.minimumTarget, minHeight: ChromeMetrics.minimumTarget)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .foregroundStyle(item.isOn ? .primary : .secondary)
        .accessibilityLabel(item.title)
        .help(item.title)
    }
}

// MARK: Pane identity

/// One row per pane: resource, local passage, link state, menu. A: 36 points, inline label.
/// B: 44 points, explicit header, and the passage field when the pane is active.
struct PaneIdentityRow: View {
    @ObservedObject var workspace: Workspace
    let pane: Pane
    let layout: LayoutVariant
    let isActive: Bool
    /// The pane's own width, which decides whether B's passage field fits beside the identity.
    var paneWidth: CGFloat = 1024
    @Environment(\.dynamicTypeSize) private var typeSize

    private var height: CGFloat { layout == .a ? ChromeMetrics.identityRowA : ChromeMetrics.headerB }
    /// Layout B in a narrow pane or at accessibility type: the active pane's passage field gets
    /// its own row under the identity row rather than crowding it (counted in LayoutMetrics).
    private var passageOnOwnRow: Bool {
        layout == .b && isActive && (paneWidth < ChromeMetrics.passageInlineMinimumPaneWidth || typeSize.isAccessibilitySize)
    }

    var body: some View {
        VStack(spacing: 0) {
            identity
            if passageOnOwnRow {
                PassageField(text: $workspace.passageFieldText, compact: true)
                    .padding(.horizontal, 8)
                    .frame(height: ChromeMetrics.passageRowB)
                    .background(.bar)
            }
        }
        .overlay(alignment: .bottom) {
            // The active pane is marked by weight and this underline, never by color alone.
            Rectangle().fill(isActive ? Color.accentColor : .clear).frame(height: 2)
        }
        .accessibilityElement(children: .contain)
    }

    private var identity: some View {
        HStack(spacing: 8) {
            Button {
                workspace.activate(pane.id)
            } label: {
                HStack(spacing: 6) {
                    Text(pane.content.shortName)
                        .fontWeight(isActive ? .semibold : .regular)
                        .lineLimit(1)
                    Text(pane.content.passage)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.tail)
                }
                .font(layout == .a ? .subheadline : .body)
                .frame(minHeight: ChromeMetrics.minimumTarget)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("\(pane.content.fullTitle), \(pane.content.passage)")
            .accessibilityValue(isActive ? "active" : "")
            .accessibilityAddTraits(isActive ? .isSelected : [])

            if layout == .b, isActive, !passageOnOwnRow {
                PassageField(text: $workspace.passageFieldText)
                    .frame(maxWidth: 220)
            }
            Spacer(minLength: 4)
            LinkStateControl(workspace: workspace, pane: pane)
            paneMenu
        }
        .padding(.leading, 12)
        .padding(.trailing, 4)
        .frame(minHeight: height)
        .background(layout == .b ? AnyShapeStyle(.bar) : AnyShapeStyle(.clear))
    }

    private var paneMenu: some View {
        Menu {
            Button("Open in new pane", systemImage: "rectangle.split.2x1") {}
            Button("Replace resource…", systemImage: "arrow.triangle.2.circlepath") {}
            Button("Typography", systemImage: "textformat.size") {}
            Button("Resource history", systemImage: "clock") {}
            Button("Focus this pane", systemImage: "arrow.up.left.and.arrow.down.right") {}
            Divider()
            Button("Close pane", systemImage: "xmark", role: .destructive) {}
        } label: {
            Image(systemName: "ellipsis")
                .font(.body)
                .frame(minWidth: ChromeMetrics.minimumTarget, minHeight: ChromeMetrics.minimumTarget)
                .contentShape(Rectangle())
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .foregroundStyle(.secondary)
        .accessibilityLabel("Pane menu, \(pane.content.shortName)")
    }
}

/// Word plus glyph for the link state; a menu changes it (specs/21 §2).
struct LinkStateControl: View {
    @ObservedObject var workspace: Workspace
    let pane: Pane
    @Environment(\.dynamicTypeSize) private var typeSize

    var body: some View {
        Menu {
            Button("Pinned", systemImage: "pin") { workspace.setLink(.pinned, for: pane.id) }
            Button("Group 1", systemImage: "link") { workspace.setLink(.group(1), for: pane.id) }
            Button("Group 2", systemImage: "link") { workspace.setLink(.group(2), for: pane.id) }
            if pane.content.kind == .commentary {
                Text("No passage index: cannot join a group")
            }
            Button("Follows active passage", systemImage: "arrow.turn.down.right") { workspace.setLink(.follows, for: pane.id) }
        } label: {
            HStack(spacing: 4) {
                Image(systemName: pane.link.symbol)
                // At accessibility sizes the word yields to the resource name; the glyph and
                // the accessibility label keep the state explicit.
                if !typeSize.isAccessibilitySize { Text(pane.link.label) }
            }
            .font(.caption)
            .lineLimit(1)
            .fixedSize()
            .padding(.horizontal, 8)
            .frame(minHeight: 28)
            .background(Capsule().strokeBorder(.secondary.opacity(0.6), lineWidth: 1))
            .frame(minHeight: ChromeMetrics.minimumTarget)
            .contentShape(Rectangle())
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .foregroundStyle(.primary)
        .accessibilityLabel("Link state \(pane.link.label)")
        .accessibilityHint("Changes how this pane follows navigation")
    }
}

/// The compact and medium width tab row for the panes that are not visible (specs/21 §10).
struct PaneTabs: View {
    @ObservedObject var workspace: Workspace
    let visible: Set<String>

    var body: some View {
        HStack(spacing: 0) {
            ForEach(workspace.panes) { pane in
                Button {
                    workspace.activate(pane.id)
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: pane.link.symbol).imageScale(.small)
                        Text(pane.content.shortName)
                    }
                    .font(.caption)
                    .fontWeight(visible.contains(pane.id) ? .semibold : .regular)
                    .padding(.horizontal, 10)
                    .frame(maxWidth: .infinity, minHeight: ChromeMetrics.tabsRow)
                    .contentShape(Rectangle())
                    .overlay(alignment: .bottom) {
                        Rectangle().fill(visible.contains(pane.id) ? Color.primary : .clear).frame(height: 1)
                    }
                }
                .buttonStyle(.plain)
                .foregroundStyle(visible.contains(pane.id) ? .primary : .secondary)
                .accessibilityLabel("\(pane.content.fullTitle), \(pane.link.label)")
                .accessibilityAddTraits(visible.contains(pane.id) ? .isSelected : [])
            }
        }
        .frame(height: ChromeMetrics.tabsRow)
        .overlay(alignment: .bottom) { Divider() }
    }
}

/// The few-second confirmation after capture or a link change, near the indicator, never over
/// the selection (specs/21 §5).
struct ConfirmationLine: View {
    @ObservedObject var workspace: Workspace

    var body: some View {
        if let confirmation = workspace.confirmation {
            HStack(spacing: 10) {
                Image(systemName: "checkmark.circle")
                VStack(alignment: .leading, spacing: 2) {
                    Text(confirmation.message).font(.subheadline)
                    if let detail = confirmation.detail {
                        Text(detail).font(.caption).foregroundStyle(.secondary)
                    }
                }
                Spacer(minLength: 0)
                Button("Undo") { workspace.undoLastCapture() }
                    .font(.subheadline)
                    .frame(minWidth: ChromeMetrics.minimumTarget, minHeight: ChromeMetrics.minimumTarget)
                Button {
                    workspace.confirmation = nil
                } label: {
                    Image(systemName: "xmark")
                        .frame(minWidth: ChromeMetrics.minimumTarget, minHeight: ChromeMetrics.minimumTarget)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Dismiss")
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 4)
            .background(RoundedRectangle(cornerRadius: 10).fill(.thickMaterial).shadow(radius: 4))
            .frame(maxWidth: 420)
            .accessibilityElement(children: .combine)
            .accessibilityAddTraits(.updatesFrequently)
        }
    }
}
