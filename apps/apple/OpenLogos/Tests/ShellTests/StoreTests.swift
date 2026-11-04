// B074: the user store in the Mac shell (specs/24), hosted in the app over the real UniFFI
// handles, every file in a temporary directory (never the real Application Support store):
// a quit and relaunch brings back the workspaces, columns, tabs, scroll anchors, tool, notes,
// highlights and ink; a damaged file shows the notice, keeps a backup and starts fresh; a
// note survives the trip through the stored block form exactly; a tab drag and a column
// resize undo; the palette reaches every workspace's tabs.
import AppKit
import SwiftUI
import XCTest
@testable import OpenLogos

@MainActor
final class StoreTests: XCTestCase {
    private var directory: URL!

    override func setUp() async throws {
        directory = FileManager.default.temporaryDirectory.appendingPathComponent("openlogos-store-\(UUID().uuidString)", isDirectory: true)
    }

    override func tearDown() async throws {
        try? FileManager.default.removeItem(at: directory)
    }

    private var file: UserStoreFile { UserStoreFile(directory: directory) }

    private func host(_ model: ShellModel, size: CGSize = CGSize(width: 1440, height: 900)) -> NSWindow {
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled, .resizable], backing: .buffered, defer: false)
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

    private func shownTab(_ model: ShellModel, column: Int) -> ShellTab? {
        let c = model.activeWorkspace.columns[column - 1]
        return c.tabs.first { $0.id == c.shownTabId }
    }

    // MARK: Never the real store

    func testTestsAndScriptedRunsNeverUseTheRealStore() {
        XCTAssertNil(ShellPersistence.launchFile(), "XCTest gets no store")
        let defaults = UserDefaults(suiteName: "openlogos-store-test-\(UUID().uuidString)")!
        XCTAssertNil(ShellPersistence.launchFile(defaults: defaults, environment: ["XCTestConfigurationFilePath": "x"]))
        defaults.set("/tmp/x.png", forKey: "OPENLOGOS_SNAPSHOT")
        XCTAssertNil(ShellPersistence.launchFile(defaults: defaults, environment: [:]), "a snapshot keeps no store")
        defaults.set(directory.path, forKey: "OPENLOGOS_STORE")
        XCTAssertEqual(ShellPersistence.launchFile(defaults: defaults, environment: [:])?.directory.path, directory.path, "unless one is named")
        XCTAssertTrue(UserStoreFile.applicationSupport.path.hasSuffix("Application Support/OpenLogos"))
    }

    // MARK: Quit and relaunch

    func testAQuitAndRelaunchBringsEverythingBack() throws {
        let model = ShellModel(store: file)
        var window: NSWindow? = host(model)
        spin()
        XCTAssertNil(model.restored?.report, "no file yet: the seed session")
        // A third workspace, then back to the first.
        model.run("workspace.new", args: ShellCommandArgs(text: "Romans"))
        model.run("nav.navigate", args: ShellCommandArgs(column: 1, reference: "Rom.8.1"))
        model.run("workspace.switch", args: ShellCommandArgs(workspaceId: "w1"))
        spin()
        // Column 2 on its own at Acts 2, scrolled by the reader.
        model.focusColumn(2)
        model.run("tab.pin")
        model.run("nav.navigate", args: ShellCommandArgs(column: 2, reference: "Acts.2.1"))
        spin(0.5)
        let acts = try XCTUnwrap(model.stream(for: "t3"))
        for _ in 0..<9 { acts.scrollAsReader(by: 40); spin(0.02) }
        spin(0.4)
        let actsTop = try XCTUnwrap(acts.topSegment)
        XCTAssertTrue(actsTop.hasPrefix("Acts.2."), actsTop)
        // A highlight on John 3:16 and ink on John 3:17 in column 1; a note; the Highlight tool.
        model.focusColumn(1)
        let stream = try XCTUnwrap(model.stream(for: "t1"))
        let layout = try XCTUnwrap(stream.surface.layout)
        model.run("tool.highlight")
        spin(0.1)
        stream.surface.select(display: layout.runMap.displayRange(fromCore: try XCTUnwrap(layout.coreRange(ofPhrase: "loved the world"))))
        stream.surface.dragEnded()
        XCTAssertEqual(model.highlights("t1").count, 1)
        StoreLaunch.drawInk(model, stream: stream, on: "John.3.17")
        let sheet = model.inkSheet(for: "t1")
        let stroke = try XCTUnwrap(sheet.liveStrokes.first)
        XCTAssertTrue(stroke.id.hasPrefix("k"), "the core's id: \(stroke.id)")
        let drawnY = try XCTUnwrap(stroke.points.first?.y) + Double(try XCTUnwrap(sheet.displacement(of: stroke.id)))
        ShellLaunch.noteDemo(model)
        model.focusColumn(1)
        model.run("tool.highlight")
        spin(0.2)
        XCTAssertTrue(try XCTUnwrap(model.persistence).isDirty)
        // Quit.
        model.persistence?.saveNow()
        XCTAssertFalse(try XCTUnwrap(model.persistence).isDirty)
        let saved = try XCTUnwrap(file.read())
        window?.close()
        window = nil

        // Relaunch.
        let back = ShellModel(store: file)
        let again = host(back)
        defer { again.close() }
        spin(0.8)
        let report = try XCTUnwrap(back.restored?.report)
        XCTAssertTrue(report.issues.isEmpty)
        XCTAssertNil(back.lastNotice, "nothing to say")
        XCTAssertEqual(back.session.workspaces.map(\.title), ["Study", "Romans", "Hebrew"])
        XCTAssertEqual(back.session.activeWorkspaceId, "w1")
        XCTAssertEqual(back.tool, "highlight", "the tool")
        XCTAssertEqual(back.activeWorkspace.columns.count, 3)
        XCTAssertEqual(shownTab(back, column: 2)?.scrollSegmentId, actsTop, "the column's scroll anchor")
        let restoredActs = try XCTUnwrap(back.stream(for: "t3"))
        XCTAssertEqual(restoredActs.topSegment, actsTop, "Acts 2 on the reading line")
        // The highlight is painted on its words.
        let restored = try XCTUnwrap(back.stream(for: "t1"))
        let storage = try XCTUnwrap(restored.surface.storage)
        let backLayout = try XCTUnwrap(restored.surface.layout)
        let phrase = backLayout.runMap.displayRange(fromCore: try XCTUnwrap(backLayout.coreRange(ofPhrase: "loved the world")))
        var effective = NSRange()
        XCTAssertNotNil(storage.attribute(.backgroundColor, at: phrase.location + 1, longestEffectiveRange: &effective, in: NSRange(location: 0, length: storage.length)))
        XCTAssertEqual((storage.string as NSString).substring(with: effective), "loved the world")
        // The ink is on its verse, where it was drawn.
        let backSheet = back.inkSheet(for: "t1")
        let backStroke = try XCTUnwrap(backSheet.liveStrokes.first { $0.id == stroke.id })
        XCTAssertEqual(backSheet.anchors[backStroke.id]?.segment, "John.3.17")
        let backY = try XCTUnwrap(backStroke.points.first?.y) + Double(try XCTUnwrap(backSheet.displacement(of: backStroke.id)))
        XCTAssertEqual(backY, drawnY, accuracy: 1, "drawn where it was")
        XCTAssertTrue(back.hasInk("t1"))
        // The note with its heading, bold words and passage link.
        let note = back.notes.document(ShellModel.defaultNoteID)
        XCTAssertTrue(note.string.hasPrefix("Love that gives\n"))
        XCTAssertNotNil(note.attribute(.openLogosNoteHeading, at: 0, effectiveRange: nil))
        let bold = (note.string as NSString).range(of: "so loved")
        XCTAssertTrue(((note.attribute(.font, at: bold.location, effectiveRange: nil) as? NSFont)?.fontDescriptor.symbolicTraits.contains(.bold)) ?? false)
        let link = (note.string as NSString).range(of: "John 3:16")
        XCTAssertEqual(note.attribute(.link, at: link.location, effectiveRange: nil).flatMap(NoteStore.reference), "John.3.16")
        // Nothing to undo after a restore, and saving again writes the same bytes.
        XCTAssertNil(back.session.undoTitle)
        XCTAssertEqual(back.handle.exportStore(), String(decoding: saved, as: UTF8.self))
    }

    func testSavesWaitForTheDebounceAndCountQuietChanges() throws {
        let model = ShellModel(store: file)
        let window = host(model)
        defer { window.close() }
        spin()
        let persistence = try XCTUnwrap(model.persistence)
        spin(1.3)
        let saves = persistence.saves
        try XCTUnwrap(model.stream(for: "t1")).scrollAsReader(by: 200)
        spin(0.3)
        XCTAssertTrue(persistence.isDirty, "a scroll anchor is a change to save")
        XCTAssertEqual(persistence.saves, saves, "not before the debounce")
        spin(1.3)
        XCTAssertFalse(persistence.isDirty)
        XCTAssertEqual(persistence.saves, saves + 1)
        let json = String(decoding: try XCTUnwrap(file.read()), as: UTF8.self)
        XCTAssertTrue(json.hasPrefix("{\"format\":\"openlogos-user-store\""))
        let anchor = try XCTUnwrap(model.activeWorkspace.columns[0].tabs[0].scrollSegmentId)
        XCTAssertTrue(json.contains("\"scroll_anchor\":{\"segment_id\":\"\(anchor)\""), "the file has the new anchor \(anchor)")
        XCTAssertFalse(FileManager.default.fileExists(atPath: directory.appendingPathComponent("user-store.json.tmp").path), "the temporary file is swapped in")
    }

    // MARK: Damage

    func testADamagedFileShowsTheNoticeKeepsABackupAndTheAppWorks() throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let damaged = Data("{\"format\":\"openlogos-user-store\",\"schema_version\":\"1.0\",\"session\":{".utf8)
        try damaged.write(to: file.url)
        let model = ShellModel(store: file)
        let window = host(model)
        defer { window.close() }
        spin()
        XCTAssertEqual(model.restored?.refused, "not_json")
        XCTAssertEqual(model.lastNotice, "Your saved workspace could not be read; a copy was kept.")
        XCTAssertEqual(model.toast, model.lastNotice, "shown once")
        let backups = file.backups()
        XCTAssertEqual(backups.count, 1)
        XCTAssertEqual(try Data(contentsOf: backups[0]), damaged, "the bytes as they were")
        XCTAssertTrue(backups[0].lastPathComponent.hasPrefix("user-store.backup-"))
        XCTAssertEqual(model.session.workspaces.map(\.title), ["Study", "Hebrew"], "the seed session")
        // The app works, and the next save replaces the damaged file.
        XCTAssertTrue(model.run("column.new"))
        model.persistence?.saveNow()
        XCTAssertNoThrow(try ShellHandle(sessionJson: ShellSeed.json).importStore(json: String(decoding: try XCTUnwrap(file.read()), as: UTF8.self)))
        XCTAssertEqual(file.backups().count, 1, "a save never overwrites a backup")
    }

    func testANewerVersionAndSetAsideParts() throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        // A store from a newer major.
        let seed = ShellModel()
        var newer = seed.handle.exportStore().replacingOccurrences(of: "\"schema_version\":\"1.0\",\"fixture_only\"", with: "\"schema_version\":\"2.0\",\"fixture_only\"")
        try Data(newer.utf8).write(to: file.url)
        let model = ShellModel(store: file)
        XCTAssertEqual(model.restored?.refused, "unsupported_version")
        XCTAssertEqual(model.lastNotice, "Your saved workspace was written by a newer version of OpenLogos; a copy was kept.")
        // A readable store with a bad highlight and a dangling active workspace.
        newer = seed.handle.exportStore()
            .replacingOccurrences(of: "\"active_workspace_id\":\"w1\"", with: "\"active_workspace_id\":\"nowhere\"")
            .replacingOccurrences(of: "\"highlights\":[]", with: "\"highlights\":[{\"id\":\"h1\",\"resource_id\":\"bsb\",\"content_revision\":\"0\",\"spans\":[],\"color\":\"#F5D90A\"},{\"id\":\"h2\",\"resource_id\":\"bsb\",\"content_revision\":\"0\",\"spans\":[{\"segment_id\":\"John.3.16\",\"start\":0,\"end\":3}],\"color\":\"yellow\"}]")
        try Data(newer.utf8).write(to: file.url)
        let partial = ShellModel(store: file)
        XCTAssertEqual(partial.restored?.report?.sessionRestored, false)
        XCTAssertEqual(partial.lastNotice, "Your workspaces were reset; 2 highlights could not be restored")
        XCTAssertTrue(try XCTUnwrap(partial.persistence).isDirty, "the repaired store is saved")
        XCTAssertEqual(file.backups().count, 2)
        XCTAssertEqual(ShellPersistence.notice(for: [StoreIssue(part: "note", id: "n1", detail: ""), StoreIssue(part: "ink", id: "k1", detail: "")]),
                       "1 note could not be restored; 1 ink stroke could not be restored")
    }

    func testBackupsKeepTheNewestFive() throws {
        let start = Date(timeIntervalSince1970: 1_800_000_000)
        for i in 0..<8 { file.backup(Data("\(i)".utf8), at: start.addingTimeInterval(Double(i))) }
        let kept = file.backups()
        XCTAssertEqual(kept.count, 5)
        XCTAssertEqual(try Data(contentsOf: kept[0]), Data("7".utf8), "newest first")
        XCTAssertEqual(try Data(contentsOf: kept[4]), Data("3".utf8))
    }

    func testAFailingWriteKeepsTheOldFileAndSaysSoOnTheThirdTime() throws {
        // The store's directory is a file: every write fails.
        try Data("not a directory".utf8).write(to: directory)
        let model = ShellModel(store: file)
        for i in 0..<3 {
            model.run("column.new")
            model.persistence?.saveNow()
            XCTAssertEqual(model.persistence?.failures, i + 1)
        }
        XCTAssertEqual(model.lastNotice, ShellPersistence.failureNotice)
        XCTAssertTrue(try XCTUnwrap(model.persistence).isDirty, "tried again on the next change")
        try FileManager.default.removeItem(at: directory)
    }

    // MARK: Notes

    func testANoteSurvivesTheStoredFormExactly() throws {
        let note = StoreNote(id: "fixture:note:1", blocks: [
            StoreNoteBlock(kind: .heading, spans: [StoreNoteSpan(text: "New birth "), StoreNoteSpan(text: "Jn 3:3", link: "John.3.3")]),
            StoreNoteBlock(kind: .paragraph, spans: [StoreNoteSpan(text: "Not by "), StoreNoteSpan(text: "works", italic: true), StoreNoteSpan(text: " but "),
                                                      StoreNoteSpan(text: "from above", bold: true), StoreNoteSpan(text: ".")]),
            StoreNoteBlock(kind: .listItem, spans: [StoreNoteSpan(text: "Compare "), StoreNoteSpan(text: "Jn 1:12–13", link: "John.1.12-John.1.13")]),
            StoreNoteBlock(kind: .quote, spans: [StoreNoteSpan(text: "A fixture quote, not Scripture.")]),
            StoreNoteBlock(kind: .paragraph, spans: []),
        ])
        XCTAssertEqual(NoteStore.storeNote(id: note.id, from: NoteStore.attributed(note)), note, "the form both ways")
        // Through the editor: shown, restyled, saved unchanged.
        let model = ShellModel(store: file)
        try model.handle.putNote(note: note)
        let window = host(model)
        defer { window.close() }
        spin()
        let editor = try XCTUnwrap(model.notes.editor(pane: "t4"))
        XCTAssertTrue(editor.string.hasPrefix("New birth Jn 3:3\nNot by works"))
        let restyled = NoteStore.storeNote(id: note.id, from: try XCTUnwrap(editor.textStorage))
        XCTAssertEqual(restyled, note, "the editor's text is the same note")
        // An edit is put into the core when typing pauses.
        editor.window?.makeFirstResponder(editor)
        editor.setSelectedRange(NSRange(location: (editor.string as NSString).length, length: 0))
        editor.insertText("Last line", replacementRange: editor.selectedRange())
        XCTAssertTrue(model.notes.hasPending)
        spin(0.5)
        XCTAssertFalse(model.notes.hasPending)
        XCTAssertEqual(model.handle.note(id: note.id)?.blocks.last?.spans.map(\.text), ["Last line"])
    }

    // MARK: Drag and resize

    func testATabDragAndAColumnResizeUndo() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        let before = model.activeWorkspace.columns.map { $0.tabs.map(\.id) }
        XCTAssertEqual(before, [["t1", "t2"], ["t3"], ["t4"]])
        // Drag WLC (t2) by its label onto column 2's strip, before SBLGNT.
        let from = try XCTUnwrap(model.tabFrames["t2"]), target = try XCTUnwrap(model.tabFrames["t3"])
        model.dragTab("t2", to: CGPoint(x: from.midX + 10, y: from.midY))
        XCTAssertNil(model.tabDrag?.drop, "where it is: nothing to drop")
        model.dragTab("t2", to: CGPoint(x: target.minX + 3, y: target.midY))
        XCTAssertEqual(model.tabDrag?.drop, ShellModel.TabDrop(column: 2, index: 0))
        model.dropTab()
        XCTAssertEqual(model.activeWorkspace.columns.map { $0.tabs.map(\.id) }, [["t1"], ["t2", "t3"], ["t4"]])
        XCTAssertEqual(model.session.undoTitle, "Move tab to position")
        model.run("edit.undo")
        XCTAssertEqual(model.activeWorkspace.columns.map { $0.tabs.map(\.id) }, before, "⌘Z puts it back")
        spin(0.2)
        // Into a new column, through the trailing band.
        let last = try XCTUnwrap(model.columnFrames[3])
        model.dragTab("t2", to: CGPoint(x: last.maxX - 10, y: last.midY))
        XCTAssertEqual(model.tabDrag?.drop, ShellModel.TabDrop(column: 4, newColumn: true))
        model.dropTab()
        XCTAssertEqual(model.activeWorkspace.columns.map { $0.tabs.map(\.id) }, [["t1"], ["t3"], ["t4"], ["t2"]])
        model.run("edit.undo")
        XCTAssertEqual(model.activeWorkspace.columns.map { $0.tabs.map(\.id) }, before)
        // A drop where the tab already is sends nothing and says nothing.
        let toastBefore = model.toast
        XCTAssertFalse(model.moveTab("t3", to: ShellModel.TabDrop(column: 2, index: 0)))
        XCTAssertEqual(model.toast, toastBefore)
        // A resize commits once and undoes.
        let widths = model.activeWorkspace.columns.map(\.widthFraction)
        model.commitResize(column: 1, share: 0.3)
        XCTAssertEqual(model.activeWorkspace.columns[0].widthFraction, 0.3, accuracy: 0.001)
        XCTAssertEqual(model.session.undoTitle, "Resize column")
        model.run("edit.undo")
        XCTAssertEqual(model.activeWorkspace.columns.map(\.widthFraction), widths, "⌘Z restores the width")
    }

    // MARK: Palette and lexicon

    func testThePaletteSwitchesToAnotherWorkspacesTabAndToAWorkspace() throws {
        let model = ShellModel()
        model.focusPalette()
        model.paletteText = "Hebrew"
        model.paletteTextChanged()
        let row = try XCTUnwrap(model.paletteRows.first { $0.commandId == "workspace.switch" })
        model.runRow(row)
        XCTAssertEqual(model.session.activeWorkspaceId, "w2")
        model.run("workspace.switch", args: ShellCommandArgs(workspaceId: "w1"))
        model.focusPalette()
        model.paletteText = "@wlc"
        model.paletteTextChanged()
        let tab = try XCTUnwrap(model.paletteRows.first { $0.commandId == "tab.focus" && $0.args.workspaceId == "w2" }, "the other workspace's WLC tab")
        model.runRow(tab)
        XCTAssertEqual(model.session.activeWorkspaceId, "w2")
    }

    func testARestoredLexiconTabLooksUpInTheClickedEditionsLanguage() throws {
        let model = ShellModel(store: file)
        let window = host(model)
        spin()
        model.focusColumn(2)
        model.run("tab.open_lexicon", args: ShellCommandArgs(text: "ἠγάπησεν", reference: "John.3.16"))
        let lexicon = try XCTUnwrap(model.session.workspaces[0].columns.flatMap(\.tabs).first { $0.kind == "lexicon" })
        XCTAssertEqual(lexicon.lexiconLanguage, "grc")
        model.persistence?.saveNow()
        window.close()
        let back = ShellModel(store: file)
        let restored = try XCTUnwrap(back.session.workspaces[0].columns.flatMap(\.tabs).first { $0.kind == "lexicon" })
        XCTAssertEqual(restored.lexiconLanguage, "grc")
        XCTAssertEqual(LexiconView.language(restored, word: "ἠγάπησεν"), "grc")
        XCTAssertEqual(ShellReference.languageName(LexiconView.language(restored, word: "x")), "Greek", "the tab's language, not the word's script")
    }
}
