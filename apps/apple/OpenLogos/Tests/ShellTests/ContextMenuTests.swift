// B077: the OpenLogos context menu in the Mac shell. The menu comes from the core for what was
// right-clicked; the host hit-tests the text (a word is selected first), notes and chrome,
// draws an NSMenu (header, separators, submenus, key equivalents, disabled rows with their
// words), applies the menu's focus before the chosen row and performs the host commands.
// The model runs over the real UniFFI ShellHandle; the shell is put in a window.
import AppKit
import SwiftUI
import XCTest
@testable import OpenLogos

@MainActor
final class ContextMenuTests: XCTestCase {
    private func host(_ model: ShellModel) -> NSWindow {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1440, height: 900), styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: ShellView(model: model))
        window.makeKeyAndOrderFront(nil)
        window.contentView?.layoutSubtreeIfNeeded()
        window.displayIfNeeded()
        return window
    }

    private func spin(_ seconds: TimeInterval = 0.4) {
        RunLoop.main.run(until: Date().addingTimeInterval(seconds))
    }

    /// The display range of a phrase in a tab's text.
    private func range(_ phrase: String, in tabID: String, _ model: ShellModel, occurrence: Int = 0) throws -> (ReaderSurface, NSRange) {
        let surface = try XCTUnwrap(model.surface(for: tabID))
        let layout = try XCTUnwrap(surface.layout)
        let string = layout.display.string as NSString
        var found = NSRange(location: NSNotFound, length: 0)
        var search = NSRange(location: 0, length: string.length)
        for _ in 0...occurrence {
            found = string.range(of: phrase, options: [], range: search)
            guard found.location != NSNotFound else { break }
            search = NSRange(location: NSMaxRange(found), length: string.length - NSMaxRange(found))
        }
        XCTAssertNotEqual(found.location, NSNotFound, phrase)
        return (surface, found)
    }

    /// A right-click at the middle of a phrase in a tab: the hit, as the text view sees it.
    private func rightClick(_ phrase: String, in tabID: String, _ model: ShellModel, occurrence: Int = 0) throws -> ReaderContextHit {
        let (surface, range) = try range(phrase, in: tabID, model, occurrence: occurrence)
        let box = try XCTUnwrap(surface.rect(for: range))
        return surface.contextHit(at: NSPoint(x: box.midX, y: box.midY))
    }

    private func tab(_ id: String, _ model: ShellModel) throws -> (ShellTab, ShellColumn) {
        let column = try XCTUnwrap(model.activeWorkspace.columns.first { $0.tabs.contains { $0.id == id } })
        return (try XCTUnwrap(column.tabs.first { $0.id == id }), column)
    }

    private func menu(_ hit: ReaderContextHit, in tabID: String, _ model: ShellModel) throws -> ShellNSMenu {
        let (tab, column) = try tab(tabID, model)
        return try XCTUnwrap(model.nsMenu(for: hit, tab: tab, column: column) as? ShellNSMenu)
    }

    /// Every row of a menu and its submenus, by title.
    private func item(_ title: String, in menu: NSMenu) throws -> NSMenuItem {
        try XCTUnwrap(menu.items.lazy.flatMap { [$0] + ($0.submenu?.items ?? []) }.first { $0.title == title }, title)
    }

    private func choose(_ title: String, in menu: NSMenu) throws {
        let row = try item(title, in: menu)
        XCTAssertTrue(row.isEnabled, "\(title) is enabled")
        let dispatcher = try XCTUnwrap(row.target as? MenuDispatcher, "\(title) has an action")
        dispatcher.choose(row)
    }

    // MARK: Targets and drawing

    func testAWordMenuSelectsTheWordAndIsDrawnFromTheCore() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()

        // Greek in column 2 (not focused): Word study enabled, focus first.
        let hit = try rightClick("ἠγάπησεν", in: "t3", model)
        guard case .word(let selection) = hit else { return XCTFail("a word: \(hit)") }
        XCTAssertEqual(selection.text, "ἠγάπησεν")
        XCTAssertEqual(model.surface(for: "t3")?.selectedRange.length, ("ἠγάπησεν" as NSString).length, "the word is selected, as macOS does")
        XCTAssertEqual(model.selection?.paneID, "t3")
        let (tab, column) = try tab("t3", model)
        guard case .word(let text, let reference, let segment, let start, let end, let resource, let tabID) = model.target(for: hit, tab: tab, column: column) else {
            return XCTFail("a word target")
        }
        XCTAssertEqual([text, reference, segment, resource, tabID], ["ἠγάπησεν", "John.3.16", "John.3.16", "sblgnt", "t3"])
        XCTAssertEqual(start.map { Int($0) }, (selection.anchors.first?.range.start).map { Int($0) })
        XCTAssertNotNil(end)

        let greek = try menu(hit, in: "t3", model)
        let core = try XCTUnwrap(greek.shellMenu)
        XCTAssertEqual(core.title, "ἠγάπησεν")
        XCTAssertEqual(core.focus?.commandId, "tab.focus", "column 2 is not focused")
        XCTAssertEqual(greek.items.first?.isSectionHeader, true, "the word heads the menu")
        XCTAssertEqual(greek.items.filter(\.isSeparatorItem).count, core.sections.count - 1, "sections between separators")
        XCTAssertEqual(greek.items.filter { !$0.isSeparatorItem && !$0.isSectionHeader }.map(\.title),
                       core.sections.flatMap(\.items).map(\.label), "rows in core order with core labels")
        XCTAssertTrue(try item("Word study", in: greek).isEnabled)
        XCTAssertEqual(try item("Compare in BSB", in: greek).identifier?.rawValue, "tab.open_edition", "one other edition: a plain row")
        let lemma = try item("Search lemma", in: greek)
        XCTAssertFalse(lemma.isEnabled)
        XCTAssertEqual(lemma.subtitle, "Needs the lexicon import")
        XCTAssertEqual(lemma.toolTip, "Needs the lexicon import")
        let collect = try item("Collect", in: greek)
        XCTAssertEqual(collect.keyEquivalent, "c")
        XCTAssertEqual(collect.keyEquivalentModifierMask, [.command, .shift])
        let highlight = try item("Highlight", in: greek)
        XCTAssertEqual(highlight.submenu?.items.map(\.title), ["Yellow", "Green", "Blue", "Pink"])
        XCTAssertEqual(highlight.submenu?.items.map(\.state), [.on, .off, .off, .off], "the current swatch is checked")
        XCTAssertNotEqual(highlight.action, #selector(MenuDispatcher.choose(_:)), "a Mac submenu just opens")
        for row in greek.items.lazy.flatMap({ [$0] + ($0.submenu?.items ?? []) }) where !row.isSeparatorItem && !row.isSectionHeader {
            XCTAssertNotNil(row.image, "\(row.title) has an icon")
        }

        // A translation: Word study is there, disabled with words.
        let bsb = try self.menu(try rightClick("loved", in: "t1", model), in: "t1", model)
        XCTAssertNil(bsb.shellMenu?.focus, "column 1 has focus")
        let study = try item("Word study", in: bsb)
        XCTAssertFalse(study.isEnabled)
        XCTAssertEqual(study.subtitle, "Word study works on Greek and Hebrew text")
        XCTAssertNotNil(try? item("Compare in SBLGNT", in: bsb), "SBLGNT, not the note or the lexicon")
    }

    func testIconsMapToSymbols() {
        for (icon, symbol) in ContextMenuBuilder.symbols {
            XCTAssertNotNil(NSImage(systemSymbolName: symbol, accessibilityDescription: nil), "\(icon) → \(symbol)")
        }
        XCTAssertEqual(ContextMenuBuilder.symbol("search"), "magnifyingglass", "the well's icons stay the well's")
    }

    func testInsideASelectionAMarginAndThePlatformMenu() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        let (surface, range) = try range("loved the world", in: "t1", model)
        surface.select(display: range)
        let box = try XCTUnwrap(surface.rect(for: range))
        guard case .selection(let selection) = surface.contextHit(at: NSPoint(x: box.midX, y: box.midY)) else { return XCTFail("the selection") }
        XCTAssertEqual(selection.text, "loved the world")
        let menu = try self.menu(.selection(selection), in: "t1", model)
        XCTAssertEqual(menu.shellMenu?.targetKind, "selection")
        XCTAssertEqual(try item("Copy", in: menu).keyEquivalent, "c")
        XCTAssertNotNil(try? item("Study", in: menu))

        // Far below the text: the column's background.
        let text = try XCTUnwrap(surface.textView)
        XCTAssertEqual(surface.contextHit(at: NSPoint(x: 30, y: text.bounds.maxY - 4)), .background)
        let background = try XCTUnwrap(model.nsMenu(for: model.target(for: .background, tab: try tab("t1", model).0, column: try tab("t1", model).1)) as? ShellNSMenu)
        XCTAssertEqual(background.shellMenu?.targetKind, "background")
        XCTAssertEqual(try item("Theme", in: background).submenu?.items.map(\.title), ["System", "Light", "Dark", "Sepia"])

        // ⌥-right-click: the platform's menu, none of ours.
        let point = text.convert(NSPoint(x: box.midX, y: box.midY), to: nil)
        let option = try XCTUnwrap(NSEvent.mouseEvent(with: .rightMouseDown, location: point, modifierFlags: .option, timestamp: 0,
                                                       windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1))
        let platform = try XCTUnwrap(text.menu(for: option))
        XCTAssertFalse(platform is ShellNSMenu)
        XCTAssertFalse(platform.items.contains { $0.identifier?.rawValue == "search.text" })
        let plain = try XCTUnwrap(NSEvent.mouseEvent(with: .rightMouseDown, location: point, modifierFlags: [], timestamp: 0,
                                                      windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1))
        XCTAssertTrue(text.menu(for: plain) is ShellNSMenu, "a plain right-click: ours")
    }

    func testChromeAnswersRightClicksAndLetsClicksThrough() throws {
        let model = ShellModel()
        let window = host(model)
        defer {
            MenuCatcherView.eventTypeOverride = nil
            window.close()
        }
        spin()
        let content = try XCTUnwrap(window.contentView)
        for key in ["tab-t1", "tab-t3", "column-2", "strip-1", "background-1"] {
            let catcher = try XCTUnwrap(model.catchers.view(key), key)
            let box = catcher.convert(catcher.bounds, to: nil)
            // A spot inside the catcher that no other piece of chrome covers.
            let point = key.hasPrefix("background") ? NSPoint(x: box.minX + 20, y: box.minY + 20)
                : key.hasPrefix("strip") ? NSPoint(x: box.maxX - 40, y: box.midY) : NSPoint(x: box.minX + 8, y: box.midY)
            MenuCatcherView.eventTypeOverride = .rightMouseDown
            // hitTest takes the superview's coordinates: the window's, for the content view.
            let hit = content.hitTest(point)
            XCTAssertTrue(hit === catcher, "a right-click at \(key) reaches its catcher, not \(String(describing: hit))")
            MenuCatcherView.eventTypeOverride = .leftMouseDown
            XCTAssertFalse(content.hitTest(point) is MenuCatcherView, "a click at \(key) goes through")
        }
        MenuCatcherView.eventTypeOverride = nil
        let tabMenu = try XCTUnwrap(model.catchers.view("tab-t2")?.provide() as? ShellNSMenu)
        XCTAssertEqual(tabMenu.shellMenu?.targetKind, "tab")
        XCTAssertEqual(tabMenu.shellMenu?.focus?.commandId, "tab.focus", "t2 is hidden")
        XCTAssertEqual(try item("Close tab", in: tabMenu).keyEquivalentModifierMask, [.option])
        let move = try item("Move to", in: tabMenu)
        let own = try XCTUnwrap(move.submenu?.items.first)
        XCTAssertFalse(own.isEnabled)
        XCTAssertEqual(own.subtitle, "The tab is already in column 1")
        XCTAssertEqual(own.keyEquivalent, "1")
        XCTAssertEqual(own.keyEquivalentModifierMask, [.option, .shift])
        let columnMenu = try XCTUnwrap(model.catchers.view("column-2")?.provide() as? ShellNSMenu)
        XCTAssertEqual(columnMenu.shellMenu?.title, "Column 2")
        let option = try XCTUnwrap(NSEvent.mouseEvent(with: .rightMouseDown, location: .zero, modifierFlags: .option, timestamp: 0,
                                                       windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1))
        XCTAssertNil(model.catchers.view("tab-t1")?.menu(for: option), "⌥-right-click: no OpenLogos menu on chrome")
    }

    // MARK: Dispatch

    func testFocusIsAppliedOnChooseNotOnOpen() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        let menu = try self.menu(try rightClick("ἠγάπησεν", in: "t3", model), in: "t3", model)
        XCTAssertEqual(model.activeWorkspace.activeColumnNumber, 1, "opening the menu changes nothing")
        try choose("Copy word", in: menu)
        XCTAssertEqual(model.activeWorkspace.activeColumnNumber, 2, "the click's focus change, then the row")
        XCTAssertEqual(NSPasteboard.general.string(forType: .string), "ἠγάπησεν")
        XCTAssertEqual(model.toast, "Copied “ἠγάπησεν”")
    }

    func testCompareOpensTheOtherEditionInTheNextColumnAndUndoes() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        model.run("nav.navigate", args: ShellCommandArgs(column: 2, reference: "John.3.17"))
        let menu = try self.menu(try rightClick("loved", in: "t1", model), in: "t1", model)
        try choose("Compare in SBLGNT", in: menu)
        let column2 = model.activeWorkspace.columns[1]
        let shown = try XCTUnwrap(column2.tabs.first { $0.id == column2.shownTabId })
        XCTAssertEqual([shown.contentId, shown.reference], ["sblgnt", "John.3.16"], "column 2 shows SBLGNT at the word's verse")
        XCTAssertEqual(model.activeWorkspace.activeColumnNumber, 1, "focus stays on the source")
        model.run("edit.undo")
        XCTAssertEqual(model.activeWorkspace.columns[1].tabs.first?.reference, "John.3.17", "undone")

        // From the Greek in column 2: BSB opens beside it, in column 3.
        model.run("nav.navigate", args: ShellCommandArgs(column: 2, reference: "John.3.16"))
        spin(0.3)
        let greek = try self.menu(try rightClick("ἠγάπησεν", in: "t3", model), in: "t3", model)
        try choose("Compare in BSB", in: greek)
        let column3 = model.activeWorkspace.columns[2]
        XCTAssertEqual(column3.tabs.first { $0.id == column3.shownTabId }?.contentId, "bsb")
    }

    func testSearchOpensThePaletteWithItsScope() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        let menu = try self.menu(try rightClick("loved", in: "t1", model), in: "t1", model)
        try choose("Search “loved” in this edition", in: menu)
        XCTAssertTrue(model.paletteOpen)
        XCTAssertEqual(model.paletteText, "#loved")
        XCTAssertEqual(model.palette.scope?.label, "This edition · BSB")
        let text = model.paletteResult?.groups.first { $0.section == .text }?.items ?? []
        XCTAssertFalse(text.isEmpty)
        XCTAssertTrue(text.allSatisfy { $0.title.hasPrefix("BSB ·") }, text.map(\.title).joined(separator: ", "))
        model.clearSearchScope()
        XCTAssertNil(model.palette.scope, "× searches every edition again")
        model.closePalette()

        model.surface(for: "t1")?.clearSelection()
        let again = try self.menu(try rightClick("loved", in: "t1", model), in: "t1", model)
        try choose("Search “loved” in all editions", in: again)
        XCTAssertEqual(model.palette.scope?.label, "All editions")
        model.closePalette()
        XCTAssertNil(model.palette.scope, "closing the palette drops the scope")
    }

    func testFindInThisChapterPaintsMatchesAndSteps() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        let menu = try self.menu(try rightClick("world", in: "t1", model), in: "t1", model)
        try choose("Find in this chapter", in: menu)
        let find = try XCTUnwrap(model.find)
        XCTAssertEqual(find.tabID, "t1")
        XCTAssertEqual(find.ranges.count, 4, "world: once in 3:16, three times in 3:17")
        XCTAssertEqual(model.toast, "4 matches in John 3")
        XCTAssertFalse(model.paletteOpen, "Find stays in the column")
        XCTAssertEqual(model.readerMarks("t1").filter { $0.id.hasPrefix("find-") }.count, 4)

        func key(_ code: UInt16, _ characters: String, _ flags: NSEvent.ModifierFlags) -> NSEvent {
            NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: window.windowNumber, context: nil,
                             characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code)!
        }
        XCTAssertTrue(model.handleMenuKey(key(5, "g", .command)))
        XCTAssertEqual(model.find?.index, 1)
        XCTAssertEqual(model.toast, "2 of 4 in John 3")
        XCTAssertTrue(model.handleMenuKey(key(5, "G", [.command, .shift])))
        XCTAssertTrue(model.handleMenuKey(key(5, "G", [.command, .shift])))
        XCTAssertEqual(model.find?.index, 3, "⇧⌘G wraps backwards")
        XCTAssertTrue(model.handleMenuKey(key(53, "\u{1b}", [])))
        XCTAssertNil(model.find, "Esc clears")
        XCTAssertFalse(model.handleMenuKey(key(5, "g", .command)), "⌘G passes on without a find")
        XCTAssertTrue(model.readerMarks("t1").isEmpty)
    }

    func testCopiesPutPassagesOnThePasteboard() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        var menu = try self.menu(try rightClick("loved", in: "t1", model), in: "t1", model)
        try choose("Copy reference", in: menu)
        XCTAssertEqual(NSPasteboard.general.string(forType: .string), "John 3:16")
        XCTAssertEqual(model.toast, "Copied John 3:16")
        model.surface(for: "t1")?.clearSelection()
        menu = try self.menu(try rightClick("loved", in: "t1", model), in: "t1", model)
        try choose("Copy verse with reference", in: menu)
        let verse = try XCTUnwrap(NSPasteboard.general.string(forType: .string))
        XCTAssertTrue(verse.hasPrefix("For God so loved the world"), verse)
        XCTAssertTrue(verse.hasSuffix(" — John 3:16 (BSB)"), verse)
        XCTAssertEqual(model.toast, "Copied John 3:16 with its text")

        // A selection over two verses: its text and the range.
        let (surface, start) = try range("eternal life", in: "t1", model)
        let (_, end) = try range("send", in: "t1", model)
        surface.select(display: NSRange(location: start.location, length: NSMaxRange(end) - start.location))
        let box = try XCTUnwrap(surface.rect(for: start))
        let selection = try self.menu(surface.contextHit(at: NSPoint(x: box.midX, y: box.midY)), in: "t1", model)
        try choose("Copy with reference", in: selection)
        let copied = try XCTUnwrap(NSPasteboard.general.string(forType: .string))
        XCTAssertTrue(copied.hasPrefix("eternal life."), copied)
        XCTAssertTrue(copied.hasSuffix("send — John 3:16–17 (BSB)"), copied)
        try choose("Copy", in: try self.menu(surface.contextHit(at: NSPoint(x: box.midX, y: box.midY)), in: "t1", model))
        XCTAssertTrue(NSPasteboard.general.string(forType: .string)?.hasPrefix("eternal life.") ?? false, "host.copy copies the selection")
    }

    func testHighlightInASwatch() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        let menu = try self.menu(try rightClick("loved", in: "t1", model), in: "t1", model)
        try choose("Green", in: menu)
        let mark = try XCTUnwrap(model.marks["t1"]?.first)
        XCTAssertEqual(mark.color, ShellTokens.swatch(hex: "#4CC38A").platformColor)
        XCTAssertNil(model.selection, "the selection collapses into the highlight")
    }

    func testPastePassageOpensTheClipboardsPassage() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(" Jn 3:17\n", forType: .string)
        let menu = try XCTUnwrap(model.nsMenu(for: .background(column: 1)))
        try choose("Paste passage", in: menu)
        XCTAssertEqual(model.activeTab?.reference, "John.3.17")
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString("nothing here", forType: .string)
        try choose("Paste passage", in: try XCTUnwrap(model.nsMenu(for: .background(column: 1))))
        XCTAssertEqual(model.toast, "No passage on the clipboard")
    }

    func testTabMenuClosesOtherTabsAndUndoes() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        let menu = try XCTUnwrap(model.catchers.view("tab-t2")?.provide())
        try choose("Close other tabs", in: menu)
        XCTAssertEqual(model.activeWorkspace.columns[0].tabs.map(\.id), ["t2"], "t2 was focused first, then the others closed")
        model.run("edit.undo")
        XCTAssertEqual(Set(model.activeWorkspace.columns[0].tabs.map(\.id)), ["t1", "t2"])
    }

    func testNoteMenusCutCopyPasteAndRemoveALink() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        let editor = try XCTUnwrap(model.notes.editor(pane: "t4"))
        let storage = try XCTUnwrap(editor.textStorage)
        window.makeFirstResponder(editor)
        editor.setSelectedRange(NSRange(location: storage.length, length: 0))
        model.run("note.link_passage", args: ShellCommandArgs(reference: "Rom.8.28"))
        let label = ShellReference.humanRange("Rom.8.28")
        let link = (storage.string as NSString).range(of: label)
        XCTAssertNotEqual(link.location, NSNotFound, storage.string)

        guard case .link(let reference, let range) = editor.hit(at: link.location + 2) else { return XCTFail("a link") }
        XCTAssertEqual(reference, "Rom.8.28")
        let menu = try XCTUnwrap(model.nsMenu(for: .link(reference, range), editor: editor) as? ShellNSMenu)
        XCTAssertEqual(menu.shellMenu?.targetKind, "note_link")
        XCTAssertEqual(menu.shellMenu?.title, "Rom 8:28")
        try choose("Copy reference", in: menu)
        XCTAssertEqual(NSPasteboard.general.string(forType: .string), label)
        try choose("Remove link", in: try XCTUnwrap(model.nsMenu(for: .link(reference, range), editor: editor)))
        XCTAssertNil(storage.attribute(.link, at: link.location + 2, effectiveRange: nil), "the words stay, the link goes")
        XCTAssertEqual(model.toast, "Removed link to \(label)")
        model.run("edit.undo")

        // Note text: Cut and Copy need a selection; Bold acts on the editor.
        editor.setSelectedRange(NSRange(location: 0, length: 7))
        let word = (storage.string as NSString).substring(with: NSRange(location: 0, length: 7))
        let text = try XCTUnwrap(model.nsMenu(for: editor.hit(at: 3), editor: editor) as? ShellNSMenu)
        XCTAssertEqual(text.shellMenu?.targetKind, "note_text")
        XCTAssertEqual(try item("Cut", in: text).keyEquivalent, "x")
        try choose("Copy", in: text)
        XCTAssertEqual(NSPasteboard.general.string(forType: .string), word)
        try choose("Bold", in: try XCTUnwrap(model.nsMenu(for: editor.hit(at: 3), editor: editor)))
        XCTAssertTrue((storage.attribute(.font, at: 2, effectiveRange: nil) as? NSFont)?.fontDescriptor.symbolicTraits.contains(.bold) ?? false)
        try choose("Cut", in: try XCTUnwrap(model.nsMenu(for: editor.hit(at: 3), editor: editor)))
        XCTAssertFalse(storage.string.hasPrefix(word), "cut")
        editor.setSelectedRange(NSRange(location: 0, length: 0))
        try choose("Paste", in: try XCTUnwrap(model.nsMenu(for: editor.hit(at: 0), editor: editor)))
        XCTAssertTrue(storage.string.hasPrefix(word), "pasted back")
    }

    // MARK: Keyboard

    /// Opens a menu from the keyboard and returns its core menu, closing it as it opens.
    private func keyboardMenu(_ model: ShellModel) -> ShellContextMenu? {
        var opened: ShellContextMenu?
        model.lastMenu = nil
        let timer = Timer(timeInterval: 0.05, repeats: true) { _ in
            MainActor.assumeIsolated {
                guard let menu = model.lastMenu as? ShellNSMenu else { return }
                opened = menu.shellMenu
                menu.cancelTracking()
            }
        }
        RunLoop.main.add(timer, forMode: .common)
        defer { timer.invalidate() }
        let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: 0, context: nil,
                                     characters: "", charactersIgnoringModifiers: "", isARepeat: false, keyCode: ShellModel.menuKeyCode)!
        XCTAssertTrue(model.handleMenuKey(event))
        return opened
    }

    func testTheMenuKeyOpensTheMenuForWhatHasFocus() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        let (surface, word) = try range("loved", in: "t1", model)
        let text = try XCTUnwrap(surface.textView)
        window.makeFirstResponder(text)
        text.setSelectedRange(NSRange(location: word.location + 1, length: 0))
        XCTAssertEqual(keyboardMenu(model)?.title, "loved", "the caret's word")
        XCTAssertEqual(surface.selectedRange, word, "and it is selected")
        XCTAssertEqual(keyboardMenu(model)?.targetKind, "selection", "now the selection")

        window.makeFirstResponder(nil)
        XCTAssertEqual(keyboardMenu(model)?.targetKind, "tab", "no text has focus: the focused tab")

        let editor = try XCTUnwrap(model.notes.editor(pane: "t4"))
        window.makeFirstResponder(editor)
        editor.setSelectedRange(NSRange(location: 0, length: 0))
        XCTAssertEqual(keyboardMenu(model)?.targetKind, "note_text")
    }
}
