// B072: the numbered columns. Each column: a 28 pt tab strip (the shown tab by weight and a
// 2 pt accent underline, a `+` at the end, × on hover), a 32 pt identity row (the column's
// number badge, filled in the accent when active, the resource, its passage, the link state
// and a pane menu), then the TextKit 2 reading surface for the shown tab, capped at the
// measure and centred, with an ink layer over it. Columns are separated by 1 pt rules that
// resize them when dragged. B077: a right-click on a tab, the identity row, the strip or the
// column's empty area opens the OpenLogos menu for it (ContextMenu.swift). B074: a tab is
// dragged by its label to a slot in any column's strip (an accent bar marks the slot), onto a
// column's text (its end) or onto the band at the last column's trailing edge (a new
// column); the drop is one undoable `tab.move`. A rule drag commits one `column.resize`.
import SwiftUI

struct ColumnsView: View {
    @ObservedObject var model: ShellModel

    var body: some View {
        GeometryReader { geometry in
            let ws = model.activeWorkspace
            let widths = Self.widths(ws.columns, overrides: model.widthOverrides, total: geometry.size.width - CGFloat(max(0, ws.columns.count - 1)) * ShellTokens.rule)
            HStack(spacing: 0) {
                ForEach(Array(ws.columns.enumerated()), id: \.element.id) { index, column in
                    if index > 0 {
                        ColumnRule(model: model, left: ws.columns[index - 1], right: column, widths: widths, index: index)
                    }
                    ColumnView(model: model, column: column, isActive: column.number == ws.activeColumnNumber, leading: index == 0)
                        .frame(width: max(0, widths[index]))
                }
            }
            .coordinateSpace(name: Self.space)
            .overlay(alignment: .topLeading) { TabDropOverlay(model: model) }
            .id(ws.id)
        }
    }

    /// The columns' coordinate space (tab frames, the drag's pointer).
    static let space = "shell-columns"

    /// The columns' widths: the core's fractions, or the host's after a drag, normalised.
    static func widths(_ columns: [ShellColumn], overrides: [String: CGFloat], total: CGFloat) -> [CGFloat] {
        let fractions = columns.map { overrides[$0.id] ?? CGFloat($0.widthFraction) }
        let sum = fractions.reduce(0, +)
        guard sum > 0 else { return columns.map { _ in total / CGFloat(max(1, columns.count)) } }
        return fractions.map { $0 / sum * total }
    }
}

/// The 1 pt rule between two columns, with a wider invisible handle that drags it.
struct ColumnRule: View {
    @ObservedObject var model: ShellModel
    let left: ShellColumn
    let right: ShellColumn
    let widths: [CGFloat]
    let index: Int
    @State private var start: (CGFloat, CGFloat)?

    var body: some View {
        Rectangle()
            .fill(model.theme.rulesColor)
            .frame(width: ShellTokens.rule)
            .overlay(
                Color.clear
                    .frame(width: 9)
                    .contentShape(Rectangle())
                    #if os(macOS)
                    .onHover { inside in if inside { NSCursor.resizeLeftRight.push() } else { NSCursor.pop() } }
                    #endif
                    .gesture(
                        DragGesture(minimumDistance: 1)
                            .onChanged { value in
                                let total = widths.reduce(0, +)
                                guard total > 0 else { return }
                                let (l, r) = start ?? (widths[index - 1], widths[index])
                                if start == nil { start = (l, r) }
                                let minimum: CGFloat = 220
                                let dx = max(minimum - l, min(r - minimum, value.translation.width))
                                var overrides = model.widthOverrides
                                for (i, column) in model.activeWorkspace.columns.enumerated() {
                                    overrides[column.id] = widths[i] / total
                                }
                                overrides[left.id] = (l + dx) / total
                                overrides[right.id] = (r - dx) / total
                                model.widthOverrides = overrides
                            }
                            .onEnded { _ in
                                start = nil
                                if let share = model.widthOverrides[left.id] { model.commitResize(column: left.number, share: share) }
                            }
                    )
            )
            .accessibilityElement()
            .accessibilityLabel("Rule between columns \(left.number) and \(right.number)")
            .accessibilityHint("Drag to resize")
    }
}

