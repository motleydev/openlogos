// B072: the shell's host state around the core's `ShellHandle` (specs/22). The session, the
// registry, apply/undo and the palette query live in the core; this model keeps what only the
// host knows (text size, theme, the palette's field and list, the tool well's place, swatches,
// highlights, ink, toasts) and performs the commands the core returns `handled == false` for.
// Every button, menu item and shortcut goes through `run(_:args:)`, which dispatches a
// registered command; the view re-renders from `session` after each.
import Combine
import SwiftUI
#if canImport(AppKit)
import AppKit
#endif

@MainActor
final class ShellModel: ObservableObject {
    let handle: ShellHandle
    let registry: [ShellCommandInfo]
    let keymap: ShellKeymap
    let textSource: TextSource

    @Published private(set) var session: ShellSession

    // MARK: Palette

    /// The palette's state, observed only by the field and the result list, so a keystroke
    /// never re-renders the columns.
    let palette = PaletteState()
    var paletteText: String {
        get { palette.text }
        set { palette.text = newValue }
    }
    var paletteResult: ShellPaletteResult? { palette.result }
    var paletteOpen: Bool { palette.open }
    /// Index into `paletteRows`.
    var paletteSelection: Int {
        get { palette.selection }
        set { palette.selection = newValue }
    }
    var expandedSection: ShellPaletteSection? { palette.expanded }
    /// Set once the window has settled; focus before that is the system's default focus.
    var launched = false

    // MARK: Appearance

    @Published var sizeIndex = ShellTokens.defaultReadingSizeIndex
    @Published var spacingIndex = ShellTokens.defaultLineSpacingIndex
    @Published var theme = ShellTokens.light

    // MARK: Tools

    @Published var toolWellCollapsed = false
    /// The well's centre as a fraction of the columns' height.
    @Published var toolWellPosition: CGFloat = 0.5
    @Published private(set) var swatchIndex = 0
    @Published private(set) var alternateSwatchIndex = 1
    @Published var inkWidthIndex = 1

    // MARK: Per tab

    @Published private(set) var marks: [String: [ReaderMark]] = [:]
    @Published private(set) var toast: String?
    @Published private(set) var selection: ReaderSelection?
    /// Host-side column widths after a drag of the rule (the core has no resize command yet).
    @Published var widthOverrides: [String: CGFloat] = [:]

    private var inkSheets: [String: InkSheet] = [:]
    private var surfaces: [String: WeakSurface] = [:]
    private var scrollAnchors: [String: String] = [:]
    private var toastTask: Task<Void, Never>?
    private var markCounter = 0
    #if canImport(AppKit)
    private var monitor: Any?
    #endif

    init(handle: ShellHandle, textSource: TextSource = FixtureTextSource()) {
        self.handle = handle
        self.textSource = textSource
        registry = shellRegistry()
        keymap = ShellKeymap(registry: registry, platform: .mac)
        session = handle.session()
    }

    convenience init(sessionJSON: String = ShellSeed.json) {
        // The seed is ours; a seed the core rejects is a programming error.
        let handle = try! ShellHandle(sessionJson: sessionJSON)
        handle.useFixtureText()
        self.init(handle: handle)
    }

    // MARK: Session views

    var activeWorkspace: ShellWorkspace {
        session.workspaces.first { $0.id == session.activeWorkspaceId } ?? session.workspaces[0]
    }

    var activeWorkspaceIndex: Int {
        session.workspaces.firstIndex { $0.id == session.activeWorkspaceId } ?? 0
    }

    var activeColumn: ShellColumn {
        let ws = activeWorkspace
        return ws.columns[max(0, min(ws.columns.count - 1, Int(ws.activeColumnNumber) - 1))]
    }

    var activeTab: ShellTab? { activeColumn.tabs.first { $0.id == activeColumn.shownTabId } }

    var tool: String { session.tool }

    var readingSize: CGFloat { ShellTokens.readingSizes[sizeIndex] }
    var lineSpacing: CGFloat { ShellTokens.lineSpacings[spacingIndex] }
    var swatch: ShellTokens.Swatch { ShellTokens.swatches[swatchIndex] }
    var alternateSwatch: ShellTokens.Swatch { ShellTokens.swatches[alternateSwatchIndex] }
    var inkWidth: Double { ShellTokens.inkWidths[inkWidthIndex] }

