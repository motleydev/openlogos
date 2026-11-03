// B077: the OpenLogos context menu (specs/22 §4.2). The core decides the menu for what was
// right-clicked (`contextMenu(target:context:)`): a word in reading text (selected first, as
// macOS does), a selection, note text or a passage link in a note, a tab, a column's identity
// row, or the empty area of a column. The host builds the target from the hit, draws the menu
// as a native NSMenu (a header with the word, sections between separators, one-level
// submenus, key equivalents shown, disabled rows with their reason as subtitle and tooltip),
// and on a choice applies the menu's `focus` first, then the row under the menu's `context`.
// ⌥-right-click keeps the platform menu. The keyboard opens the same menu: ⌃-click (AppKit
// sends it as a right-click), VoiceOver's VO-⇧-M and a PC keyboard's menu key. The host
// performs the commands the core hands back: searches (the palette's Text section with a
// scope chip; Find in this chapter paints matches in the column), copies, clipboard edits,
// Paste passage, Highlight in a swatch and Remove link.
import SwiftUI
#if canImport(AppKit)
import AppKit
#endif

/// A scope a menu's Search put on the palette's Text section, shown as a chip by the field.
struct SearchScope: Equatable {
    /// `edition`, `all`, `workspace` (`chapter` is Find in this chapter, never a chip).
    let scope: String
    var resourceID: String?
    /// The chip: `This edition · BSB`, `All editions`, `This workspace`.
    let label: String
}

/// Find in this chapter: the matches of a word in one tab's text, the current one revealed.
struct FindState: Equatable {
    let tabID: String
    let text: String
    /// `John 3`.
    let label: String
    let ranges: [NSRange]
    var index: Int
}

/// What a menu was opened on that a host command acts on later (the note link to remove).
struct MenuSubject {
    weak var note: AnyObject?
    var range: NSRange?
}

extension ShellReference {
    /// The core's references (`John.3.16`, `John.3`, `John.3.16-John.3.17`) in full words:
    /// `John 3:16`, `John 3`, `John 3:16–17`.
    static func humanOsis(_ osis: String?) -> String {
        guard let osis else { return "" }
        let parts = osis.split(separator: "-", maxSplits: 1).map(String.init)
        guard parts.count == 2 else { return human(osis) }
        let start = parts[0].split(separator: "."), end = parts[1].split(separator: ".")
        if start.count == 3, end.count == 3, start[0] == end[0], start[1] == end[1] {
            return "\(human(parts[0]))–\(end[2])"
        }
        return "\(human(parts[0]))–\(human(parts[1]))"
    }
}

extension ShellModel {
    // MARK: The menu

    /// The core's menu for a target, or nil (a toast says why) for a target that is gone.
    func contextMenu(for target: ShellContextTarget) -> ShellContextMenu? {
        do {
            return try handle.contextMenu(target: target, context: commandContext)
        } catch ShellCommandError.InvalidDocument(let detail) {
            show(toast: detail)
        } catch {
            show(toast: "\(error)")
        }
        return nil
    }

    /// A row was chosen: the focus change a click on the target would have made, then the row
    /// under the menu's context. Dismissing the menu runs neither.
    func choose(_ menu: ShellContextMenu, commandId: String, args: ShellCommandArgs) {
        setFocus(menu.context.focus)
        if let focus = menu.focus { run(focus.commandId, args: focus.args) }
        run(commandId, args: args, context: menu.context)
    }

    /// The target of a hit in a reading tab's text.
    func target(for hit: ReaderContextHit, tab: ShellTab, column: ShellColumn) -> ShellContextTarget {
        switch hit {
        case .word(let selection):
            let anchor = selection.anchors.first
            let word = selection.text.trimmingCharacters(in: .whitespacesAndNewlines.union(.punctuationCharacters))
            return .word(text: word, reference: anchor?.segmentId, segmentId: anchor?.segmentId,
                         scalarStart: anchor?.range.start, scalarEnd: anchor?.range.end, resourceId: tab.contentId, tabId: tab.id)
        case .selection(let selection):
            let first = selection.anchors.first?.segmentId, last = selection.anchors.last?.segmentId
            return .selection(text: selection.text, reference: first, endReference: last != first ? last : nil,
                              resourceId: tab.contentId, tabId: tab.id)
        case .background:
            return .background(column: column.number)
        }
    }