struct ColumnView: View {
    @ObservedObject var model: ShellModel
    let column: ShellColumn
    let isActive: Bool
    /// The leading column's text clears the floating tool well, expanded or collapsed.
    var leading = false

    var body: some View {
        VStack(spacing: 0) {
            TabStrip(model: model, column: column, isActive: isActive)
            IdentityRow(model: model, column: column, isActive: isActive)
            Rectangle().fill(model.theme.rulesColor).frame(height: ShellTokens.rule)
            if let tab = column.tabs.first(where: { $0.id == column.shownTabId }) {
                TabBody(model: model, tab: tab, column: column, leadingInset: leading ? model.toolWellClearance : 0)
                    .id(tab.id)
            }
        }
        .background(model.theme.pageColor)
        .onGeometryChange(for: CGRect.self) { $0.frame(in: .named(ColumnsView.space)) } action: { model.columnFrames[column.number] = $0 }
        .contentShape(Rectangle())
        .simultaneousGesture(TapGesture().onEnded { model.focusColumn(column.number) })
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Column \(column.number)\(isActive ? ", active" : "")")
        .accessibilityIdentifier("shell-column-\(column.number)")
    }
}

// MARK: - Tab strip

struct TabStrip: View {
    @ObservedObject var model: ShellModel
    let column: ShellColumn
    let isActive: Bool

    var body: some View {
        HStack(spacing: 0) {
            GeometryReader { geometry in
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 0) {
                        ForEach(column.tabs, id: \.id) { tab in
                            TabLabel(model: model, live: model.liveLabels, column: column, tab: tab, shown: tab.id == column.shownTabId)
                        }
                    }
                    // B077: the strip's empty area is inside the scroll view, so its catcher is too.
                    .frame(minWidth: geometry.size.width, minHeight: geometry.size.height, alignment: .leading)
                    .background(BackgroundMenu(model: model, key: "strip-\(column.number)", column: column.number))
                }
            }
            Button {
                model.newTab(inColumn: column.number)
            } label: {
                Image(systemName: "plus").font(.system(size: 11, weight: .medium))
                    .frame(width: 24, height: ShellTokens.tabStripHeight)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .foregroundStyle(model.theme.chromeColor)
            .help("New tab  \(model.shortcut("tab.new") ?? "")")
            .accessibilityLabel("New tab in column \(column.number)")
        }
        .padding(.leading, ShellTokens.Spacing.xs)
        .frame(height: ShellTokens.tabStripHeight)
    }
}

/// B077: the column's empty area answers a right-click with the background menu.
struct BackgroundMenu: View {
    let model: ShellModel
    let key: String
    let column: UInt32

    var body: some View {
        #if os(macOS)
        MenuCatcher(model: model, key: key) { model.nsMenu(for: .background(column: column)) }
        #else
        Color.clear
        #endif
    }
}

struct TabLabel: View {
    @ObservedObject var model: ShellModel
    /// B078: the labels a scroll moved, ahead of the session (refreshed when scrolling stops).
    @ObservedObject var live: ReadingLabels
    let column: ShellColumn
    let tab: ShellTab
    let shown: Bool
    @State private var hovering = false