    var typography: ReaderTypography {
        ReaderTypography.fixed(
            size: readingSize, lineHeight: lineSpacing,
            text: PlatformColor(hex: theme.text), secondary: PlatformColor(hex: theme.chrome),
            key: "\(theme.id)-\(sizeIndex)-\(spacingIndex)"
        )
    }

    func command(_ id: String) -> ShellCommandInfo? { registry.first { $0.id == id } }

    /// The Mac default shortcut of a command, for tooltips and menus.
    func shortcut(_ id: String) -> String? { command(id)?.shortcuts.mac.first.map(Self.symbols) }

    static func symbols(_ shortcut: String) -> String {
        shortcut.replacingOccurrences(of: "Cmd+", with: "⌘").replacingOccurrences(of: "Alt+", with: "⌥")
            .replacingOccurrences(of: "Shift+", with: "⇧").replacingOccurrences(of: "Ctrl+", with: "⌃")
    }

    /// Whether the command can run now, from the core (`commands(hasSelection:)`), asked once
    /// per session revision: every menu item of every column reads it on each render.
    func disabledReason(_ id: String) -> String? {
        let key = "\(session.revision)-\(selection != nil)"
        if key != disabledKey {
            disabledKey = key
            disabled = Dictionary(uniqueKeysWithValues: handle.commands(hasSelection: selection != nil).map { ($0.id, $0.disabledReason) })
        }
        return disabled[id] ?? nil
    }

    private var disabledKey = ""
    private var disabled: [String: String?] = [:]

    // MARK: Running commands

    /// Dispatches a registered command; the host performs what the core does not.
    @discardableResult
    func run(_ id: String, args: ShellCommandArgs = ShellCommandArgs()) -> Bool {
        rememberScroll()
        do {
            let outcome = try handle.apply(commandId: id, args: args, hasSelection: selection != nil)
            if !outcome.handled { performHost(id, args: args) }
            refresh()
            return true
        } catch let error as ShellCommandError {
            switch error {
            case .Disabled(_, _, let reason): show(toast: reason)
            case .UnknownCommand(let commandId): show(toast: "Unknown command \(commandId)")
            case .InvalidDocument(let detail): show(toast: detail)
            }
            return false
        } catch {
            show(toast: "\(error)")
            return false
        }
    }

    func refresh() {
        let next = handle.session()
        if next != session { session = next }
        if paletteOpen { query() }
        syncInk()
    }

    private func performHost(_ id: String, args: ShellCommandArgs) {
        switch id {
        case "palette.focus": focusPalette()
        case "view.text_size_up": sizeIndex = min(ShellTokens.readingSizes.count - 1, sizeIndex + 1)
        case "view.text_size_down": sizeIndex = max(0, sizeIndex - 1)
        case "view.text_size_reset": sizeIndex = ShellTokens.defaultReadingSizeIndex
        case "view.theme":
            if let name = args.text, let named = ShellTokens.theme(named: name) {
                theme = named
            } else {
                let index = ShellTokens.themes.firstIndex(of: theme) ?? 0
                theme = ShellTokens.themes[(index + 1) % ShellTokens.themes.count]
            }
        case "tool.swap_swatches": swap(&swatchIndex, &alternateSwatchIndex); syncInk()
        case "view.tool_palette": toolWellCollapsed.toggle()
        case "selection.highlight":
            if let selection { highlight(selection) }
        case "selection.collect":
            if let selection { collect(selection) }
        case "selection.study":
            show(toast: "Study: \(selection.map { ShellReference.short($0.anchors.first?.segmentId ?? "") } ?? "") (no study drawer yet)")
        case "view.collection_drawer", "view.study_drawer", "view.focus_mode", "workspace.export":
            show(toast: "\(command(id)?.title ?? id): not in this preview yet")
        default:
            show(toast: "\(command(id)?.title ?? id) is not performed by this host")
        }
    }

    // MARK: Convenience dispatch for the chrome

    func focusColumn(_ number: UInt32) {
        guard number != activeWorkspace.activeColumnNumber else { return }
        run("column.focus", args: ShellCommandArgs(column: number))
    }

