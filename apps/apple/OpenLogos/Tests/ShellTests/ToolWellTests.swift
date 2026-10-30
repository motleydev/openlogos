// B076: the contextual tool well in the Mac shell. The well comes from the core for what has
// focus (translation, original, notes, reference) and whether text is selected; the stored
// tool survives a context that does not offer it; the note actions, the selection actions and
// Word study are the host's. The model runs over the real UniFFI ShellHandle; the rendered
// cases put the shell in a window.
import AppKit
import SwiftUI
import XCTest
@testable import OpenLogos

@MainActor
final class ToolWellTests: XCTestCase {
    private func commands(_ well: ShellToolWell, _ kind: String = "tools") -> [String] {
        well.groups.first { $0.kind == kind }?.items.map(\.commandId) ?? []
    }

    private func active(_ well: ShellToolWell) -> [String] {
        well.groups.flatMap(\.items).filter(\.active).map(\.commandId)
    }

    // MARK: Contexts

    func testTheWellFollowsTheFocusedTab() {
        let model = ShellModel()
        XCTAssertEqual(model.well.context, "translation", "column 1 is BSB")
        XCTAssertEqual(commands(model.well), ["tool.select", "tool.highlight", "tool.pen", "tool.highlighter_pen", "tool.eraser", "tool.collect"])
        XCTAssertEqual(model.well.groups.map(\.kind), ["tools", "ink"])
        XCTAssertEqual(commands(model.well, "ink"), ["tool.swatch", "tool.swap_swatches", "tool.width"])

        model.focusColumn(2)
        XCTAssertEqual(model.well.context, "original", "SBLGNT is registered as grc")
        XCTAssertEqual(commands(model.well), ["tool.select", "tool.word_study", "tool.highlight", "tool.pen", "tool.highlighter_pen", "tool.eraser", "tool.collect"])

        model.focusColumn(3)
        XCTAssertEqual(model.well.context, "notes")
        XCTAssertEqual(commands(model.well), ["tool.type", "note.bold", "note.italic", "note.heading", "note.list", "note.link_passage", "tool.collect"])
        XCTAssertEqual(model.well.groups.map(\.kind), ["tools"], "no ink on notes")
        XCTAssertEqual(active(model.well), ["tool.type"])

        model.focusColumn(1)
        model.run("tab.next")
        XCTAssertEqual(model.well.context, "original", "WLC is registered as hbo")

        model.run("view.notes_dock")
        model.setFocus(.notesDock)
        XCTAssertEqual(model.well.context, "notes", "the dock's editor has focus")
        model.run("view.notes_dock")
        XCTAssertEqual(model.focus, .tab, "closing the dock gives focus back to the tab")
        XCTAssertEqual(model.well.context, "original")
    }

    func testIconsMapToSymbolsAndTooltipsCarryKeys() throws {
        let model = ShellModel()
        model.focusColumn(3)
        for icon in ["select", "word-study", "highlight", "pen", "highlighter-pen", "eraser", "collect", "type", "bold", "italic", "heading", "list", "link-passage", "highlight-now", "note", "search", "copy-reference", "study"] {
            let symbol = try XCTUnwrap(ToolWell.symbols[icon], icon)
            XCTAssertNotNil(NSImage(systemSymbolName: symbol, accessibilityDescription: nil), "\(icon) → \(symbol) exists")
        }
        let bold = try XCTUnwrap(model.well.groups[0].items.first { $0.commandId == "note.bold" })
        XCTAssertEqual(ToolWell.tooltip(bold), "Bold (⌘B)")
        let heading = try XCTUnwrap(model.well.groups[0].items.first { $0.commandId == "note.heading" })
        XCTAssertEqual(ToolWell.tooltip(heading), "Heading")
        let pen = ShellToolWellItem(commandId: "tool.pen", tool: "pen", label: "Pen", key: "P", icon: "pen", active: false, value: nil, disabledText: "Pen works on Bible text")
        XCTAssertEqual(ToolWell.tooltip(pen), "Pen (P) — Pen works on Bible text")
    }

    // MARK: The effective tool