    var body: some View {
        let label = TabTitle.label(tab, live: live.byTab[tab.id])
        HStack(spacing: ShellTokens.Spacing.xs) {
            Text(label)
                .font(.system(size: ShellTokens.tabLabelSize, weight: shown ? .semibold : .regular))
                .foregroundStyle(shown ? model.theme.textColor : model.theme.chromeColor)
                .lineLimit(1)
            Button {
                model.closeTab(tab.id)
            } label: {
                Image(systemName: "xmark").font(.system(size: 8, weight: .bold))
                    .frame(width: 14, height: 14)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .foregroundStyle(model.theme.chromeColor)
            .opacity(hovering ? 1 : 0)
            .accessibilityLabel("Close \(label)")
            .help("Close tab  \(model.shortcut("tab.close") ?? "")")
        }
        .padding(.horizontal, ShellTokens.Spacing.s)
        .frame(height: ShellTokens.tabStripHeight)
        .overlay(alignment: .bottom) {
            if shown {
                Rectangle().fill(model.theme.accentColor).frame(height: ShellTokens.accentUnderline)
                    .padding(.horizontal, ShellTokens.Spacing.xs)
            }
        }
        .contentShape(Rectangle())
        .opacity(model.tabDrag?.tabID == tab.id ? 0.4 : 1)
        .onGeometryChange(for: CGRect.self) { $0.frame(in: .named(ColumnsView.space)) } action: { model.tabFrames[tab.id] = $0 }
        .onHover { hovering = $0 }
        .onTapGesture {
            model.focusColumn(column.number)
            if !shown { model.showTab(tab.id) }
        }
        // B074: dragged to another slot or column.
        .gesture(
            DragGesture(minimumDistance: 6, coordinateSpace: .named(ColumnsView.space))
                .onChanged { model.dragTab(tab.id, to: $0.location) }
                .onEnded { _ in model.dropTab() }
        )
        .shellContextMenu("tab-\(tab.id)", { .tab(tabId: tab.id) }, model: model)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(label)
        .accessibilityAddTraits(shown ? [.isButton, .isSelected] : .isButton)
        .accessibilityValue(shown ? "shown" : "")
        .accessibilityIdentifier("shell-tab-\(column.number)-\(tab.id)")
    }
}

enum TabTitle {
    /// `BSB Jn 3`, from the core's tab view (registered short name and the chapter's short
    /// form); a tab with no passage shows its resource alone.
    static func label(_ tab: ShellTab, live: ReadingLabels.Live? = nil) -> String {
        (live?.chapterLabel ?? tab.chapterLabel).map { "\(tab.resourceTitle) \($0)" } ?? tab.resourceTitle
    }

    /// `Group 1`, `Pinned`, `Follows`.
    static func linkState(_ tab: ShellTab) -> String {
        switch tab.linkMode {
        case "group": return "Group \(tab.linkGroup ?? "")"
        case "pinned": return "Pinned"
        default: return "Follows"
        }
    }
}

/// B074: while a tab is dragged, the slot it would drop into (a 2 pt accent bar in the strip),
/// the new-column band at the trailing edge, and the tab's label under the pointer.
struct TabDropOverlay: View {
    @ObservedObject var model: ShellModel

    var body: some View {
        if let drag = model.tabDrag {
            let ws = model.activeWorkspace
            ZStack(alignment: .topLeading) {
                if let last = ws.columns.last, let frame = model.columnFrames[last.number] {
                    let active = drag.drop?.newColumn == true
                    RoundedRectangle(cornerRadius: 6)
                        .fill(model.theme.accentColor.opacity(active ? 0.18 : 0.07))
                        .overlay(
                            Text("New column")
                                .font(ShellTokens.chromeFont)
                                .foregroundStyle(model.theme.accentColor)
                                .rotationEffect(.degrees(-90))
                                .fixedSize()
                        )
                        .frame(width: ShellModel.newColumnZone - 8, height: max(0, frame.height - 16))
                        .offset(x: frame.maxX - ShellModel.newColumnZone + 4, y: frame.minY + 8)
                        .accessibilityIdentifier("shell-drop-new-column")
                }
                if let drop = drag.drop, !drop.newColumn, let bar = slotBar(drop, ws: ws) {
                    Rectangle().fill(model.theme.accentColor)
                        .frame(width: 2, height: bar.height)
                        .offset(x: bar.minX - 1, y: bar.minY)
                        .accessibilityIdentifier("shell-drop-slot")
                }
                if let tab = ws.columns.lazy.flatMap(\.tabs).first(where: { $0.id == drag.tabID }) {
                    Text(TabTitle.label(tab))
                        .font(.system(size: ShellTokens.tabLabelSize, weight: .semibold))
                        .padding(.horizontal, ShellTokens.Spacing.s)
                        .padding(.vertical, 4)
                        .background(RoundedRectangle(cornerRadius: 5).fill(model.theme.pageColor).shadow(radius: 3))
                        .fixedSize()
                        .offset(x: drag.location.x + 10, y: drag.location.y + 6)
                }
            }
            .allowsHitTesting(false)
        }
    }