    func showTab(_ tabID: String) {
        run("tab.focus", args: ShellCommandArgs(workspaceId: session.activeWorkspaceId, tabId: tabID))
    }

    func closeTab(_ tabID: String) {
        if activeTab?.id != tabID { showTab(tabID) }
        run("tab.close")
    }

    func newTab(inColumn number: UInt32) {
        focusColumn(number)
        run("tab.new")
    }

    // MARK: Palette

    var paletteContext: ShellPaletteContext {
        ShellPaletteContext(hasSelection: selection != nil, platform: .mac, expanded: expandedSection)
    }

    private var queriedText = ""

    /// The rows in display order (the flat list ↑/↓ walk).
    var paletteRows: [ShellPaletteItem] { paletteResult?.groups.flatMap(\.items) ?? [] }

    /// Called on every keystroke; no debounce (specs/22 §7: 16 ms).
    func query() {
        queriedText = paletteText
        palette.result = handle.paletteQuery(text: paletteText, context: paletteContext)
        paletteSelection = min(paletteSelection, max(0, paletteRows.count - 1))
    }

    func paletteTextChanged() {
        // The field's onChange repeats a change made in code; the second call is a no-op.
        guard palette.result == nil || paletteText != queriedText else { return }
        palette.open = true
        palette.expanded = nil
        paletteSelection = 0
        query()
    }

    func focusPalette() {
        palette.open = true
        palette.focusRequest += 1
        query()
    }

    func movePaletteSelection(_ delta: Int) {
        let count = paletteRows.count
        guard count > 0 else { return }
        paletteSelection = (paletteSelection + delta + count) % count
    }

    /// ⌥Enter or Tab: show all of the selected row's section, or collapse it again.
    func toggleExpandSelectedSection() {
        guard let row = paletteRows[safe: paletteSelection] else { return }
        expand(row.section)
    }

    func expand(_ section: ShellPaletteSection) {
        palette.expanded = expandedSection == section ? nil : section
        query()
        if let first = paletteRows.firstIndex(where: { $0.section == section }) { paletteSelection = first }
    }

    func runSelectedRow() {
        guard let row = paletteRows[safe: paletteSelection] else { return }
        runRow(row)
    }

    func runRow(_ row: ShellPaletteItem) {
        if let reason = row.disabledReason {
            show(toast: reason)
            return
        }
        closePalette()
        run(row.commandId, args: row.args)
    }

    /// Esc: closes the list and returns focus to the tab that had it.
    func closePalette() {
        palette.open = false
        palette.text = ""
        palette.expanded = nil
        palette.selection = 0
        palette.result = nil
        focusActiveTab()
    }

    func focusActiveTab() {
        guard let id = activeTab?.id else { return }
        // After SwiftUI has dropped the field's focus.
        DispatchQueue.main.async { [weak self] in self?.surfaces[id]?.surface?.focus() }
    }

    // MARK: Text

    func text(for tab: ShellTab) -> SourceText? {
        textSource.text(resourceID: tab.contentId, reference: tab.reference)
    }

    func register(_ surface: ReaderSurface, for tabID: String) {
        guard surfaces[tabID]?.surface !== surface else { return }
        surfaces[tabID] = WeakSurface(surface: surface)
        // A tab revealed again opens where it was left.
        if let segment = scrollAnchors[tabID] ?? session.workspaces.lazy.flatMap(\.columns).flatMap(\.tabs).first(where: { $0.id == tabID })?.scrollSegmentId {
            DispatchQueue.main.async { surface.scroll(toSegment: segment) }
        }
    }

    func surface(for tabID: String) -> ReaderSurface? { surfaces[tabID]?.surface }

    /// The scroll anchor of every shown tab, kept on the host until the core records it.
    private func rememberScroll() {
        for column in activeWorkspace.columns {
            if let segment = surfaces[column.shownTabId]?.surface?.topSegmentID { scrollAnchors[column.shownTabId] = segment }
        }
    }

    func selectionChanged(_ selection: ReaderSelection?, tabID: String) {
        if let selection {
            self.selection = selection
        } else if self.selection?.paneID == tabID {
            self.selection = nil
        }
    }