    func testAStoredToolSurvivesAContextThatDoesNotOfferIt() {
        let model = ShellModel()
        model.run("tool.pen")
        XCTAssertTrue(model.inkActive)
        model.focusColumn(3)
        XCTAssertEqual(model.tool, "pen", "stored")
        XCTAssertEqual(model.effectiveTool, "type", "notes default")
        XCTAssertEqual(active(model.well), ["tool.type"])
        XCTAssertFalse(model.inkActive, "no ink while Type is in effect")
        model.focusColumn(1)
        XCTAssertEqual(model.effectiveTool, "pen", "restored")
        XCTAssertEqual(active(model.well), ["tool.pen"])

        model.focusColumn(2)
        XCTAssertTrue(model.handle(chord: KeyChord(key: "W"), inTextField: false))
        XCTAssertEqual(model.tool, "word_study")
        XCTAssertEqual(model.dragMode, .word)
        model.focusColumn(1)
        XCTAssertEqual(model.effectiveTool, "select", "Word study is for Greek and Hebrew")
        XCTAssertEqual(model.dragMode, .select)
        model.focusColumn(2)
        XCTAssertEqual(model.effectiveTool, "word_study", "restored")
    }

    func testABareKeyOutsideItsContextSaysWhyInAToast() {
        let model = ShellModel()
        XCTAssertTrue(model.handle(chord: KeyChord(key: "W"), inTextField: false))
        XCTAssertEqual(model.tool, "select", "not stored")
        XCTAssertEqual(model.toast, "Word study works on Greek and Hebrew text")
        model.focusColumn(3)
        XCTAssertTrue(model.handle(chord: KeyChord(key: "P"), inTextField: false))
        XCTAssertEqual(model.toast, "Pen works on Bible text")
        XCTAssertTrue(model.handle(chord: KeyChord(key: "T"), inTextField: false))
        XCTAssertEqual(model.tool, "type")
        model.focusColumn(1)
        XCTAssertEqual(model.effectiveTool, "select")
        XCTAssertFalse(model.handle(chord: KeyChord(key: "B", command: true), inTextField: false), "⌘B passes on outside the note editor")
    }