    /// The gap before the slot's tab, or after the column's last tab.
    private func slotBar(_ drop: ShellModel.TabDrop, ws: ShellWorkspace) -> CGRect? {
        guard let column = ws.columns.first(where: { $0.number == drop.column }), let index = drop.index else { return nil }
        if let tab = column.tabs[safe: Int(index)], let frame = model.tabFrames[tab.id] {
            return CGRect(x: frame.minX, y: frame.minY + 4, width: 0, height: frame.height - 8)
        }
        if let tab = column.tabs.last, let frame = model.tabFrames[tab.id] {
            return CGRect(x: frame.maxX, y: frame.minY + 4, width: 0, height: frame.height - 8)
        }
        return nil
    }
}

// MARK: - Identity row

struct IdentityRow: View {
    @ObservedObject var model: ShellModel
    let column: ShellColumn
    let isActive: Bool

    var body: some View {
        IdentityRowContent(model: model, live: model.liveLabels, column: column, isActive: isActive)
    }
}

private struct IdentityRowContent: View {
    @ObservedObject var model: ShellModel
    @ObservedObject var live: ReadingLabels
    let column: ShellColumn
    let isActive: Bool

    var body: some View {
        let tab = column.tabs.first { $0.id == column.shownTabId }
        let resource = tab.map { model.textSource.resource($0.contentId) }
        let reference = tab.flatMap { live.byTab[$0.id]?.reference } ?? tab?.reference
        HStack(spacing: ShellTokens.Spacing.s) {
            ColumnBadge(number: column.number, active: isActive, theme: model.theme)
            Text(resource?.name ?? "")
                .font(.system(size: ShellTokens.chromeSize, weight: isActive ? .semibold : .regular))
                .lineLimit(1)
                .truncationMode(.tail)
                .layoutPriority(0)
            Text(ShellReference.humanOsis(reference))
                .font(ShellTokens.chromeFont)
                .foregroundStyle(model.theme.chromeColor)
                .lineLimit(1)
                .fixedSize()
                .layoutPriority(1)
            if let attribution = resource?.attribution, let resource {
                AboutTextButton(resource: resource, attribution: attribution, theme: model.theme, open: model.aboutColumn == column.number)
            }
            Spacer(minLength: ShellTokens.Spacing.xs)
            if let tab {
                Text(TabTitle.linkState(tab))
                    .font(ShellTokens.chromeFont)
                    .foregroundStyle(model.theme.chromeColor)
                    .lineLimit(1)
                    .fixedSize()
            }
            PaneMenu(model: model, column: column)
        }
        .padding(.horizontal, ShellTokens.Spacing.s)
        .frame(height: ShellTokens.identityRowHeight)
        .shellContextMenu("column-\(column.number)", { .column(column: column.number) }, model: model)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Column \(column.number), \(resource?.name ?? ""), \(ShellReference.humanOsis(reference)), \(tab.map(TabTitle.linkState) ?? "")\(isActive ? ", active" : "")")
        .accessibilityIdentifier("shell-identity-\(column.number)")
    }
}

/// B071: the licence's words, as the edition's manifest has them (`Edition.attribution`): one
/// quiet mark on the identity row, a small panel on click, nothing over the text.
struct AboutTextButton: View {
    let resource: ResourceInfo
    let attribution: String
    let theme: ShellTokens.Theme
    var open = false
    @State private var shown = false

    var body: some View {
        Button { shown.toggle() } label: {
            Image(systemName: "info.circle")
                .font(.system(size: 11))
                .foregroundStyle(theme.chromeColor)
                .frame(width: 16, height: 16)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help("About this text")
        .accessibilityLabel("About this text: \(resource.name)")
        .accessibilityIdentifier("shell-about-\(resource.id)")
        .popover(isPresented: $shown, arrowEdge: .bottom) {
            AboutTextPanel(resource: resource, attribution: attribution)
        }
        .onChange(of: open) { _, open in if open { shown = true } }
    }
}

struct AboutTextPanel: View {
    let resource: ResourceInfo
    let attribution: String

