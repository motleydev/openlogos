// B072: the shell's host state around the core's `ShellHandle` (specs/22). The session (with
// its reading settings, ink width and swatches, column widths and scroll anchors), the
// registry, apply/undo and the palette query live in the core; this model keeps what only the
// host knows (the palette's field and list, the tool well's place, highlights, ink strokes,
// toasts, the notes being edited) and performs the commands the core returns
// `handled == false` for. B076: the tool well comes from the core for what has focus (a tab,
// the notes dock) and whether text is selected; every command carries that context.
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

    // MARK: Tools

    @Published var toolWellCollapsed = false
    /// 40 pt expanded, the 22 pt grip collapsed.
    var toolWellWidth: CGFloat { toolWellCollapsed ? ShellTokens.toolGripSide : ShellTokens.toolWellWidth }
    /// Column 1's text inset: the well (or its grip) + 12 + 8, so nothing ever covers text.
    var toolWellClearance: CGFloat { ShellTokens.toolWellInset + toolWellWidth + ShellTokens.Spacing.s }
    /// The well's centre as a fraction of the columns' height.
    @Published var toolWellPosition: CGFloat = 0.5

    // MARK: Focus and the well (B076)

    /// Where focus is: the active column's shown tab, or the notes dock's editor.
    @Published private(set) var focus: ShellFocus = .tab
    /// The core's well for `commandContext`, rebuilt on every focus, tab, selection and
    /// session change.
    @Published private(set) var well: ShellToolWell
    /// The notes being edited, in memory until B074.
    let notes = NoteStore()

    // MARK: Per tab

    @Published private(set) var marks: [String: [ReaderMark]] = [:]
    /// What each Lexicon tab shows, by tab id (B076 Word study).
    @Published private(set) var lexiconEntries: [String: LexiconEntry] = [:]
    @Published private(set) var toast: String?
    @Published private(set) var selection: ReaderSelection? {
        didSet { if selection != oldValue { refreshWell() } }
    }
    /// Column widths while a rule is dragged; the drag's end commits `column.resize`.
    @Published var widthOverrides: [String: CGFloat] = [:]
    /// The core's layout constants (measure caps, pane minimums).
    let layout = layoutConstants()

    private var inkSheets: [String: InkSheet] = [:]
    private var surfaces: [String: WeakSurface] = [:]
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
        // Tab views and palette rows name resources by these (`BSB · Jn 3:16`).
        handle.setResourceTitles(titles: textSource.resourceTitles)
        // Original-language tabs get the original tools (Word study).
        handle.setResourceLanguages(languages: textSource.resourceLanguages)
        session = handle.session()
        well = handle.toolWell(context: ShellCommandContext(hasSelection: false, focus: .tab))
        notes.model = self
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

    /// The stored tool; moving focus never changes it.
    var tool: String { session.tool }
    /// The tool in effect where focus is: the stored one, or the context's default when the
    /// context does not offer it (Pen stored, notes focused: Type).
    var effectiveTool: String { well.effectiveTool }

    /// What the core needs to know that the session does not.
    var commandContext: ShellCommandContext {
        ShellCommandContext(hasSelection: selection != nil, focus: focus)
    }

    /// Focus moved to a tab or into the notes dock's editor.
    func setFocus(_ next: ShellFocus) {
        guard next != focus else { return }
        focus = next
        refreshWell()
    }

    func refreshWell() {
        let next = handle.toolWell(context: commandContext)
        if next != well { well = next }
    }

    // MARK: Reading and ink settings (session.reading, session.ink)

    /// The core's text-size step (−2…4) as an index into the tokens' seven sizes (0 = 19 pt).
    var sizeIndex: Int {
        max(0, min(ShellTokens.readingSizes.count - 1, ShellTokens.defaultReadingSizeIndex + Int(session.reading.textSizeStep)))
    }
    var readingSize: CGFloat { ShellTokens.readingSizes[sizeIndex] }
    var lineSpacing: CGFloat { CGFloat(session.reading.lineSpacing) }
    /// `system` follows the app's appearance.
    var theme: ShellTokens.Theme {
        if let named = ShellTokens.theme(named: session.reading.theme) { return named }
        #if canImport(AppKit)
        let dark = NSApp?.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
        return dark ? ShellTokens.dark : ShellTokens.light
        #else
        return ShellTokens.light
        #endif
    }
    var swatch: ShellTokens.Swatch { ShellTokens.swatch(hex: session.ink.swatch) }
    var alternateSwatch: ShellTokens.Swatch { ShellTokens.swatch(hex: session.ink.alternateSwatch) }
    var inkWidth: Double { session.ink.width }
    var measureCap: CGFloat { CGFloat(layout.measureCapStandardPt) }

    var typography: ReaderTypography {
        ReaderTypography.fixed(
            size: readingSize, lineHeight: lineSpacing,
            text: PlatformColor(hex: theme.text), secondary: PlatformColor(hex: theme.chrome),
            key: "\(theme.id)-\(sizeIndex)-\(lineSpacing)"
        )
    }

    #if canImport(AppKit)
    /// A note's fonts and colours: the reading size, spacing and theme.
    var noteTypography: NoteTypography {
        NoteTypography(size: readingSize, lineSpacing: lineSpacing, text: theme.text, accent: theme.accent)
    }
    #endif

    func command(_ id: String) -> ShellCommandInfo? { registry.first { $0.id == id } }

    /// The Mac default shortcut of a command, for tooltips and menus.
    func shortcut(_ id: String) -> String? { command(id)?.shortcuts.mac.first.map(Self.symbols) }

    static func symbols(_ shortcut: String) -> String {
        shortcut.replacingOccurrences(of: "Cmd+", with: "⌘").replacingOccurrences(of: "Alt+", with: "⌥")
            .replacingOccurrences(of: "Shift+", with: "⇧").replacingOccurrences(of: "Ctrl+", with: "⌃")
    }

    /// Whether the command can run now, from the core (`commandsIn(context:)`), asked once
    /// per session revision and context: every menu item of every column reads it on each
    /// render.
    func disabledReason(_ id: String) -> String? {
        let key = "\(session.revision)-\(selection != nil)-\(focus)"
        if key != disabledKey {
            disabledKey = key
            disabled = Dictionary(uniqueKeysWithValues: handle.commandsIn(context: commandContext).map { ($0.id, $0.disabledReason) })
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
            let outcome = try handle.applyIn(commandId: id, args: args, context: commandContext)
            if !outcome.handled { performHost(id, args: args) }
            refresh()
            return true
        } catch let error as ShellCommandError {
            switch error {
            // A bare key outside its context (`wrong_context`) says so in words, briefly.
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
        // The dock closed under the focus: focus is back on the tab.
        if focus == .notesDock, !activeWorkspace.notesDockOpen { focus = .tab }
        refreshWell()
        if paletteOpen { query() }
        syncInk()
    }

    /// A well button: its command, or, for an item that takes a value, the host's question.
    func runWellItem(_ item: ShellToolWellItem) {
        switch item.commandId {
        case "tool.width": cycleInkWidth()
        case "note.link_passage": askForPassageLink()
        default: run(item.commandId)
        }
    }

    private func performHost(_ id: String, args: ShellCommandArgs) {
        switch id {
        case "palette.focus": focusPalette()
        case "view.tool_palette": toolWellCollapsed.toggle()
        case "selection.highlight":
            if let selection { highlight(selection) }
        case "selection.collect":
            if let selection { collect(selection) }
        case "selection.study":
            show(toast: "Study: \(selection.map { ShellReference.short($0.anchors.first?.segmentId ?? "") } ?? "") (no study drawer yet)")
        case "selection.note":
            if let selection { noteOn(selection) }
        case "selection.search":
            if let selection { search(selection.text) }
        case "selection.copy_reference":
            if let selection { copyReference(selection) }
        case "note.bold": notes.toggle(.bold)
        case "note.italic": notes.toggle(.italic)
        case "note.heading": notes.toggleHeading()
        case "note.list": notes.toggleList()
        case "note.link_passage":
            if let reference = args.reference { notes.insertLink(to: reference) } else { askForPassageLink() }
        case "view.collection_drawer", "view.study_drawer", "view.focus_mode", "workspace.export":
            show(toast: "\(command(id)?.title ?? id): not in this preview yet")
        default:
            show(toast: "\(command(id)?.title ?? id) is not performed by this host")
        }
    }

    // MARK: Convenience dispatch for the chrome

    func focusColumn(_ number: UInt32) {
        setFocus(.tab)
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
        ShellPaletteContext(hasSelection: selection != nil, platform: .mac, expanded: expandedSection, focus: focus)
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
        // A tab revealed again opens where it was left (the core's scroll anchor).
        if let segment = session.workspaces.lazy.flatMap(\.columns).flatMap(\.tabs).first(where: { $0.id == tabID })?.scrollSegmentId {
            DispatchQueue.main.async { surface.scroll(toSegment: segment) }
        }
    }

    func surface(for tabID: String) -> ReaderSurface? { surfaces[tabID]?.surface }

    /// Records every shown tab's scroll anchor in the core (`tab.record_scroll`, quiet: no
    /// revision bump, no undo) before a command can hide or rebuild it.
    private func rememberScroll() {
        let ws = activeWorkspace
        for column in ws.columns {
            guard let segment = surfaces[column.shownTabId]?.surface?.topSegmentID,
                  column.tabs.first(where: { $0.id == column.shownTabId })?.scrollSegmentId != segment else { continue }
            _ = try? handle.apply(commandId: "tab.record_scroll",
                                  args: ShellCommandArgs(workspaceId: ws.id, tabId: column.shownTabId, segmentId: segment, scalar: 0),
                                  hasSelection: false)
        }
    }

    /// The end of a rule drag: the left column's share goes to the core (`column.resize`).
    func commitResize(column number: UInt32, share: CGFloat) {
        widthOverrides = [:]
        run("column.resize", args: ShellCommandArgs(column: number, number: Double(share)))
    }

    /// The width dot: the next brush width, back to the narrowest after the widest.
    func cycleInkWidth() {
        if disabledReason("tool.width_up") == nil {
            run("tool.width_up")
        } else {
            run("tool.width", args: ShellCommandArgs(number: 1))
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
        case .word: openWordStudy(selection)
        case .select: break
        }
    }

    var dragMode: ReaderDragMode {
        switch effectiveTool {
        case "highlight": return .highlight
        case "collect": return .collect
        case "word_study": return .word
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

    // MARK: Selection actions and notes (B076)

    /// Note on it: the quoted text and its passage appended to the open note, opening the
    /// dock when no note is open.
    private func noteOn(_ selection: ReaderSelection) {
        let ws = activeWorkspace
        let open = ws.notesDockOpen || ws.columns.contains { column in column.tabs.contains { $0.id == column.shownTabId && $0.kind == "notes" } }
        if !open { run("view.notes_dock") }
        notes.appendQuote(selection.text, reference: selection.reference, to: openNoteID)
    }

    /// The note `selection.note` writes to: the dock's while it is open, else a shown notes
    /// tab's, else the dock's.
    var openNoteID: String {
        let ws = activeWorkspace
        if !ws.notesDockOpen,
           let tab = ws.columns.lazy.compactMap({ column in column.tabs.first { $0.id == column.shownTabId && $0.kind == "notes" } }).first {
            return tab.contentId
        }
        return ws.notesDockNoteId ?? Self.defaultNoteID
    }

    static let defaultNoteID = "fixture:note:1"

    /// Word study: the clicked word opens in a Lexicon tab in the next column to the right,
    /// reusing that column's Lexicon tab, or a new column when there is none.
    private func openWordStudy(_ selection: ReaderSelection) {
        let ws = activeWorkspace
        guard let index = ws.columns.firstIndex(where: { $0.shownTabId == selection.paneID }),
              let source = ws.columns[index].tabs.first(where: { $0.id == selection.paneID }) else { return }
        let entry = LexiconEntry(word: selection.text.trimmingCharacters(in: .whitespacesAndNewlines.union(.punctuationCharacters)),
                                 language: textSource.resource(source.contentId).language,
                                 reference: selection.anchors.first?.segmentId ?? source.reference ?? "")
        guard let tabID = lexiconTab(after: ws.columns[index]) else { return }
        lexiconEntries[tabID] = entry
    }

    /// The Lexicon tab in the column after `column`, shown; nil when none can be opened.
    private func lexiconTab(after column: ShellColumn) -> String? {
        let ws = activeWorkspace
        if let next = ws.columns.first(where: { $0.number == column.number + 1 }),
           let tab = next.tabs.first(where: { $0.kind == "lexicon" }) {
            focusColumn(next.number)
            if next.shownTabId != tab.id { showTab(tab.id) }
            return tab.id
        }
        show(toast: "Lexicon tabs need the core's open command (pending)")
        return nil
    }

    /// Search this: the selected text in the palette field, the palette open.
    private func search(_ text: String) {
        focusPalette()
        paletteText = text
        paletteTextChanged()
    }

    /// Copy reference: the selection's passage in full words on the pasteboard.
    private func copyReference(_ selection: ReaderSelection) {
        let label = ShellReference.humanRange(selection.reference)
        #if canImport(AppKit)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(label, forType: .string)
        #endif
        lastCopied = label
        show(toast: "Copied \(label)")
    }

    /// What Copy reference put on the pasteboard last (the tests read it).
    private(set) var lastCopied: String?

    /// Link passage without a passage: the palette asks for it (`link passage Jn 3:16`).
    func askForPassageLink() {
        focusPalette()
        paletteText = "link passage "
        paletteTextChanged()
    }

    /// A passage link in a note was clicked: the passage opens in the column the note sits
    /// beside (the active column when it reads Bible text, else the first reading column).
    func openPassageLink(_ reference: String) {
        let ws = activeWorkspace
        let reading = ws.columns.first { column in
            column.number == ws.activeColumnNumber && column.tabs.first { $0.id == column.shownTabId }?.kind == "reading"
        } ?? ws.columns.first { column in column.tabs.first { $0.id == column.shownTabId }?.kind == "reading" }
        guard let column = reading else { return show(toast: "No Bible text open for \(ShellReference.human(reference))") }
        setFocus(.tab)
        run("nav.navigate", args: ShellCommandArgs(column: column.number, reference: reference))
    }

    // MARK: Ink

    var inkActive: Bool { ["pen", "highlighter_pen", "eraser"].contains(effectiveTool) }

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
        let tool = effectiveTool
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
        // ⌘B and ⌘I are the note editor's only while it has focus; elsewhere they pass on.
        if binding.commandID.hasPrefix("note."), !notes.editing { return false }
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