    func testTheNewKeysComeFromTheRegistry() {
        let keymap = ShellKeymap(registry: shellRegistry(), platform: .mac)
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "W"))?.commandID, "tool.word_study")
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "T"))?.commandID, "tool.type")
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "B", command: true))?.commandID, "note.bold")
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "I", command: true))?.commandID, "note.italic")
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "E", command: true))?.commandID, "selection.search")
    }

    // MARK: Rendered

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

    private func select(_ phrase: String, in tabID: String, _ model: ShellModel) throws {
        let surface = try XCTUnwrap(model.surface(for: tabID))
        let layout = try XCTUnwrap(surface.layout)
        surface.select(display: layout.runMap.displayRange(fromCore: try XCTUnwrap(layout.coreRange(ofPhrase: phrase))))
    }

    func testASelectionAddsItsGroupBelowWithoutMovingTheTools() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        let top = ToolWell.top(position: 0.5, in: 800, height: ToolWell.referenceHeight, collapsed: false)
        XCTAssertEqual(ToolWell.top(position: 0.5, in: 800, height: ToolWell.referenceHeight + 6 * 34 + 17, collapsed: false), top,
                       "the selection group grows the well downward")
        XCTAssertEqual(ToolWell.top(position: 0.5, in: 800, height: ToolWell.referenceHeight - 3 * 34, collapsed: false), top,
                       "a context with fewer tools keeps the top")

        try select("loved the world", in: "t1", model)
        XCTAssertEqual(model.well.groups.map(\.kind), ["tools", "ink", "selection"])
        XCTAssertEqual(commands(model.well, "selection"), ["selection.highlight", "selection.collect", "selection.note", "selection.search", "selection.copy_reference", "selection.study"])
        model.surface(for: "t1")?.clearSelection()
        XCTAssertEqual(model.well.groups.map(\.kind), ["tools", "ink"])
    }

    func testCopyReferenceAndSearchThis() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        try select("loved the world", in: "t1", model)
        model.run("selection.copy_reference")
        XCTAssertEqual(model.lastCopied, "John 3:16")
        XCTAssertEqual(NSPasteboard.general.string(forType: .string), "John 3:16")
        XCTAssertEqual(model.toast, "Copied John 3:16")

        try select("loved the world", in: "t1", model)
        XCTAssertTrue(model.handle(chord: KeyChord(key: "E", command: true), inTextField: false))
        XCTAssertTrue(model.paletteOpen)
        XCTAssertEqual(model.paletteText, "loved the world")
        model.closePalette()
    }

    func testNoteOnItAppendsTheQuoteToTheOpenNote() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        try select("loved the world", in: "t1", model)
        model.run("selection.note")
        let text = model.notes.document(ShellModel.defaultNoteID).string
        XCTAssertTrue(text.hasSuffix("\n> “loved the world” — John 3:16"), text)
        XCTAssertFalse(model.activeWorkspace.notesDockOpen, "the notes tab is open, so no dock")

        model.run("workspace.next")
        spin(0.6)
        let surface = try XCTUnwrap(model.surface(for: model.activeColumn.shownTabId))
        let layout = try XCTUnwrap(surface.layout)
        XCTAssertEqual(layout.content.editionId, "wlc", "Hebrew's column 1 (its tab ids repeat Study's)")
        surface.select(display: layout.runMap.displayRange(fromCore: NSRange(location: 0, length: 6)))
        model.run("selection.note")
        XCTAssertTrue(model.activeWorkspace.notesDockOpen, "no note open: the dock opens")
        let after = model.notes.document(ShellModel.defaultNoteID).string
        XCTAssertTrue(after.hasSuffix("” — \(ShellReference.human("Gen.1.1"))"), after)
    }

    func testNotesTakeBoldItalicHeadingListAndPassageLinks() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        let editor = try XCTUnwrap(model.notes.editor(pane: "t4"))
        let storage = try XCTUnwrap(editor.textStorage)
        window.makeFirstResponder(editor)
        spin(0.1)
        XCTAssertEqual(model.well.context, "notes")
        XCTAssertTrue(model.notes.editing)

        editor.setSelectedRange(NSRange(location: 0, length: 7))
        XCTAssertTrue(model.handle(chord: KeyChord(key: "B", command: true), inTextField: true), "⌘B in the editor")
        let bold = try XCTUnwrap(storage.attribute(.font, at: 2, effectiveRange: nil) as? NSFont)
        XCTAssertTrue(bold.fontDescriptor.symbolicTraits.contains(.bold))
        model.run("note.italic")
        XCTAssertTrue((storage.attribute(.font, at: 2, effectiveRange: nil) as? NSFont)?.fontDescriptor.symbolicTraits.contains(.italic) ?? false)
        model.run("note.bold")
        XCTAssertFalse((storage.attribute(.font, at: 2, effectiveRange: nil) as? NSFont)?.fontDescriptor.symbolicTraits.contains(.bold) ?? true, "toggles off")

        model.run("note.heading")
        XCTAssertNotNil(storage.attribute(.openLogosNoteHeading, at: 0, effectiveRange: nil))
        XCTAssertGreaterThan((storage.attribute(.font, at: 0, effectiveRange: nil) as? NSFont)?.pointSize ?? 0, 19)
        model.run("note.list")
        XCTAssertTrue(storage.string.hasPrefix("• "))
        model.run("note.list")
        XCTAssertFalse(storage.string.hasPrefix("• "))

        editor.setSelectedRange(NSRange(location: storage.length, length: 0))
        model.run("note.link_passage", args: ShellCommandArgs(reference: "John.3.17"))
        XCTAssertTrue(storage.string.hasSuffix("John 3:17 "))
        let link = storage.attribute(.link, at: storage.length - 4, effectiveRange: nil)
        XCTAssertEqual(NoteStore.reference(try XCTUnwrap(link)), "John.3.17")
        XCTAssertTrue(model.notes.document(ShellModel.defaultNoteID).string.hasSuffix("John 3:17 "), "the note keeps the edit")

        // A click on the link opens the passage beside the note.
        XCTAssertTrue(editor.delegate?.textView?(editor, clickedOnLink: link as Any, at: storage.length - 4) ?? false)
        XCTAssertEqual(model.activeWorkspace.activeColumnNumber, 1)
        XCTAssertEqual(model.activeTab?.reference, "John.3.17")

        // Link passage from the well asks for the passage in the palette.
        model.focusColumn(3)
        model.runWellItem(try XCTUnwrap(model.well.groups[0].items.first { $0.commandId == "note.link_passage" }))
        XCTAssertTrue(model.paletteOpen)
        model.paletteText = "link passage Jn 3:16"
        model.paletteTextChanged()
        XCTAssertTrue(model.paletteRows.contains { $0.commandId == "note.link_passage" && $0.args.reference == "John.3.16" && $0.disabledReason == nil })
        model.closePalette()
    }
}