    var body: some View {
        VStack(alignment: .leading, spacing: ShellTokens.Spacing.s) {
            Text(resource.name).font(ShellTokens.chromeFont.weight(.semibold))
            Text(attribution)
                .font(ShellTokens.chromeFont)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(ShellTokens.Spacing.m)
        .frame(width: 340, alignment: .leading)
        .accessibilityIdentifier("shell-about-panel")
    }
}

struct ColumnBadge: View {
    let number: UInt32
    let active: Bool
    let theme: ShellTokens.Theme

    var body: some View {
        Text("\(number)")
            .font(ShellTokens.badgeFont)
            .foregroundStyle(active ? theme.pageColor : theme.chromeColor)
            .frame(width: ShellTokens.badgeSide, height: ShellTokens.badgeSide)
            .background(
                RoundedRectangle(cornerRadius: 5)
                    .fill(active ? theme.accentColor : .clear)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 5)
                    .strokeBorder(active ? theme.accentColor : theme.chromeColor.opacity(0.6), lineWidth: 1)
            )
            .accessibilityLabel("Column \(number)")
    }
}

/// The pane menu: the tab's and column's commands, each a registered command.
struct PaneMenu: View {
    @ObservedObject var model: ShellModel
    let column: ShellColumn

    var body: some View {
        Menu {
            ForEach(["nav.back", "nav.forward"], id: \.self) { CommandButton(model: model, id: $0) }
            Divider()
            CommandButton(model: model, id: "tab.link", title: "Link to group 1", args: ShellCommandArgs(group: "1"))
            CommandButton(model: model, id: "tab.pin")
            CommandButton(model: model, id: "tab.follow")
            Divider()
            let others = model.activeWorkspace.columns.map(\.number).filter { $0 != column.number }
            if !others.isEmpty {
                Menu("Move tab to column") {
                    ForEach(others, id: \.self) { n in
                        CommandButton(model: model, id: "tab.move_to_column", title: "Column \(n)", args: ShellCommandArgs(column: n))
                    }
                }
                Menu("Duplicate tab into column") {
                    ForEach(others, id: \.self) { n in
                        CommandButton(model: model, id: "tab.duplicate_to_column", title: "Column \(n)", args: ShellCommandArgs(column: n))
                    }
                }
            }
            CommandButton(model: model, id: "tab.reopen_closed")
            Divider()
            ForEach(["column.new", "column.move_left", "column.move_right", "column.close", "view.notes_dock"], id: \.self) {
                CommandButton(model: model, id: $0)
            }
        } label: {
            Image(systemName: "ellipsis")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(model.theme.chromeColor)
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .fixedSize()
        .simultaneousGesture(TapGesture().onEnded { model.focusColumn(column.number) })
        .accessibilityLabel("Column \(column.number) menu")
    }
}

// MARK: - Text

struct TabBody: View {
    @ObservedObject var model: ShellModel
    let tab: ShellTab
    let column: ShellColumn
    var leadingInset: CGFloat = 0