    func dragEnded(_ mode: ReaderDragMode, _ selection: ReaderSelection) {
        switch mode {
        case .highlight: highlight(selection)
        case .collect: collect(selection)
        case .select: break
        }
    }

    var dragMode: ReaderDragMode {
        switch tool {
        case "highlight": return .highlight
        case "collect": return .collect
        default: return .select
        }
    }

    private func highlight(_ selection: ReaderSelection) {
        markCounter += 1
        let mark = ReaderMark(id: "mark-\(markCounter)", ranges: selection.highlights, color: swatch.platformColor)
        marks[selection.paneID, default: []].append(mark)
        self.selection = nil
    }

    private func collect(_ selection: ReaderSelection) {
        let ids = selection.anchors.map(\.segmentId)
        var label = ShellReference.short(ids.first ?? "")
        if let last = ids.last, last != ids.first { label += "–\(last.split(separator: ".").last ?? "")" }
        show(toast: "Collected: \(label)")
        self.selection = nil
    }

    // MARK: Ink

    var inkActive: Bool { ["pen", "highlighter_pen", "eraser"].contains(tool) }

    func inkSheet(for tabID: String) -> InkSheet {
        if let sheet = inkSheets[tabID] { return sheet }
        let sheet = InkSheet(id: "shell-ink-\(tabID)")
        inkSheets[tabID] = sheet
        configure(sheet)
        return sheet
    }

    func hasInk(_ tabID: String) -> Bool { !(inkSheets[tabID]?.liveStrokes.isEmpty ?? true) }

    private func syncInk() { inkSheets.values.forEach(configure) }

    private func configure(_ sheet: InkSheet) {
        switch tool {
        case "highlighter_pen": sheet.tool = .highlighter
        case "eraser": sheet.tool = .eraser
        default: sheet.tool = .pen
        }
        // The pen draws in the theme's text colour, the highlighter in the current swatch.
        sheet.color = tool == "highlighter_pen" ? swatch.solid : theme.accentColor
        sheet.width = inkWidth
        // A pointer draws on the Mac; B039's palm rules still apply on iPad.
        sheet.mode = .finger
    }

    func cycleInkWidth() {
        inkWidthIndex = (inkWidthIndex + 1) % ShellTokens.inkWidths.count
        syncInk()
    }

    // MARK: Toasts

    func show(toast text: String) {
        toast = text
        toastTask?.cancel()
        toastTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 2_400_000_000)
            guard !Task.isCancelled else { return }
            self?.toast = nil
        }
    }

    // MARK: Keyboard

    /// Whether a text field (the palette, a rename field) has keyboard focus: bare keys are
    /// off then. The reading surface is a non-editable text view and does not count.
    var textFieldHasFocus: Bool {
        #if canImport(AppKit)
        guard let responder = NSApp.keyWindow?.firstResponder else { return false }
        if let text = responder as? NSTextView { return text.isEditable }
        return responder is NSTextField
        #else
        return false
        #endif
    }

    /// Runs the command bound to the chord; true when it was consumed.
    func handle(chord: KeyChord, inTextField: Bool) -> Bool {
        guard let binding = keymap.binding(for: chord) else { return false }
        if binding.bare && inTextField { return false }
        // In a field ⌘Z and ⌘⇧Z belong to the field's own editing.
        if inTextField, ["edit.undo", "edit.redo"].contains(binding.commandID) { return false }
        run(binding.commandID, args: ShellCommandArgs(column: binding.column))
        return true
    }

    #if canImport(AppKit)
    func installKeyMonitor() {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, let chord = KeyChord(event: event) else { return event }
            return self.handle(chord: chord, inTextField: self.textFieldHasFocus) ? nil : event
        }
    }

    func removeKeyMonitor() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }
    #endif
}

/// The palette field's text, its results, the selected row and the expanded section.
@MainActor
final class PaletteState: ObservableObject {
    @Published var text = ""
    @Published var result: ShellPaletteResult?
    @Published var open = false
    @Published var selection = 0
    @Published var expanded: ShellPaletteSection?
    /// Bumped to ask the view to focus the field.
    @Published var focusRequest = 0
}

private struct WeakSurface {
    weak var surface: ReaderSurface?
}

extension Array {
    subscript(safe index: Int) -> Element? { indices.contains(index) ? self[index] : nil }
}