    // MARK: Host commands

    /// Performs a B077 host command; false for one this host does not know.
    func performMenuHost(_ id: String, args: ShellCommandArgs) -> Bool {
        switch id {
        case "search.text":
            let text = args.text ?? ""
            switch args.scope ?? "all" {
            case "chapter": findInChapter(text, reference: args.reference)
            case "edition":
                let name = args.resourceId.map { textSource.resource($0).shortName } ?? ""
                openSearch(text, scope: SearchScope(scope: "edition", resourceID: args.resourceId, label: "This edition · \(name)"))
            case "workspace": openSearch(text, scope: SearchScope(scope: "workspace", label: "This workspace"))
            default: openSearch(text, scope: SearchScope(scope: "all", label: "All editions"))
            }
        case "copy.text":
            let text = args.text ?? ""
            copy(text, toast: "Copied “\(text)”")
        case "copy.reference":
            let label = ShellReference.humanOsis(args.reference)
            copy(label, toast: "Copied \(label)")
        case "copy.passage":
            let label = ShellReference.humanOsis(args.reference)
            let text = args.text ?? passageText(resourceID: args.resourceId, reference: args.reference)
            copy(Self.passageClipboard(text: text, reference: label, edition: args.resourceId.map { textSource.resource($0).shortName }),
                 toast: "Copied \(label) with its text")
        case "host.cut", "host.copy", "host.paste":
            clipboardEdit(id)
        case "nav.paste_passage":
            pastePassage()
        case "selection.highlight_in":
            if let selection { highlight(selection, in: ShellTokens.swatch(hex: args.text ?? session.ink.swatch)) }
        case "note.remove_link":
            removeNoteLink(args.reference)
        default:
            return false
        }
        return true
    }

    /// `“For God so loved the world” — John 3:16 (BSB)`: one line, the shape Note on it quotes in.
    static func passageClipboard(text: String, reference: String, edition: String?) -> String {
        let line = text.split(whereSeparator: \.isNewline).joined(separator: " ").trimmingCharacters(in: .whitespaces)
        return "“\(line)” — \(reference)\(edition.map { " (\($0))" } ?? "")"
    }

    /// The verses of a passage (`John.3.16`, `John.3.16-John.3.17`) in an edition, joined.
    func passageText(resourceID: String?, reference: String?) -> String {
        guard let resourceID, let reference else { return "" }
        let ends = reference.split(separator: "-", maxSplits: 1).map(String.init)
        guard let verses = textSource.text(resourceID: resourceID, reference: ends[0])?.content.verses,
              let first = verses.firstIndex(where: { $0.id == ends[0] }) else { return "" }
        let last = ends.count == 2 ? verses.firstIndex(where: { $0.id == ends[1] }) ?? first : first
        return verses[first...max(first, last)].map(\.text).joined(separator: " ")
    }

    private func copy(_ text: String, toast: String) {
        #if canImport(AppKit)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        #endif
        lastCopied = text
        show(toast: toast)
    }

    /// Search “word” in this edition / in all editions: the palette's Text section (`#word`)
    /// with the scope as a chip.
    func openSearch(_ text: String, scope: SearchScope) {
        focusPalette()
        palette.scope = scope
        paletteText = "#\(text)"
        paletteTextChanged()
    }

    /// The scope chip's ×: the Text section over every edition again.
    func clearSearchScope() {
        palette.scope = nil
        query()
    }

