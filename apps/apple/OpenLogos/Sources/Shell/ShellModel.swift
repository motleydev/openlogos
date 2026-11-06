// B072: the shell's host state around the core's `ShellHandle` (specs/22). The session (with
// its reading settings, ink width and swatches, column widths and scroll anchors), the
// registry, apply/undo and the palette query live in the core; this model keeps what only the
// host knows (the palette's field and list, the tool well's place, highlights, ink strokes,
// toasts, the notes being edited) and performs the commands the core returns
// `handled == false` for. B076: the tool well comes from the core for what has focus (a tab,
// the notes dock) and whether text is selected; every command carries that context.
// Every button, menu item and shortcut goes through `run(_:args:)`, which dispatches a
// registered command; the view re-renders from `session` after each. B077: the context menu
// comes from the core for what was right-clicked (ContextMenu.swift); its rows dispatch under
// the menu's context. B074: the core keeps the notes, highlights and ink too, and the session
// is saved to the user store and restored at launch (ShellStore.swift, specs/24).
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
    /// B078: the editions as streams of chapters; B071: the preview pack's where it is
    /// installed, placeholder editions otherwise.
    let reading: ReadingHandle
    /// B071: the Text section over the search packs, off the main thread; nil on the fixtures
    /// (the session handle answers those itself) or with no search packs.
    let textSearch: EditionTextSearch?

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
    /// The notes being edited; the core keeps them (B074).
    let notes = NoteStore()

    // MARK: The user store (B074)

    /// The save policy over the store file; nil when this shell keeps nothing (tests, scripted
    /// runs).
    private(set) var persistence: ShellPersistence?
    /// What the launch restored.
    private(set) var restored: StoreRestore?
    /// Bumped when a tab's stored ink first appears, so its canvas is shown.
    @Published private(set) var inkRevision = 0

    // MARK: Per tab

    @Published private(set) var marks: [String: [ReaderMark]] = [:]
    @Published private(set) var toast: String?
    @Published private(set) var selection: ReaderSelection? {
        didSet { if selection != oldValue { refreshWell() } }
    }
    /// B077: Find in this chapter, painted over the tab's text until Esc.
    @Published var find: FindState?
    /// B077: what the menu was opened on, for the host commands that act on it (a note link
    /// to remove).
    var menuSubject: MenuSubject?
    #if canImport(AppKit)
    /// B077: the chrome's menu catchers, for the keyboard.
    let catchers = MenuCatchers()
    /// The menu built last (the measurement and the tests close it as it opens).
    weak var lastMenu: NSMenu?
    #endif
    /// Column widths while a rule is dragged; the drag's end commits `column.resize`.
    @Published var widthOverrides: [String: CGFloat] = [:]
    /// The core's layout constants (measure caps, pane minimums).
    let layout = layoutConstants()

    private var inkSheets: [String: InkSheet] = [:]
    private var surfaces: [String: WeakSurface] = [:]
    #if os(macOS)
    private var streams: [String: WeakStream] = [:]
    private var streamPool: [String: ReadingStream] = [:]
    private var streamPoolOrder: [String] = []
    #endif
    private var labelsStale = false
    private var toastTask: Task<Void, Never>?
    private var markCounter = 0
    #if canImport(AppKit)
    private var monitor: Any?
    #endif

    init(handle: ShellHandle, textSource: TextSource = FixtureTextSource(), reading: ReadingHandle = .placeholder(), store: UserStoreFile? = nil,
         textSearch: EditionTextSearch? = nil) {
        self.handle = handle
        self.textSource = textSource
        self.reading = reading
        self.textSearch = textSearch
        registry = shellRegistry()
        keymap = ShellKeymap(registry: registry, platform: .mac)
        // Tab views and palette rows name resources by these (`BSB · Jn 3:16`).
        handle.setResourceTitles(titles: textSource.resourceTitles)
        // Original-language tabs get the original tools (Word study).
        handle.setResourceLanguages(languages: textSource.resourceLanguages)
        // B078: linked tabs follow through each edition's versification.
        handle.setResourceSchemes(schemes: reading.schemes())
        // B074: the saved store over the seed session, before anything is shown (specs/24 §5).
        if let store {
            let before = handle.changeCount()
            restored = ShellPersistence.restore(handle, from: store)
            #if os(macOS)
            // File read → first painted text (with no file: the launch's own share).
            if let restored {
                ShellPersistence.paintProbe = (restored.started, handle.session().activeWorkspaceId)
            }
            #endif
            // After a clean restore a save of the same bytes writes nothing.
            let persistence = ShellPersistence(file: store, handle: handle, lastSaved: before, written: restored?.cleanText)
            self.persistence = persistence
        }
        session = handle.session()
        well = handle.toolWell(context: ShellCommandContext(hasSelection: false, focus: .tab))
        notes.model = self
        if let persistence {
            persistence.beforeSave = { [weak self] in self?.notes.flush() }
            persistence.onFailure = { [weak self] words in self?.show(notice: words) }
            // A restore that set something aside saves the repaired store.
            persistence.changed()
        }
        if let notice = restored?.notice { show(notice: notice) }
    }

    /// `pack`: the real editions (B071); without one, the placeholder library and the fixtures'
    /// text search, each of which says what it is.
    convenience init(sessionJSON: String = ShellSeed.json, store: UserStoreFile? = nil, pack: PreviewPack? = nil) {
        // The seed is ours; a seed the core rejects is a programming error.
        let handle = try! ShellHandle(sessionJson: sessionJSON)
        guard let pack else {
            handle.useFixtureText()
            self.init(handle: handle, store: store)
            return
        }
        self.init(handle: handle, textSource: InstalledTextSource(editions: pack.installed), reading: pack.reading, store: store, textSearch: pack.search)
    }

    /// Something the store keeps may have changed: the save debounce restarts when it did.
    func storeChanged() {
        persistence?.changed()
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
        var next = handle.toolWell(context: commandContext)
        if selectionIsPlaceholder {
            for g in next.groups.indices {
                for i in next.groups[g].items.indices where Self.placeholderBlocked.contains(next.groups[g].items[i].commandId) {
                    next.groups[g].items[i].disabledText = Self.placeholderReason
                }
            }
        }
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
        NoteTypography(size: readingSize, lineSpacing: lineSpacing, text: theme.text, accent: theme.accent, rules: theme.rules)
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
    func run(_ id: String, args: ShellCommandArgs = ShellCommandArgs(), context: ShellCommandContext? = nil) -> Bool {
        rememberScroll()
        do {
            if let reason = placeholderRefusal(id) {
                show(toast: reason)
                return false
            }
            let outcome = try handle.applyIn(commandId: id, args: args, context: context ?? commandContext)
            if !outcome.handled { performHost(id, args: args) }
            apply(positions: outcome)
            refresh()
            storeChanged()
            return true
        } catch let error as ShellCommandError {
            switch error {
            // B074: a tab dropped where it was changes nothing and says nothing.
            case .Disabled(_, "already_in_column", _) where id == "tab.move": break
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
        defer { liveLabels.clear() }
        let next = handle.session()
        if next != session { session = next }
        if activeTab?.kind == "reading" { lastReadingColumnID = activeColumn.id }
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
            if !performMenuHost(id, args: args) {
                show(toast: "\(command(id)?.title ?? id) is not performed by this host")
            }
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
        var result = handle.paletteQuery(text: paletteText, context: paletteContext)
        paletteBase = result
        if let textSearch {
            // B071: passages, commands and tabs paint now; the Text section follows from its
            // own handle. The rows of the last keystroke stay until then, so the list does
            // not jump at every letter, unless the field no longer asks for text at all.
            if result.mode == "all" || result.mode == "text" {
                if let shown = paletteTextGroup { result.groups.append(shown) }
                pendingText = textSearch.request(text: paletteText, context: paletteContext) { [weak self] generation, group in
                    self?.textDelivered(generation, group)
                }
            } else {
                textSearch.cancel()
                paletteTextGroup = nil
                pendingText = nil
            }
        }
        palette.result = scoped(result)
        paletteSelection = min(paletteSelection, max(0, paletteRows.count - 1))
    }

    /// The session handle's answer to the last keystroke, before the Text section is merged.
    private var paletteBase: ShellPaletteResult?
    /// The Text rows on show (they may be the previous keystroke's until `pendingText` lands).
    private var paletteTextGroup: ShellPaletteGroup?
    private var pendingText: UInt64?
    /// The column whose About this text panel a scripted run opens (`-OPENLOGOS_ABOUT`).
    @Published var aboutColumn: UInt32?
    /// Called on the main thread once a keystroke's Text section is in `palette.result`
    /// (ShellMeasure stops its clock on the paint after this).
    var onTextDelivered: (() -> Void)?

    private func textDelivered(_ generation: UInt64, _ group: ShellPaletteGroup?) {
        guard generation == pendingText, var result = paletteBase else { return }
        pendingText = nil
        let checked = group.map { checkedAgainstSession($0, target: result) }
        paletteTextGroup = checked
        if let checked { result.groups.append(checked) }
        palette.result = scoped(result)
        paletteSelection = min(paletteSelection, max(0, paletteRows.count - 1))
        onTextDelivered?()
    }

    /// The search handle has a session of one column: whether a row's target column exists is
    /// the real session's to say. The words are the core's: its own passage row for that column.
    private func checkedAgainstSession(_ group: ShellPaletteGroup, target result: ShellPaletteResult) -> ShellPaletteGroup {
        var group = group
        var reason: (kind: String?, words: String?) = (nil, nil)
        if let number = result.targetColumn, !activeWorkspace.columns.contains(where: { $0.number == number }),
           let probe = handle.paletteQuery(text: ": John 3:16 @\(number)", context: paletteContext).groups.first?.items.first {
            reason = (probe.disabledKind, probe.disabledReason)
        }
        for i in group.items.indices {
            group.items[i].disabledKind = reason.kind
            group.items[i].disabledReason = reason.words
        }
        return group
    }

    /// Before a row runs: the Text section for what is typed now, not the last keystroke's.
    private func settlePaletteText() {
        guard pendingText != nil, let answer = textSearch?.settled() else { return }
        textDelivered(answer.generation, answer.group)
    }

    func paletteTextChanged() {
        // The field's onChange repeats a change made in code; the second call is a no-op.
        guard palette.result == nil || paletteText != queriedText else { return }
        // Closing empties the field in code: that change must not open the list again.
        guard palette.open || !paletteText.isEmpty else { return }
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
        settlePaletteText()
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
        palette.scope = nil
        palette.open = false
        palette.text = ""
        palette.expanded = nil
        palette.selection = 0
        palette.result = nil
        textSearch?.cancel()
        paletteBase = nil
        paletteTextGroup = nil
        pendingText = nil
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
        // Tab ids repeat across workspaces (`t1`): a released workspace's surface, updated
        // once more on its way out, must not take the shown tab's place.
        if let tab = activeWorkspace.columns.lazy.flatMap(\.tabs).first(where: { $0.id == tabID }),
           let content = surface.layout?.content.id, !content.hasPrefix("\(tab.contentId)|") { return }
        surfaces[tabID] = WeakSurface(surface: surface)
        // A tab revealed again opens where it was left (the core's scroll anchor).
        if let segment = session.workspaces.lazy.flatMap(\.columns).flatMap(\.tabs).first(where: { $0.id == tabID })?.scrollSegmentId {
            DispatchQueue.main.async { surface.scroll(toSegment: segment) }
        }
    }

    func surface(for tabID: String) -> ReaderSurface? { surfaces[tabID]?.surface }

    // MARK: Continuous reading (B078)

    /// What a scroll moved, ahead of the session.
    let liveLabels = ReadingLabels()

    #if os(macOS)
    /// A reading tab's stream, registered by its view; it also stands as the tab's surface.
    func register(_ stream: ReadingStream) {
        guard streams[stream.tabID]?.stream !== stream else { return }
        // A released workspace's stream, updated once more on its way out, must not take the
        // place of the shown tab with the same id.
        if !stream.workspaceID.isEmpty, stream.workspaceID != session.activeWorkspaceId { return }
        if let tab = activeWorkspace.columns.lazy.flatMap(\.tabs).first(where: { $0.id == stream.tabID }), tab.contentId != stream.resourceID { return }
        streams[stream.tabID] = WeakStream(stream: stream)
        surfaces[stream.tabID] = WeakSurface(surface: stream.surface)
        if let sheet = inkSheets[stream.tabID] { pin(sheet, to: stream) }
        // B074: the core's highlights and ink on its window, now and whenever it changes.
        stream.storedMarks = { [weak self] resource, layout in self?.storedMarks(resourceID: resource, layout: layout) ?? [] }
        // The ink follows a frame later (ReadingScheduler): text first, then its strokes.
        stream.onWindowChanged = { [weak self, weak stream] in
            guard let self, let stream else { return }
            self.scheduleInkSync(stream)
        }
        stream.repaintMarks()
        scheduleInkSync(stream)
    }

    /// The tab's ink moves with its verses: strokes are pinned to the verse under them.
    private func pin(_ sheet: InkSheet, to stream: ReadingStream) {
        sheet.placeAnchor = { [weak stream] y in stream?.inkAnchor(atCanvasY: y) }
        sheet.offset = { [weak stream] anchor in stream?.inkOffset(anchor) }
        stream.onScrolled = { [weak sheet] in sheet?.engine?.syncOffsets() }
        sheet.clipTop = ReadingHeaderView.height
        sheet.engine?.clipTop = ReadingHeaderView.height
        sheet.anchorStrokes()
        sheet.engine?.syncOffsets()
    }

    func stream(for tabID: String) -> ReadingStream? { streams[tabID]?.stream }

    /// A reading tab's stream: the one kept since it was last shown, or a new one. The most
    /// recently shown `streamPoolCap` are kept (each a window of at most eight chapters).
    func pooledStream(for tab: ShellTab) -> ReadingStream {
        let key = "\(session.activeWorkspaceId)|\(tab.id)|\(tab.contentId)"
        if let stream = streamPool[key] {
            streamPoolOrder.removeAll { $0 == key }
            streamPoolOrder.append(key)
            return stream
        }
        let stream = ReadingStream(tabID: tab.id, workspaceID: session.activeWorkspaceId, resourceID: tab.contentId, reading: reading)
        streamPool[key] = stream
        streamPoolOrder.append(key)
        while streamPoolOrder.count > Self.streamPoolCap { streamPool[streamPoolOrder.removeFirst()] = nil }
        return stream
    }

    static let streamPoolCap = 40
    #endif

    /// The verse at the top of a reading tab moved (the reader scrolled): the core moves the
    /// tab's passage and its linked tabs (quiet: no undo, no history). Followers scroll now,
    /// and the tab labels and identity rows follow (`liveLabels`); the session is refreshed
    /// when scrolling settles, so a scroll does not re-render the shell for every verse.
    func recordScroll(tabID: String, segmentID: String, scalar: UInt32 = 0) {
        let ws = activeWorkspace
        let t0 = CACurrentMediaTime()
        defer {
            if followSamples.count < 4096 { followSamples.append((CACurrentMediaTime() - t0) * 1000) }
        }
        guard let outcome = try? handle.apply(commandId: "tab.record_scroll",
                                              args: ShellCommandArgs(workspaceId: ws.id, tabId: tabID, segmentId: segmentID, scalar: scalar),
                                              hasSelection: false),
              !outcome.positions.isEmpty else { return }
        apply(positions: outcome)
        storeChanged()
        recordedScrolls += 1
        // The tab labels and identity rows follow now, through a small object only they
        // observe; the session (and the whole shell) catches up when scrolling stops.
        for position in outcome.positions where position.workspaceId == ws.id {
            liveLabels.set(position.tabId, ReadingLabels.Live(chapterLabel: position.chapterLabel, reference: position.reference))
        }
        labelsStale = true
    }

    /// Scrolling stopped: the identity rows and tab labels catch up.
    func scrollSettled() {
        guard labelsStale else { return }
        labelsStale = false
        refresh()
    }

    /// Hands every tab the core moved to its stream: the scrolled tab learns its new passage,
    /// followers scroll, navigated tabs jump and flash.
    func apply(positions outcome: ShellOutcome) {
        #if os(macOS)
        // Streams are the active workspace's shown tabs (tab ids repeat across workspaces).
        for position in outcome.positions where position.workspaceId == session.activeWorkspaceId {
            if position.driver, !position.scrolled { typedPassage[position.tabId] = position.reference }
            streams[position.tabId]?.stream?.apply(position, revision: outcome.revision)
        }
        #endif
    }

    /// Whether a tab is shown as a continuous stream (it reports its own scrolling).
    func hasStream(_ tabID: String) -> Bool {
        #if os(macOS)
        return streams[tabID]?.stream != nil
        #else
        return false
        #endif
    }

    /// Scroll reports the core moved a passage for (the evidence counts them).
    private(set) var recordedScrolls = 0
    /// How long each scroll report took on the host: the core's record_scroll and the
    /// followers scrolled (the measurement reads them).
    var followSamples: [Double] = []

    /// specs/21's not-in-this-edition state: the core's words when a reading tab's passage is
    /// not in its edition (`John is not in WLC`, `John has no chapter 22`), else nil.
    func readingError(for tab: ShellTab) -> String? {
        guard tab.kind == "reading", let reference = tab.reference else { return nil }
        let key = "\(tab.contentId)|\(reference)"
        // B071: only a passage typed in this tab can be refused for a verse past the chapter's
        // end; a linked tab that followed there stays on its chapter (specs/23 §4), the stream
        // at the last verse there is.
        let followed = tab.linkGroup != nil && typedPassage[tab.id] != reference
        if !followed, let known = readingErrors[key] { return known }
        var reason: String?
        do {
            _ = try reading.locate(resourceId: tab.contentId, input: reference)
        } catch ReadingCallError.Refused(let kind, _) where followed && kind == "verse_out_of_range" {
            return nil
        } catch ReadingCallError.Refused(let kind, let words) {
            reason = kind == "unknown_resource" ? nil : words
        } catch {
            reason = "\(error)"
        }
        if readingErrors.count > 512 { readingErrors.removeAll() }
        readingErrors[key] = reason
        return reason
    }

    private var readingErrors: [String: String?] = [:]
    /// The passage last typed in each tab as the driver of a navigation (`readingError`).
    private var typedPassage: [String: String] = [:]

    /// The fixtures' licence line under a placeholder block whose verses include real fixture
    /// text. An installed edition's attribution is one click away from the identity row, so
    /// it is not repeated under every chapter.
    func attribution(for chapter: ReadingChapter) -> String? {
        guard chapter.placeholder, chapter.verses.contains(where: { !$0.placeholder }),
              let text = textSource.text(resourceID: chapter.resourceId, reference: "\(chapter.chapterKey).1") else { return nil }
        return text.content.attribution
    }

    /// The verses of a passage in a reading edition, joined (B078: from the stream's source).
    func readingText(resourceID: String, from first: String, to last: String) -> (text: String, placeholder: Bool)? {
        let chapter = ShellReference.chapter(of: first)
        guard let block = try? reading.chapterContent(resourceId: resourceID, chapterKey: chapter),
              let start = block.verses.firstIndex(where: { $0.segmentId == first }) else { return nil }
        var verses = Array(block.verses[start...])
        if let end = verses.firstIndex(where: { $0.segmentId == last }) { verses = Array(verses[...end]) } else if first == last { verses = [verses[0]] }
        return (verses.map(\.text).joined(separator: " "), verses.contains { $0.placeholder })
    }

    // MARK: Placeholder text (specs/23 §5)

    /// What a placeholder verse cannot be: quoted, collected, highlighted into a collection,
    /// studied or sent on.
    static let placeholderBlocked: Set<String> = [
        "selection.highlight", "selection.collect", "selection.note", "selection.study", "selection.highlight_in",
        "copy.passage", "tab.open_lexicon", "search.lemma",
    ]
    static let placeholderReason = "Placeholder text can’t be quoted or collected"

    /// Whether the selection (or the word a menu was opened on) touches placeholder text.
    var selectionIsPlaceholder: Bool {
        guard let selection, let layout = surface(for: selection.paneID)?.layout else { return false }
        return selection.anchors.contains { layout.placeholderSegments.contains($0.segmentId) }
    }

    private func placeholderRefusal(_ id: String) -> String? {
        // B074: a highlight in a reading stream is refused by the core (`placeholder_text`).
        if Self.coreRefused.contains(id), let selection, hasStream(selection.paneID) { return nil }
        return Self.placeholderBlocked.contains(id) && selectionIsPlaceholder ? Self.placeholderReason : nil
    }

    static let coreRefused: Set<String> = ["selection.highlight", "selection.highlight_in"]

    /// Records every shown tab's scroll anchor in the core (`tab.record_scroll`, quiet: no
    /// revision bump, no undo) before a command can hide or rebuild it.
    private func rememberScroll() {
        let ws = activeWorkspace
        for column in ws.columns where !hasStream(column.shownTabId) {
            guard let segment = surfaces[column.shownTabId]?.surface?.topSegmentID,
                  column.tabs.first(where: { $0.id == column.shownTabId })?.scrollSegmentId != segment else { continue }
            _ = try? handle.apply(commandId: "tab.record_scroll",
                                  args: ShellCommandArgs(workspaceId: ws.id, tabId: column.shownTabId, segmentId: segment, scalar: 0),
                                  hasSelection: false)
        }
    }

    // MARK: Tab drag (B074)

    /// Where a dragged tab would land: a slot in a column (the slot in the column as drawn
    /// before the move), or a new column with this number.
    struct TabDrop: Equatable {
        var column: UInt32
        var index: UInt32?
        var newColumn = false
    }

    struct TabDrag: Equatable {
        let tabID: String
        var location: CGPoint
        var drop: TabDrop?
    }

    /// The tab being dragged, where the pointer is and where it would drop.
    @Published private(set) var tabDrag: TabDrag?
    /// Tab labels and columns in the columns' coordinate space, as last laid out.
    var tabFrames: [String: CGRect] = [:]
    var columnFrames: [UInt32: CGRect] = [:]
    /// The trailing band of the last column that drops a tab into a new column.
    static let newColumnZone: CGFloat = 56

    func dragTab(_ tabID: String, to location: CGPoint) {
        let next = TabDrag(tabID: tabID, location: location, drop: tabDrop(tabID, at: location))
        if next != tabDrag { tabDrag = next }
    }

    /// The pointer was let go: `tab.move` to the slot under the indicator (undoable).
    func dropTab() {
        guard let drag = tabDrag else { return }
        tabDrag = nil
        if let drop = drag.drop { moveTab(drag.tabID, to: drop) }
    }

    func cancelTabDrag() { tabDrag = nil }

    @discardableResult
    func moveTab(_ tabID: String, to drop: TabDrop) -> Bool {
        var args = ShellCommandArgs(tabId: tabID)
        args.column = drop.column
        if drop.newColumn { args.newColumn = true } else { args.index = drop.index }
        return run("tab.move", args: args)
    }

    /// The slot under a point: past the last column's trailing band a new column; in a column's
    /// tab strip the gap nearest the pointer; elsewhere in a column its end. Nil where a drop
    /// would change nothing.
    func tabDrop(_ tabID: String, at point: CGPoint) -> TabDrop? {
        let ws = activeWorkspace
        guard let last = ws.columns.last, let lastFrame = columnFrames[last.number],
              let from = ws.columns.first(where: { $0.tabs.contains { $0.id == tabID } }),
              let position = from.tabs.firstIndex(where: { $0.id == tabID }) else { return nil }
        if point.x >= lastFrame.maxX - Self.newColumnZone {
            return from.tabs.count > 1 || from.number != last.number ? TabDrop(column: UInt32(ws.columns.count + 1), newColumn: true) : nil
        }
        guard let column = ws.columns.first(where: { columnFrames[$0.number].map { point.x >= $0.minX && point.x < $0.maxX } ?? false }) else { return nil }
        var index = column.tabs.count
        if let strip = column.tabs.first.flatMap({ tabFrames[$0.id] }), point.y <= strip.maxY + ShellTokens.Spacing.xs {
            index = column.tabs.filter { (tabFrames[$0.id]?.midX ?? .infinity) < point.x }.count
        }
        if column.id == from.id, index == position || index == position + 1 { return nil }
        return TabDrop(column: column.number, index: UInt32(index))
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
        if mode != .select, !(mode == .highlight && hasStream(selection.paneID)), let layout = surface(for: selection.paneID)?.layout,
           selection.anchors.contains(where: { layout.placeholderSegments.contains($0.segmentId) }) {
            return show(toast: Self.placeholderReason)
        }
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

    func highlight(_ selection: ReaderSelection, in color: ShellTokens.Swatch? = nil) {
        #if os(macOS)
        // B074: in a reading stream the core keeps it, as segment spans in Unicode scalars;
        // it refuses placeholder text.
        if let stream = stream(for: selection.paneID), let layout = stream.surface.layout, !layout.blocks.isEmpty {
            let spans = Self.merged(selection.highlights.flatMap(layout.spans(ofDisplay:))).compactMap(layout.storeSpan)
            guard !spans.isEmpty else { return }
            let highlight = StoreHighlight(resourceId: stream.resourceID, contentRevision: layout.content.contentRevision, spans: spans,
                                           color: ShellTokens.hex((color ?? swatch).rgb))
            do {
                _ = try handle.addHighlight(highlight: highlight, reading: reading)
            } catch StoreCallError.Refused(let kind, let reason) {
                return show(toast: kind == "placeholder_text" ? Self.placeholderReason : reason)
            } catch {
                return show(toast: "\(error)")
            }
            self.selection = nil
            repaintHighlights(resourceID: stream.resourceID)
            storeChanged()
            return
        }
        #endif
        markCounter += 1
        var mark = ReaderMark(id: "mark-\(markCounter)", ranges: selection.highlights, color: (color ?? swatch).platformColor)
        // B078: kept as segment spans, so it stays on its verses in a continuous window.
        if let layout = surface(for: selection.paneID)?.layout, !layout.blocks.isEmpty {
            mark.spans = selection.highlights.flatMap(layout.spans(ofDisplay:))
        }
        marks[selection.paneID, default: []].append(mark)
        self.selection = nil
    }

    /// One span per segment: a selection split around verse numbers gives several.
    static func merged(_ spans: [SegmentSpan]) -> [SegmentSpan] {
        var out: [SegmentSpan] = []
        for span in spans {
            if let last = out.last, last.segmentId == span.segmentId {
                let start = min(last.start, span.start), end = max(last.start + last.length, span.start + span.length)
                out[out.count - 1] = SegmentSpan(segmentId: span.segmentId, start: start, length: end - start)
            } else {
                out.append(span)
            }
        }
        return out
    }

    /// The core's highlights on the chapters a window shows, as marks (oldest first, so later
    /// ones paint over earlier ones).
    func storedMarks(resourceID: String, layout: ReaderLayout) -> [ReaderMark] {
        #if os(macOS)
        let t0 = CACurrentMediaTime()
        defer { ShellPersistence.cost("highlights", since: t0) }
        #endif
        var seen: Set<String> = []
        var marks: [ReaderMark] = []
        for block in layout.blocks {
            for highlight in handle.highlightsIn(resourceId: resourceID, chapterKey: block.chapterKey) where seen.insert(highlight.id).inserted {
                let spans = highlight.spans.compactMap(layout.segmentSpan)
                guard !spans.isEmpty else { continue }
                marks.append(ReaderMark(id: highlight.id, ranges: [], color: ShellTokens.swatch(hex: highlight.color).platformColor, spans: spans))
            }
        }
        return marks
    }

    /// The highlights a tab paints: the core's on a reading stream's window, else the host's
    /// on its single passage.
    func highlights(_ tabID: String) -> [ReaderMark] {
        #if os(macOS)
        if let stream = stream(for: tabID), let layout = stream.surface.layout, !layout.blocks.isEmpty {
            return storedMarks(resourceID: stream.resourceID, layout: layout)
        }
        #endif
        return marks[tabID] ?? []
    }

    /// Every shown stream of the edition paints the core's highlights again.
    func repaintHighlights(resourceID: String) {
        #if os(macOS)
        for stream in streams.values.compactMap(\.stream) where stream.resourceID == resourceID { stream.repaintMarks() }
        #endif
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

    /// Word study: the clicked word opens in the Lexicon tab of the next column to the right
    /// (`tab.open_lexicon` reuses it, or opens a new column). The core keeps the word on the
    /// tab (`lexiconWord`) with the verse; focus stays on the clicked column, so Word study
    /// goes on there.
    private func openWordStudy(_ selection: ReaderSelection) {
        let ws = activeWorkspace
        guard let column = ws.columns.first(where: { $0.shownTabId == selection.paneID }) else { return }
        let word = selection.text.trimmingCharacters(in: .whitespacesAndNewlines.union(.punctuationCharacters))
        guard !word.isEmpty else { return }
        focusColumn(column.number)
        run("tab.open_lexicon", args: ShellCommandArgs(text: word, reference: selection.anchors.first?.segmentId))
    }

    /// Search this: the selected text in the palette field, the palette open.
    func search(_ text: String) {
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
    var lastCopied: String?

    /// Link passage without a passage: the palette asks for it (`link passage Jn 3:16`).
    func askForPassageLink() {
        focusPalette()
        paletteText = "link passage "
        paletteTextChanged()
    }

    /// The column whose Bible text last had focus (passage links in notes move it).
    private(set) var lastReadingColumnID: String?

    /// A passage link in a note was clicked: the passage opens in the last-focused reading
    /// column (column 1 when none has had focus).
    func openPassageLink(_ reference: String) {
        let ws = activeWorkspace
        let isReading = { (column: ShellColumn) in column.tabs.first { $0.id == column.shownTabId }?.kind == "reading" }
        let reading = ws.columns.first { $0.id == lastReadingColumnID && isReading($0) }
            ?? ws.columns.first { $0.number == 1 && isReading($0) } ?? ws.columns.first(where: isReading)
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
        #if os(macOS)
        sheet.onCommit = { [weak self, weak sheet] stroke in
            guard let self, let sheet else { return nil }
            return self.storeInk(stroke, tabID: tabID, sheet: sheet)
        }
        sheet.onErase = { [weak self] id in self?.eraseInk(id, tabID: tabID) }
        if let stream = stream(for: tabID) { pin(sheet, to: stream) }
        #endif
        return sheet
    }

    #if os(macOS)
    // MARK: Ink in the core (B074)

    /// A finished stroke on a reading stream goes to the core, pinned to the verse under its
    /// first point: `x` from the text's leading edge (mirrored for right-to-left), `y` from
    /// that verse's top. The sheet keeps it
    /// under the core's id, pinned to that verse as B078 pins a stroke.
    private func storeInk(_ stroke: InkStroke, tabID: String, sheet: InkSheet) -> (InkStroke, InkSheet.Anchor)? {
        guard let stream = stream(for: tabID), let first = stroke.points.first,
              let anchor = stream.inkAnchor(atCanvasY: CGFloat(first.y)) else { return nil }
        var pinned = stroke
        pinned.points = stroke.points.map { var p = $0; p.x = stream.inkStoredX(p.x); p.y -= Double(anchor.y); return p }
        let id: String
        do {
            id = try handle.addInk(mark: StoreInkMark(resourceId: stream.resourceID, segmentId: anchor.segment, stroke: pinned))
        } catch {
            show(toast: "\(error)")
            return nil
        }
        storeChanged()
        inkGeneration += 1
        var kept = stroke
        kept.id = id
        DispatchQueue.main.async { [weak self] in self?.syncStoredInk(resourceID: stream.resourceID, except: tabID) }
        return (kept, anchor)
    }

    /// The eraser took a stroke: the core forgets it, and every tab of the edition drops it.
    private func eraseInk(_ id: String, tabID: String) {
        guard let stream = stream(for: tabID), (try? handle.removeInk(id: id)) != nil else { return }
        erasedInk.insert(id)
        inkGeneration += 1
        storeChanged()
        syncStoredInk(resourceID: stream.resourceID, except: tabID)
    }

    /// Strokes the eraser took this session (other tabs of the edition drop them too).
    private var erasedInk: Set<String> = []
    /// Bumped by every stroke added or erased: each stream then takes all its chapters again;
    /// otherwise a window change asks the core only for the chapters it has not seen.
    private var inkGeneration = 0
    private var inkSynced: [String: (generation: Int, chapters: Set<String>)] = [:]

    /// Every shown stream of the edition takes the core's strokes again.
    private func syncStoredInk(resourceID: String, except tabID: String? = nil) {
        for stream in streams.values.compactMap(\.stream) where stream.resourceID == resourceID && stream.tabID != tabID {
            syncStoredInk(stream)
        }
    }

    private func scheduleInkSync(_ stream: ReadingStream) {
        ShellPersistence.whenTextIsOnScreen { [weak self, weak stream] in
            guard let stream else { return }
            ReadingScheduler.shared.enqueue("ink-\(ObjectIdentifier(stream).hashValue)") { [weak self, weak stream] in
                if let self, let stream { self.syncStoredInk(stream) }
                return false
            }
        }
    }

    /// A stream's window changed: its sheet shows the core's strokes on the chapters in it.
    func syncStoredInk(_ stream: ReadingStream) {
        guard streams[stream.tabID]?.stream === stream, let layout = stream.surface.layout else { return }
        let t0 = CACurrentMediaTime()
        defer { ShellPersistence.cost("ink", since: t0) }
        let key = "\(stream.workspaceID)|\(stream.tabID)|\(stream.resourceID)"
        var synced = inkSynced[key].flatMap { $0.generation == inkGeneration ? $0 : nil } ?? (generation: inkGeneration, chapters: [])
        let chapters = layout.blocks.map(\.chapterKey).filter { !synced.chapters.contains($0) }
        guard !chapters.isEmpty else { return }
        synced.chapters.formUnion(chapters)
        inkSynced[key] = synced
        var strokes: [(InkStroke, InkSheet.Anchor)] = []
        for chapter in chapters {
            for mark in handle.inkIn(resourceId: stream.resourceID, chapterKey: chapter) {
                var stroke = mark.stroke
                stroke.id = mark.id
                stroke.points = stroke.points.map { var p = $0; p.x = stream.inkCanvasX(p.x); return p }
                strokes.append((stroke, InkSheet.Anchor(segment: mark.segmentId, y: 0)))
            }
        }
        if strokes.isEmpty && inkSheets[stream.tabID] == nil { return }
        let sheet = inkSheet(for: stream.tabID)
        let wasEmpty = sheet.liveStrokes.isEmpty
        sheet.syncStored(strokes, erased: erasedInk)
        if wasEmpty != sheet.liveStrokes.isEmpty {
            DispatchQueue.main.async { [weak self] in self?.inkRevision += 1 }
        }
    }
    #endif

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

    func show(toast text: String, seconds: Double = 2.4) {
        toast = text
        toastTask?.cancel()
        toastTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
            guard !Task.isCancelled else { return }
            self?.toast = nil
        }
    }

    /// B074: said once at launch (a file that could not be restored) or when saves keep
    /// failing; it stays up longer than a toast.
    func show(notice text: String) {
        lastNotice = text
        show(toast: text, seconds: 6)
    }

    /// The last notice (the tests and the evidence read it).
    private(set) var lastNotice: String?

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
            guard let self else { return event }
            // B077: the menu key, and ⌘G / ⇧⌘G / Esc while Find in this chapter is showing.
            if self.handleMenuKey(event) { return nil }
            guard let chord = KeyChord(event: event) else { return event }
            return self.handle(chord: chord, inTextField: self.textFieldHasFocus) ? nil : event
        }
    }

    func removeKeyMonitor() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }
    #endif
}

/// B078: tab labels and identity rows a scroll moved, by tab, until the session is refreshed.
@MainActor
final class ReadingLabels: ObservableObject {
    struct Live: Equatable {
        let chapterLabel: String?
        let reference: String
    }

    @Published private(set) var byTab: [String: Live] = [:]

    func set(_ tabID: String, _ live: Live) {
        if byTab[tabID] != live { byTab[tabID] = live }
    }

    func clear() {
        if !byTab.isEmpty { byTab = [:] }
    }
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
    /// B077: the scope a menu's Search put on the Text section, shown as a chip.
    @Published var scope: SearchScope?
}

private struct WeakSurface {
    weak var surface: ReaderSurface?
}

#if os(macOS)
private struct WeakStream {
    weak var stream: ReadingStream?
}
#endif

extension Array {
    subscript(safe index: Int) -> Element? { indices.contains(index) ? self[index] : nil }
}