    var body: some View {
        GeometryReader { geometry in
            let width = min(geometry.size.width - leadingInset, model.measureCap)
            let padding = width < 420 ? ShellTokens.Spacing.l : ShellTokens.Spacing.xl
            ZStack {
                #if os(macOS)
                let isNote = tab.kind == "notes"
                // B078: a reading tab is its edition's stream of chapters.
                let continuous = tab.kind == "reading" && tab.reference != nil
                #else
                let isNote = false
                let continuous = false
                #endif
                if isNote {
                    #if os(macOS)
                    // B076: notes are edited in place (Type).
                    NoteEditorView(store: model.notes, noteID: tab.contentId, paneID: tab.id, typography: model.noteTypography,
                                   horizontalPadding: padding, onFocus: { model.focusColumn(column.number) })
                    #endif
                } else if tab.kind == "lexicon" {
                    LexiconView(model: model, tab: tab)
                } else if let reason = model.readingError(for: tab) {
                    NotInEditionView(model: model, resource: model.textSource.resource(tab.contentId), tab: tab, reason: reason)
                } else if continuous {
                    #if os(macOS)
                    ContinuousReaderView(model: model, tab: tab, column: column, horizontalPadding: padding)
                        .id("\(tab.id)|\(tab.contentId)")
                        .accessibilityLabel("\(model.textSource.resource(tab.contentId).name), \(ShellReference.human(tab.reference))")
                    #endif
                } else if let text = model.text(for: tab) {
                    ReaderTextView(
                        paneID: tab.id,
                        content: text.content,
                        horizontalPadding: padding,
                        onSelection: { model.selectionChanged($0, tabID: tab.id) },
                        onAction: { action, selection in
                            guard let selection else { return }
                            switch action {
                            case .highlight: model.selectionChanged(selection, tabID: tab.id); model.run("selection.highlight")
                            case .collect: model.selectionChanged(selection, tabID: tab.id); model.run("selection.collect")
                            case .study: model.selectionChanged(selection, tabID: tab.id); model.run("selection.study")
                            default: model.show(toast: "\(action.rawValue): not in this preview yet")
                            }
                        },
                        typography: model.typography,
                        marks: model.readerMarks(tab.id),
                        dragMode: model.dragMode,
                        onDragEnd: { mode, selection in model.dragEnded(mode, selection) },
                        onSurface: { model.register($0, for: tab.id) },
                        onActivate: { model.focusColumn(column.number) },
                        onContextMenu: { hit in model.nsMenu(for: hit, tab: tab, column: column) }
                    )
                    .accessibilityLabel("\(text.resource.name), \(ShellReference.human(tab.reference))")
                } else {
                    PlaceholderView(model: model, resource: model.textSource.resource(tab.contentId), reference: tab.reference)
                }
                if model.inkActive || model.hasInk(tab.id) {
                    InkCanvas(sheet: model.inkSheet(for: tab.id)) { _ in }
                        .allowsHitTesting(model.inkActive)
                        .accessibilityLabel("Ink layer")
                }
            }
            .frame(width: max(0, width), height: geometry.size.height)
            // Centred in what the well leaves free, never wider than the column.
            .frame(width: max(0, geometry.size.width - leadingInset))
            .padding(.leading, leadingInset)
        }
        .background(BackgroundMenu(model: model, key: "background-\(column.number)", column: column.number))
    }
}

/// specs/21's not-in-this-edition state (B078): the tab keeps its passage and says, in the
/// core's words, why its edition has no text for it; never another edition's text.
struct NotInEditionView: View {
    @ObservedObject var model: ShellModel
    let resource: ResourceInfo
    let tab: ShellTab
    let reason: String

    var body: some View {
        VStack(alignment: .leading, spacing: ShellTokens.Spacing.s) {
            Label("\(resource.name) · \(ShellReference.humanOsis(tab.reference))", systemImage: "book.closed")
                .font(ShellTokens.chromeFont.weight(.semibold))
            Text(reason.hasSuffix(".") ? reason : reason + ".")
                .font(ShellTokens.chromeFont)
                .foregroundStyle(model.theme.chromeColor)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: ShellTokens.Spacing.s) {
                Button("Choose another…") { model.focusPalette() }
                Button("Close tab") { model.closeTab(tab.id) }
            }
            .controlSize(.small)
            .padding(.top, ShellTokens.Spacing.xs)
        }
        .padding(ShellTokens.Spacing.xl)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("shell-not-in-edition")
    }
}

/// specs/21 §11: a resource with no text for this passage shows where the text would be, not
/// an error.
struct PlaceholderView: View {
    @ObservedObject var model: ShellModel
    let resource: ResourceInfo
    let reference: String?

    var body: some View {
        VStack(alignment: .leading, spacing: ShellTokens.Spacing.s) {
            Label("Not available in this preview", systemImage: "text.page.slash")
                .font(ShellTokens.chromeFont.weight(.semibold))
            Text("\(resource.name)\(reference.map { " has no text for \(ShellReference.human($0))" } ?? " has no text here") on this device yet. The tab keeps its place and shows the text once the edition is installed.")
                .font(ShellTokens.chromeFont)
                .foregroundStyle(model.theme.chromeColor)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(ShellTokens.Spacing.xl)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("shell-placeholder")
    }
}