    /// The palette's result with the scope applied to its Text section.
    func scoped(_ result: ShellPaletteResult) -> ShellPaletteResult {
        guard let scope = palette.scope, scope.scope != "all" else { return result }
        let names: Set<String>
        switch scope.scope {
        case "edition": names = Set(scope.resourceID.map { [textSource.resource($0).shortName] } ?? [])
        default: names = Set(activeWorkspace.columns.flatMap(\.tabs).map { textSource.resource($0.contentId).shortName })
        }
        var result = result
        result.groups = result.groups.compactMap { group in
            guard group.section == .text else { return group }
            var group = group
            group.items = group.items.filter { item in names.contains { item.title.hasPrefix("\($0) ·") } }
            group.total = UInt32(group.items.count)
            return group.items.isEmpty ? nil : group
        }
        return result
    }

    // MARK: Find in this chapter

    /// Paints every match of the word in the active tab's text, reveals the first and says
    /// how many there are. ⌘G / ⇧⌘G move, Esc clears.
    func findInChapter(_ text: String, reference: String?) {
        guard let tab = activeTab, let surface = surface(for: tab.id), let layout = surface.layout, !text.isEmpty else { return }
        let string = layout.display.string as NSString
        var ranges: [NSRange] = []
        var search = NSRange(location: 0, length: string.length)
        while true {
            let found = string.range(of: text, options: [.caseInsensitive], range: search)
            guard found.location != NSNotFound else { break }
            ranges.append(found)
            search = NSRange(location: NSMaxRange(found), length: string.length - NSMaxRange(found))
        }
        let label = ShellReference.humanOsis(reference ?? tab.reference.map(ShellReference.chapter(of:)))
        find = FindState(tabID: tab.id, text: text, label: label, ranges: ranges, index: 0)
        if let first = ranges.first { surface.reveal(display: first) }
        show(toast: ranges.isEmpty ? "No matches for “\(text)” in \(label)" : "\(ranges.count) \(ranges.count == 1 ? "match" : "matches") in \(label)")
    }

    func findNext(_ delta: Int) {
        guard var state = find, !state.ranges.isEmpty else { return }
        state.index = (state.index + delta + state.ranges.count) % state.ranges.count
        find = state
        surface(for: state.tabID)?.reveal(display: state.ranges[state.index])
        show(toast: "\(state.index + 1) of \(state.ranges.count) in \(state.label)")
    }

    func clearFind() {
        find = nil
    }

    /// A tab's highlights with Find's matches over them (the current one stronger).
    func readerMarks(_ tabID: String) -> [ReaderMark] {
        let own = marks[tabID] ?? []
        guard let find, find.tabID == tabID else { return own }
        let accent = PlatformColor(hex: theme.accent)
        return own + find.ranges.enumerated().map { index, range in
            ReaderMark(id: "find-\(index)", ranges: [range], color: accent.withAlphaComponent(index == find.index ? 0.45 : 0.2))
        }
    }

    // MARK: Clipboard and notes

    private func clipboardEdit(_ id: String) {
        #if canImport(AppKit)
        if let note = menuSubject?.note as? NoteTextView ?? (notes.editing ? notes.active : nil) {
            switch id {
            case "host.cut": note.cut(nil)
            case "host.copy": note.copy(nil)
            default: note.paste(nil)
            }
            return
        }
        if id == "host.copy", let selection {
            copy(selection.text, toast: "Copied")
        }
        #endif
    }

    /// Paste passage: a passage read off the clipboard opens in the active tab.
    private func pastePassage() {
        #if canImport(AppKit)
        let text = NSPasteboard.general.string(forType: .string) ?? ""
        #else
        let text = ""
        #endif
        guard let start = (try? parseReference(text: text.trimmingCharacters(in: .whitespacesAndNewlines)))?.ranges.first?.start, let osis = try? formatOsis(reference: start) else {
            return show(toast: "No passage on the clipboard")
        }
        if run("nav.navigate", args: ShellCommandArgs(reference: osis)) {
            show(toast: "Opened \(ShellReference.humanOsis(osis))")
        }
    }

    private func removeNoteLink(_ reference: String?) {
        #if canImport(AppKit)
        guard let note = menuSubject?.note as? NoteTextView, let range = menuSubject?.range, let storage = note.textStorage,
              NSMaxRange(range) <= storage.length else { return }
        note.edit(range) { storage.removeAttribute(.link, range: range) }
        show(toast: "Removed link to \(ShellReference.humanOsis(reference))")
        #endif
    }

    // MARK: Keyboard

    #if canImport(AppKit)
    /// The PC keyboard's menu key, and ⌘G / ⇧⌘G / Esc while Find in this chapter shows.
    func handleMenuKey(_ event: NSEvent) -> Bool {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        switch event.keyCode {
        case Self.menuKeyCode:
            openKeyboardMenu()
            return true
        case 5 where find != nil && flags.contains(.command) && !flags.contains(.option):
            findNext(flags.contains(.shift) ? -1 : 1)
            return true
        case 53 where find != nil && !paletteOpen:
            clearFind()
            return true
        default:
            return false
        }
    }

    /// kVK_ContextualMenu.
    static let menuKeyCode: UInt16 = 0x6E

    /// The menu for what has focus: the selection or the caret's word in reading text, the
    /// caret in a note, else the focused tab, else the active column's background.
    func openKeyboardMenu() {
        let window = catchers.view("background-\(activeColumn.number)")?.window ?? NSApp.keyWindow
        let responder = window?.firstResponder
        if let note = responder as? NoteTextView, note.showKeyboardMenu() { return }
        if let reader = responder as? ReaderNSTextView, reader.surface?.onContextMenu != nil, reader.accessibilityPerformShowMenu() { return }
        if let tab = activeTab, catchers.popUp("tab-\(tab.id)") { return }
        catchers.popUp("background-\(activeColumn.number)")
    }

    // MARK: Drawing

    /// The menu for a target as an NSMenu, or nil when the core refused it.
    func nsMenu(for target: ShellContextTarget, subject: MenuSubject? = nil) -> NSMenu? {
        guard let menu = contextMenu(for: target) else { return nil }
        menuSubject = subject
        let built = ContextMenuBuilder.build(menu, model: self)
        lastMenu = built
        return built
    }

    /// A reading tab's right-click.
    func nsMenu(for hit: ReaderContextHit, tab: ShellTab, column: ShellColumn) -> NSMenu? {
        nsMenu(for: target(for: hit, tab: tab, column: column))
    }

    /// A note's right-click: a passage link, or a spot or selection.
    func nsMenu(for hit: NoteContextHit, editor: NoteTextView) -> NSMenu? {
        let tabID = editor.paneID == "dock" ? nil : editor.paneID
        switch hit {
        case .link(let reference, let range):
            return nsMenu(for: .noteLink(reference: reference, tabId: tabID), subject: MenuSubject(note: editor, range: range))
        case .text(let text):
            return nsMenu(for: .noteText(text: text, tabId: tabID), subject: MenuSubject(note: editor))
        }
    }
    #else
    /// iPad keeps the platform's edit menu until B075.
    func nsMenu(for hit: ReaderContextHit, tab: ShellTab, column: ShellColumn) -> ReaderPlatformMenu? { nil }
    #endif
}

#if canImport(AppKit)
/// What a right-click in a note landed on.
enum NoteContextHit: Equatable {
    case link(String, NSRange)
    case text(String?)
}

/// The NSMenu of one core menu; it keeps the dispatcher its items target alive.
final class ShellNSMenu: NSMenu {
    var dispatcher: MenuDispatcher?
    var shellMenu: ShellContextMenu? { dispatcher?.menu }
}

/// A row's command and arguments.
final class MenuChoice: NSObject {
    let commandId: String
    let args: ShellCommandArgs
    init(commandId: String, args: ShellCommandArgs) {
        self.commandId = commandId
        self.args = args
    }
}

@MainActor
final class MenuDispatcher: NSObject {
    weak var model: ShellModel?
    let menu: ShellContextMenu

    init(model: ShellModel, menu: ShellContextMenu) {
        self.model = model
        self.menu = menu
    }

    @objc func choose(_ sender: NSMenuItem) {
        guard let choice = sender.representedObject as? MenuChoice else { return }
        model?.choose(menu, commandId: choice.commandId, args: choice.args)
    }
}

/// Draws a core menu as an NSMenu.
@MainActor
enum ContextMenuBuilder {
    /// Core icon names beyond the well's, as SF Symbols.
    static let symbols: [String: String] = [
        "search-all": "magnifyingglass.circle",
        "lemma": "textformat.abc",
        "find": "text.magnifyingglass",
        "compare": "rectangle.split.2x1",
        "open": "arrow.up.forward.square",
        "column": "rectangle.portrait",
        "new-column": "rectangle.portrait.badge.plus",
        "cross-references": "arrow.triangle.branch",
        "copy": "doc.on.doc",
        "copy-passage": "doc.on.clipboard",
        "cut": "scissors",
        "paste": "clipboard",
        "remove-link": "personalhotspot.slash",
        "close": "xmark",
        "close-others": "xmark.square",
        "duplicate": "plus.square.on.square",
        "move": "arrow.right.square",
        "pin": "pin",
        "follow": "arrow.triangle.2.circlepath",
        "link": "link",
        "focus": "scope",
        "move-left": "arrow.left",
        "move-right": "arrow.right",
        "new-tab": "plus.square",
        "equalize": "rectangle.split.3x1",
        "close-column": "rectangle.portrait.slash",
        "paste-passage": "list.clipboard",
        "text-size": "textformat.size",
        "text-larger": "textformat.size.larger",
        "text-smaller": "textformat.size.smaller",
        "text-reset": "textformat",
        "theme": "circle.lefthalf.filled",
    ]

    static func symbol(_ icon: String) -> String { symbols[icon] ?? ToolWell.symbol(icon) }

    static func build(_ menu: ShellContextMenu, model: ShellModel) -> ShellNSMenu {
        let ns = ShellNSMenu(title: menu.title ?? "")
        ns.autoenablesItems = false
        let dispatcher = MenuDispatcher(model: model, menu: menu)
        ns.dispatcher = dispatcher
        if let title = menu.title { ns.addItem(.sectionHeader(title: title)) }
        for (index, section) in menu.sections.enumerated() {
            if index > 0 { ns.addItem(.separator()) }
            for row in section.items {
                let item = self.item(commandId: row.commandId, args: row.args, label: row.label, key: row.key, icon: row.icon,
                                     active: row.active, value: row.value, disabledText: row.disabledText, dispatcher: dispatcher)
                if !row.submenu.isEmpty {
                    // Mac submenus just open; the parent's command is for hosts that run it on click.
                    item.action = nil
                    item.target = nil
                    item.keyEquivalent = ""
                    let submenu = NSMenu(title: row.label)
                    submenu.autoenablesItems = false
                    for sub in row.submenu {
                        submenu.addItem(self.item(commandId: sub.commandId, args: sub.args, label: sub.label, key: sub.key, icon: sub.icon,
                                                  active: sub.active, value: sub.value, disabledText: sub.disabledText, dispatcher: dispatcher))
                    }
                    item.submenu = submenu
                }
                ns.addItem(item)
            }
        }
        return ns
    }

    static func item(commandId: String, args: ShellCommandArgs, label: String, key: String?, icon: String, active: Bool,
                     value: String?, disabledText: String?, dispatcher: MenuDispatcher) -> NSMenuItem {
        let item = NSMenuItem(title: label, action: #selector(MenuDispatcher.choose(_:)), keyEquivalent: "")
        item.target = dispatcher
        item.representedObject = MenuChoice(commandId: commandId, args: args)
        item.identifier = NSUserInterfaceItemIdentifier(commandId)
        if let key, let (equivalent, mask) = keyEquivalent(key) {
            item.keyEquivalent = equivalent
            item.keyEquivalentModifierMask = mask
        }
        item.image = value.map(swatchImage) ?? NSImage(systemSymbolName: symbol(icon), accessibilityDescription: nil)
        item.state = active ? .on : .off
        if let reason = disabledText {
            item.isEnabled = false
            item.subtitle = reason
            item.toolTip = reason
        }
        return item
    }

    /// `Alt+Shift+1` → (`1`, ⌥⇧), `Cmd+Shift+C` → (`c`, ⌘⇧).
    static func keyEquivalent(_ key: String) -> (String, NSEvent.ModifierFlags)? {
        guard let chord = KeyChord(registry: key), chord.key.count == 1 else { return nil }
        var mask: NSEvent.ModifierFlags = []
        if chord.command { mask.insert(.command) }
        if chord.option { mask.insert(.option) }
        if chord.shift { mask.insert(.shift) }
        if chord.control { mask.insert(.control) }
        return (chord.key.lowercased(), mask)
    }

    /// A swatch row's dot.
    static func swatchImage(_ hex: String) -> NSImage {
        let color = ShellTokens.swatch(hex: hex).platformColor.withAlphaComponent(1)
        return NSImage(size: NSSize(width: 14, height: 14), flipped: false) { rect in
            color.setFill()
            NSBezierPath(ovalIn: rect.insetBy(dx: 1.5, dy: 1.5)).fill()
            NSColor.black.withAlphaComponent(0.2).setStroke()
            NSBezierPath(ovalIn: rect.insetBy(dx: 1.5, dy: 1.5)).stroke()
            return true
        }
    }
}

// MARK: - SwiftUI chrome

/// A shell's catchers on screen by key (`tab-t1`, `column-2`, `background-2`), for the
/// keyboard.
@MainActor
final class MenuCatchers {
    private struct Weak { weak var view: MenuCatcherView? }
    private var views: [String: Weak] = [:]

    func register(_ view: MenuCatcherView) { views[view.key] = Weak(view: view) }
    func view(_ key: String) -> MenuCatcherView? { views[key]?.view.flatMap { $0.window == nil ? nil : $0 } }

    /// Opens the catcher's menu at its leading bottom corner; false when it is not on screen.
    @discardableResult
    func popUp(_ key: String) -> Bool {
        guard let view = view(key), let menu = view.provide() else { return false }
        menu.popUp(positioning: nil, at: NSPoint(x: 0, y: view.isFlipped ? view.bounds.maxY : 0), in: view)
        return true
    }
}

/// An invisible view over a piece of SwiftUI chrome that answers right-clicks (and ⌃-clicks)
/// with the OpenLogos menu and lets every other click through.
final class MenuCatcherView: NSView {
    var key = ""
    var provide: () -> NSMenu? = { nil }
    /// Tests stand in for the current event's type.
    static var eventTypeOverride: NSEvent.EventType?

    override func hitTest(_ point: NSPoint) -> NSView? {
        let type = Self.eventTypeOverride ?? NSApp.currentEvent?.type
        let control = NSApp.currentEvent?.modifierFlags.contains(.control) ?? false
        guard type == .rightMouseDown || type == .rightMouseUp || (type == .leftMouseDown && control) else { return nil }
        return super.hitTest(point)
    }

    override func menu(for event: NSEvent) -> NSMenu? {
        // ⌥-right-click: the platform's menu, which SwiftUI chrome does not have.
        if event.modifierFlags.contains(.option) { return nil }
        return provide()
    }
}

struct MenuCatcher: NSViewRepresentable {
    let model: ShellModel
    let key: String
    let provide: () -> NSMenu?

    func makeNSView(context: Context) -> MenuCatcherView {
        let view = MenuCatcherView()
        updateNSView(view, context: context)
        return view
    }

    func updateNSView(_ view: MenuCatcherView, context: Context) {
        view.key = key
        view.provide = provide
        model.catchers.register(view)
    }
}
#endif

extension View {
    /// The OpenLogos menu for this piece of chrome; VoiceOver's show-menu action opens it too.
    func shellContextMenu(_ key: String, _ provide: @escaping () -> ShellContextTarget, model: ShellModel) -> some View {
        #if canImport(AppKit)
        overlay(MenuCatcher(model: model, key: key) { model.nsMenu(for: provide()) })
            .accessibilityAction(.showMenu) { model.catchers.popUp(key) }
        #else
        self
        #endif
    }
}
