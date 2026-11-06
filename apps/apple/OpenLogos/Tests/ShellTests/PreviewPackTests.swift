// B071: the Mac app over installed text packs. Most of this runs on a small pack the test writes
// itself (made-up sentences, the shapes the real editions have: an absent verse, a chapter that
// ends early, tanakh_bhs order, right-to-left) with the made-up search packs in fixtures/search.
// `RealPackTests` reads the built preview pack and skips, visibly, on a machine without one.
import AppKit
import CryptoKit
import SwiftUI
import XCTest
@testable import OpenLogos

/// Writes a text pack directory: `chapters` are (book, chapter, verse numbers printed, absent).
enum TestPack {
    struct Chapter {
        let book: String
        let number: Int
        let verses: [Int]
        var absent: [Int] = []
        /// Text by verse number, where the default will not do.
        var text: [Int: String] = [:]
    }

    static func write(_ id: String, title: String, name: String, language: String, direction: String, scheme: String, order: String,
                      revision: String, attribution: String?, chapters: [Chapter], under root: URL) throws {
        let directory = root.appendingPathComponent("packs/\(id)", isDirectory: true)
        var books: [[String: Any]] = []
        for chapter in chapters {
            let key = "\(chapter.book).\(chapter.number)"
            let verses: [[String: Any]] = chapter.verses.map { number in
                let text = chapter.text[number] ?? "Made-up sentence \(number) of \(key) in \(id)."
                return ["segment_id": "\(key).\(number)", "number": number, "last_number": number, "label": "\(number)", "text": text,
                        "paragraph_start": number == chapter.verses.first, "poetry": false, "superscription": false, "placeholder": false, "tokens": [[String: Any]]()]
            }
            let chunk: [String: Any] = ["format": "openlogos-text-pack-chapter", "resource_id": id, "chapter_key": key, "verses": verses]
            let data = try JSONSerialization.data(withJSONObject: chunk, options: [.sortedKeys])
            let path = "chapters/\(chapter.book)/\(chapter.number).json"
            let file = directory.appendingPathComponent(path)
            try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
            try data.write(to: file)
            var entry: [String: Any] = ["chapter": chapter.number, "first_verse": chapter.verses.first!, "last_verse": chapter.verses.last!, "path": path,
                                        "length": data.count, "sha256": SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()]
            if !chapter.absent.isEmpty { entry["absent"] = chapter.absent }
            if let index = books.firstIndex(where: { $0["book"] as? String == chapter.book }) {
                books[index]["chapters"] = (books[index]["chapters"] as! [[String: Any]]) + [entry]
            } else {
                books.append(["book": chapter.book, "title": chapter.book, "chapters": [entry]])
            }
        }
        var manifest: [String: Any] = ["format": "openlogos-text-pack", "format_version": "1.1", "resource_id": id, "content_revision": revision,
                                       "title": title, "name": name, "language": language, "direction": direction, "scheme": scheme, "book_order": order,
                                       "license": "test", "source": "made up for tests", "fixture_only": true, "complete": true, "books": books]
        if let attribution { manifest["attribution"] = attribution }
        try JSONSerialization.data(withJSONObject: manifest, options: [.sortedKeys]).write(to: directory.appendingPathComponent("manifest.json"))
    }

    /// The made-up search packs, repository relative.
    static var searchFixtures: URL {
        URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../../../fixtures/search").standardized
    }

    /// BSB and SBLGNT with the awkward shapes; WLC is left out (it stays a placeholder).
    static func standard(under root: URL, search: Bool = true) throws {
        try write("bsb", title: "BSB", name: "Berean Standard Bible", language: "en", direction: "ltr", scheme: "eng", order: "protestant",
                  revision: "bsb-test-1", attribution: "BSB attribution words.",
                  chapters: [Chapter(book: "Mark", number: 9, verses: Array(40...50)), Chapter(book: "John", number: 3, verses: Array(1...36)),
                             Chapter(book: "Rom", number: 16, verses: Array(20...27))], under: root)
        try write("sblgnt", title: "SBLGNT", name: "SBL Greek New Testament", language: "grc", direction: "ltr", scheme: "org", order: "protestant",
                  revision: "sblgnt-test-1", attribution: "SBLGNT attribution words.",
                  chapters: [Chapter(book: "Mark", number: 9, verses: [40, 41, 42, 43, 45, 47, 48, 49, 50], absent: [44, 46]),
                             Chapter(book: "John", number: 3, verses: Array(1...36)), Chapter(book: "Rom", number: 16, verses: Array(20...24))], under: root)
        if search {
            try FileManager.default.copyItem(at: searchFixtures, to: root.appendingPathComponent("search"))
        }
    }
}

@MainActor
class PackTestCase: XCTestCase {
    var windows: [NSWindow] = []
    var root: URL!

    override func setUp() async throws {
        root = FileManager.default.temporaryDirectory.appendingPathComponent("openlogos-pack-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    }

    override func tearDown() async throws {
        windows.forEach { $0.close() }
        windows = []
        try? FileManager.default.removeItem(at: root)
    }

    func host(_ model: ShellModel) {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1440, height: 900), styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: ShellView(model: model))
        window.makeKeyAndOrderFront(nil)
        window.contentView?.layoutSubtreeIfNeeded()
        window.displayIfNeeded()
        windows.append(window)
    }

    func spin(_ seconds: TimeInterval = 0.4) {
        RunLoop.main.run(until: Date().addingTimeInterval(seconds))
    }

    func type(_ text: String, _ model: ShellModel) {
        model.focusPalette()
        model.paletteText = text
        model.paletteTextChanged()
        model.runSelectedRow()
        spin(0.3)
    }

    /// The field typed into, and the run loop turned until the Text section has landed.
    func search(_ text: String, _ model: ShellModel) {
        model.focusPalette()
        model.paletteText = text
        model.paletteTextChanged()
        spin(0.15)
    }

    func textRows(_ model: ShellModel) -> [ShellPaletteItem] {
        model.paletteResult?.groups.first { $0.section == .text }?.items ?? []
    }
}

final class PreviewPackTests: PackTestCase {
    // MARK: Where it is read from

    func testTheLocationIsTheDefaultOrAnOverrideAndTestsNeverLook() {
        let defaults = UserDefaults(suiteName: "openlogos-pack-test-\(UUID().uuidString)")!
        XCTAssertNil(PreviewPack.location(defaults: defaults, environment: ["XCTestConfigurationFilePath": "x"]), "tests open what they name")
        defaults.set("/somewhere/pack", forKey: PreviewPack.environmentKey)
        XCTAssertEqual(PreviewPack.location(defaults: defaults, environment: [:])?.path, "/somewhere/pack", "the launch argument")
        defaults.set("none", forKey: PreviewPack.environmentKey)
        XCTAssertNil(PreviewPack.location(defaults: defaults, environment: [:]), "`none`: the placeholder library")
    }

    func testNoPackAtAllIsNoPackAndTheAppStaysOnThePlaceholderSayingSo() throws {
        XCTAssertNil(PreviewPack.open(root: root.appendingPathComponent("nowhere")))
        let model = ShellModel(pack: nil)
        host(model)
        spin()
        let layout = try XCTUnwrap(model.stream(for: "t1")?.surface.layout)
        XCTAssertTrue(layout.display.string.contains("Placeholder text — this edition is not installed"))
        XCTAssertNil(model.textSource.resource("bsb").attribution, "nothing to attribute")
    }

    // MARK: Install

    func testPacksReplaceThePlaceholderAndAnEditionWithoutOneStaysAPlaceholder() throws {
        try TestPack.standard(under: root)
        let pack = try XCTUnwrap(PreviewPack.open(root: root))
        XCTAssertEqual(Set(pack.installed.keys), ["bsb", "sblgnt"])
        XCTAssertTrue(pack.problems.isEmpty, "\(pack.problems)")
        XCTAssertNotNil(pack.search)
        let model = ShellModel(pack: pack)
        host(model)
        spin()
        let stream = try XCTUnwrap(model.stream(for: "t1"))
        let layout = try XCTUnwrap(stream.surface.layout)
        XCTAssertTrue(layout.display.string.contains("Made-up sentence 16 of John.3 in bsb."))
        XCTAssertFalse(layout.display.string.contains("Placeholder text"), "no notice over installed text")
        XCTAssertFalse(layout.display.string.contains("Fixture:"), "no fixture licence line under installed text")
        XCTAssertTrue(layout.placeholderSegments.isEmpty)
        XCTAssertEqual(stream.blocks.first?.contentRevision, "bsb-test-1")
        // The WLC has no pack here: still the placeholder edition, and it says so.
        let wlc = try model.reading.chapterContent(resourceId: "wlc", chapterKey: "Gen.1")
        XCTAssertTrue(wlc.placeholder)
        XCTAssertNil(model.textSource.resource("wlc").attribution)
    }

    func testAPackThatDoesNotOpenStaysAPlaceholderAndIsNamed() throws {
        try TestPack.standard(under: root)
        try Data("not a manifest".utf8).write(to: root.appendingPathComponent("packs/sblgnt/manifest.json"))
        let pack = try XCTUnwrap(PreviewPack.open(root: root))
        XCTAssertEqual(Set(pack.installed.keys), ["bsb"])
        XCTAssertEqual(pack.problems.count, 1)
        XCTAssertTrue(pack.problems[0].hasPrefix("sblgnt:"), pack.problems[0])
        XCTAssertTrue(try pack.reading.chapterContent(resourceId: "sblgnt", chapterKey: "John.3").placeholder)
    }

    func testAChangedChunkIsRefusedNotShown() throws {
        try TestPack.standard(under: root)
        let chunk = root.appendingPathComponent("packs/bsb/chapters/John/3.json")
        var bytes = try Data(contentsOf: chunk)
        bytes[bytes.count / 2] ^= 1
        try bytes.write(to: chunk)
        let pack = try XCTUnwrap(PreviewPack.open(root: root))
        XCTAssertThrowsError(try pack.reading.chapterContent(resourceId: "bsb", chapterKey: "John.3"))
    }

    // MARK: Attribution

    func testAttributionComesWordForWordFromTheManifestAndIsNotInTheText() throws {
        try TestPack.standard(under: root)
        let model = ShellModel(pack: PreviewPack.open(root: root))
        XCTAssertEqual(model.textSource.resource("sblgnt").attribution, "SBLGNT attribution words.")
        XCTAssertEqual(model.textSource.resource("sblgnt").revision, "sblgnt-test-1")
        XCTAssertEqual(model.textSource.resource("sblgnt").name, "SBL Greek New Testament")
        XCTAssertNil(model.textSource.resource("fixture:note:1").attribution)
        host(model)
        spin()
        XCTAssertFalse(try XCTUnwrap(model.stream(for: "t3")?.surface.layout).display.string.contains("attribution words"))
    }

    func testAboutThisEditionIsTheCoresCommandOnAReadingTab() throws {
        try TestPack.standard(under: root)
        let model = ShellModel(pack: PreviewPack.open(root: root))
        host(model)
        spin()
        model.focusColumn(2)
        model.focusPalette()
        model.paletteText = ">licence"
        model.paletteTextChanged()
        spin(0.1)
        let row = try XCTUnwrap(model.paletteRows.first { $0.commandId == "view.about_edition" }, "the palette finds it")
        XCTAssertEqual(row.title, "About this edition")
        model.runRow(row)
        spin(0.1)
        XCTAssertEqual(model.aboutColumn, 2, "the active column's panel")
        model.aboutColumn = nil
        // Its addresses can be followed; the words stay as they are.
        let linked = AboutEditionPanel.linked("Licence: https://example.org/by/4.0/. Source.")
        XCTAssertEqual(String(linked.characters), "Licence: https://example.org/by/4.0/. Source.")
        XCTAssertEqual(linked.runs.compactMap(\.link).map(\.absoluteString), ["https://example.org/by/4.0/"])
        // The note's column: not a reading tab, so the core disables it.
        model.focusColumn(3)
        spin(0.1)
        model.run("view.about_edition")
        XCTAssertNil(model.aboutColumn)
    }

    // MARK: Followers

    func testAFollowerLandsBesideAVerseItsEditionDoesNotPrint() throws {
        try TestPack.standard(under: root)
        let model = ShellModel(pack: PreviewPack.open(root: root))
        host(model)
        spin()
        // BSB prints 44, the linked SBLGNT does not.
        type("Mk 9:44", model)
        spin()
        XCTAssertEqual(model.stream(for: "t1")?.topSegment, "Mark.9.44")
        let follower = try XCTUnwrap(model.stream(for: "t3"))
        XCTAssertEqual(follower.topSegment, "Mark.9.43", "the last verse numbered at or below it")
        XCTAssertNil(model.readingError(for: try XCTUnwrap(model.activeWorkspace.columns[1].tabs.first)))
        // And scrolling onto 46: the follower moves to 45.
        follower.surface.layout.map { _ in }
        model.recordScroll(tabID: "t1", segmentID: "Mark.9.46")
        spin(0.1)
        XCTAssertEqual(follower.topSegment, "Mark.9.45")
    }

    func testAFollowerPastItsChaptersLastVerseStaysOnTheChapter() throws {
        try TestPack.standard(under: root)
        let model = ShellModel(pack: PreviewPack.open(root: root))
        host(model)
        spin()
        type("Rom 16:25", model)
        spin()
        let tab = try XCTUnwrap(model.activeWorkspace.columns[1].tabs.first { $0.id == "t3" })
        XCTAssertEqual(tab.reference, "Rom.16.25", "the core's mapping is kept")
        XCTAssertNil(model.readingError(for: tab), "a follower is not refused")
        XCTAssertEqual(model.stream(for: "t3")?.topSegment, "Rom.16.24")
        // The same verse typed in that tab is an honest refusal.
        model.focusColumn(2)
        spin(0.1)
        type("Rom 16:26", model)
        let typed = try XCTUnwrap(model.activeWorkspace.columns[1].tabs.first { $0.id == "t3" })
        XCTAssertNotNil(model.readingError(for: typed), "typed here, this edition has no such verse")
    }

    func testARestoredFollowerPastTheChapterOpensItsChapter() throws {
        try TestPack.standard(under: root)
        let directory = root.appendingPathComponent("store", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let file = UserStoreFile(directory: directory)
        let model = ShellModel(store: file, pack: PreviewPack.open(root: root))
        host(model)
        spin()
        type("Rom 16:25", model)
        spin()
        model.persistence?.saveNow()
        windows.forEach { $0.close() }
        windows = []
        let back = ShellModel(store: file, pack: PreviewPack.open(root: root))
        host(back)
        spin(0.8)
        XCTAssertEqual(back.stream(for: "t3")?.topBlock?.chapterKey, "Rom.16")
    }

    // MARK: Search

    func testTextRowsArriveAfterTheOthersAndAHitOpensTheVerse() throws {
        try TestPack.standard(under: root)
        let model = ShellModel(pack: PreviewPack.open(root: root))
        host(model)
        spin()
        model.focusPalette()
        model.paletteText = "quag"
        model.paletteTextChanged()
        XCTAssertTrue(textRows(model).isEmpty, "off the main thread: not in the keystroke's own answer")
        spin(0.15)
        let row = try XCTUnwrap(textRows(model).first)
        XCTAssertEqual(row.title, "BSB · Mk 9:43")
        XCTAssertTrue(row.subtitle?.contains("quagga") == true)
        model.runRow(row)
        spin()
        XCTAssertEqual(model.stream(for: "t1")?.topSegment, "Mark.9.43")
        XCTAssertEqual(model.activeTab?.reference, "Mark.9.43")
        XCTAssertFalse(model.paletteOpen)
    }

    func testAHitFromAnotherEditionOpensThatEditionsVerseInANewTab() throws {
        try TestPack.standard(under: root)
        let model = ShellModel(pack: PreviewPack.open(root: root))
        host(model)
        spin()
        let column = model.activeWorkspace.columns[0]
        let before = try XCTUnwrap(model.activeTab)
        search("#ζεβρας", model)
        let row = try XCTUnwrap(textRows(model).first)
        XCTAssertEqual(row.title, "SBLGNT · Jn 3:16")
        XCTAssertEqual(row.args.resourceId, "sblgnt")
        model.runRow(row)
        spin()
        let opened = try XCTUnwrap(model.activeTab)
        XCTAssertEqual(opened.contentId, "sblgnt")
        XCTAssertEqual(opened.reference, "John.3.16")
        XCTAssertNotEqual(opened.id, before.id)
        XCTAssertEqual(model.activeWorkspace.columns[0].tabs.count, column.tabs.count + 1)
        XCTAssertEqual(model.stream(for: opened.id)?.topSegment, "John.3.16")
        XCTAssertEqual(model.activeWorkspace.columns[0].tabs.first { $0.id == before.id }?.reference, before.reference, "the shown tab is left alone")
    }

    func testEnterActsOnTheRowsForWhatIsTypedNow() throws {
        try TestPack.standard(under: root)
        let model = ShellModel(pack: PreviewPack.open(root: root))
        host(model)
        spin()
        // No run loop turn between the keystroke and Enter: the answer has not been delivered.
        model.focusPalette()
        model.paletteText = "#quagga"
        model.paletteTextChanged()
        XCTAssertTrue(textRows(model).isEmpty)
        model.runSelectedRow()
        spin()
        XCTAssertEqual(model.activeTab?.reference, "Mark.9.43")
    }

    func testTheRowsOfTheLastKeystrokeStayUntilTheNextAnswerAndGoWhenTextIsNotAsked() throws {
        try TestPack.standard(under: root)
        let model = ShellModel(pack: PreviewPack.open(root: root))
        host(model)
        spin()
        search("zebra", model)
        XCTAssertFalse(textRows(model).isEmpty)
        model.paletteText = "zebraq"
        model.paletteTextChanged()
        XCTAssertFalse(textRows(model).isEmpty, "held until the new answer")
        spin(0.15)
        XCTAssertTrue(textRows(model).isEmpty, "the new answer: nothing")
        search("zebra", model)
        XCTAssertFalse(textRows(model).isEmpty)
        model.paletteText = ">zebra"
        model.paletteTextChanged()
        XCTAssertTrue(textRows(model).isEmpty, "commands do not search the text")
        spin(0.15)
        XCTAssertTrue(textRows(model).isEmpty, "and nothing lands afterwards")
    }

    func testARowsTargetColumnIsCheckedAgainstTheRealWorkspace() throws {
        try TestPack.standard(under: root)
        let model = ShellModel(pack: PreviewPack.open(root: root))
        host(model)
        spin()
        search("#zebra @3", model)
        let enabled = try XCTUnwrap(textRows(model).first)
        XCTAssertEqual(enabled.args.column, 3)
        XCTAssertNil(enabled.disabledReason, "the workspace has three columns; the search handle has one")
        search("#zebra @7", model)
        let refused = try XCTUnwrap(textRows(model).first)
        XCTAssertNotNil(refused.disabledReason, "no column 7, in the core's words")
    }

    func testCloseCancelsWhatIsInFlight() throws {
        try TestPack.standard(under: root)
        let model = ShellModel(pack: PreviewPack.open(root: root))
        host(model)
        spin()
        model.focusPalette()
        model.paletteText = "zebra"
        model.paletteTextChanged()
        model.closePalette()
        spin(0.15)
        XCTAssertNil(model.paletteResult)
        XCTAssertFalse(model.paletteOpen)
    }

    func testPackWithoutSearchHasNoTextRowsAndNoFixtureRows() throws {
        try TestPack.standard(under: root, search: false)
        let pack = try XCTUnwrap(PreviewPack.open(root: root))
        XCTAssertNil(pack.search)
        let model = ShellModel(pack: pack)
        host(model)
        spin()
        search("#loved", model)
        XCTAssertTrue(textRows(model).isEmpty)
    }

    // MARK: A session saved on placeholder text

    func testASessionSavedOnThePlaceholderRestoresOnPacksWithItsHighlightOnTheSameWords() throws {
        // John 3:16 in the pack has the fixture's bytes, as the real editions do.
        let placeholder = try ReadingHandle.placeholder().chapterContent(resourceId: "bsb", chapterKey: "John.3")
        let sixteen = try XCTUnwrap(placeholder.verses.first { $0.segmentId == "John.3.16" }).text
        try TestPack.write("bsb", title: "BSB", name: "Berean Standard Bible", language: "en", direction: "ltr", scheme: "eng", order: "protestant",
                           revision: "bsb-test-2", attribution: "Words.", chapters: [TestPack.Chapter(book: "John", number: 3, verses: Array(1...36), text: [16: sixteen])], under: root)
        let directory = root.appendingPathComponent("store", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let file = UserStoreFile(directory: directory)

        let before = ShellModel(store: file)
        host(before)
        spin()
        let stream = try XCTUnwrap(before.stream(for: "t1"))
        let layout = try XCTUnwrap(stream.surface.layout)
        before.run("tool.highlight")
        spin(0.1)
        stream.surface.select(display: layout.runMap.displayRange(fromCore: try XCTUnwrap(layout.coreRange(ofPhrase: "loved the world"))))
        stream.surface.dragEnded()
        XCTAssertEqual(before.highlights("t1").count, 1)
        before.run("tool.select")
        StoreLaunch.drawInk(before, stream: stream, on: "John.3.17")
        before.persistence?.saveNow()
        let anchor = before.activeWorkspace.columns[0].tabs[0].scrollSegmentId
        windows.forEach { $0.close() }
        windows = []

        let after = ShellModel(store: file, pack: PreviewPack.open(root: root))
        XCTAssertNil(after.restored?.notice, "nothing set aside")
        host(after)
        spin(0.8)
        let real = try XCTUnwrap(after.stream(for: "t1"))
        XCTAssertEqual(real.blocks.first?.contentRevision, "bsb-test-2")
        XCTAssertEqual(after.activeWorkspace.columns[0].tabs[0].scrollSegmentId, anchor)
        let realLayout = try XCTUnwrap(real.surface.layout)
        XCTAssertTrue(realLayout.placeholderSegments.isEmpty)
        let highlight = try XCTUnwrap(after.handle.highlightsIn(resourceId: "bsb", chapterKey: "John.3").first)
        XCTAssertEqual(highlight.contentRevision, "bsb-2024", "as it was saved; the spans still fit this verse")
        let span = try XCTUnwrap(highlight.spans.first.flatMap(realLayout.segmentSpan))
        let shown = ((realLayout.segmentTexts["John.3.16"] ?? "") as NSString).substring(with: NSRange(location: span.start, length: span.length))
        XCTAssertEqual(shown, "loved the world")
        XCTAssertEqual(after.highlights("t1").count, 1, "painted")
        XCTAssertFalse(after.handle.inkIn(resourceId: "bsb", chapterKey: "John.3").isEmpty)
    }

    func testAHighlightThatNoLongerFitsItsVerseIsNotPaintedAndIsKept() throws {
        // The revision changed the verse: shorter than the span. It paints nothing and the app
        // says nothing; the highlight stays in the store for a later lineage (specs/03).
        try TestPack.write("bsb", title: "BSB", name: "B", language: "en", direction: "ltr", scheme: "eng", order: "protestant", revision: "r2", attribution: nil,
                           chapters: [TestPack.Chapter(book: "John", number: 3, verses: [16, 17], text: [16: "Short."])], under: root)
        let model = ShellModel(pack: PreviewPack.open(root: root))
        _ = try model.handle.addHighlight(highlight: StoreHighlight(resourceId: "bsb", contentRevision: "r1", spans: [StoreSegmentSpan(segmentId: "John.3.16", start: 2, end: 40)], color: "#FFD60A"), reading: model.reading)
        host(model)
        spin()
        XCTAssertTrue(model.highlights("t1").isEmpty)
        XCTAssertEqual(model.handle.highlightsIn(resourceId: "bsb", chapterKey: "John.3").count, 1)
    }

    func testAnEditionsOwnNewRevisionIsWhatItsAnchorsNowName() throws {
        try TestPack.standard(under: root)
        let model = ShellModel(pack: PreviewPack.open(root: root))
        host(model)
        spin()
        let stream = try XCTUnwrap(model.stream(for: "t1"))
        let layout = try XCTUnwrap(stream.surface.layout)
        XCTAssertEqual(layout.content.contentRevision, "bsb-test-1")
        stream.surface.select(display: layout.runMap.displayRange(fromCore: try XCTUnwrap(layout.coreRange(ofPhrase: "sentence 16"))))
        spin(0.05)
        let selection = try XCTUnwrap(model.selection)
        XCTAssertTrue(selection.anchors.allSatisfy { $0.contentRevision == "bsb-test-1" && $0.editionId == "bsb" })
        // A highlight on real text is allowed and carries the new revision.
        model.run("selection.highlight")
        XCTAssertEqual(model.handle.highlightsIn(resourceId: "bsb", chapterKey: "John.3").first?.contentRevision, "bsb-test-1")
    }
}

/// Over the built preview pack (docs/preview-pack.md): the things only the real editions show.
final class RealPackTests: PackTestCase {
    private func real() throws -> PreviewPack {
        let environment = ProcessInfo.processInfo.environment[PreviewPack.environmentKey]
        let path = environment ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".cache/openlogos/preview-pack").path
        guard let pack = PreviewPack.open(root: URL(fileURLWithPath: path)), pack.installed.count == 3 else {
            throw XCTSkip("SKIPPED: no built preview pack at \(path) (just preview-pack)")
        }
        return pack
    }

    func testTheThreeEditionsInstallWithTheirAttribution() throws {
        let pack = try real()
        XCTAssertNotNil(pack.search)
        XCTAssertTrue(pack.problems.isEmpty, "\(pack.problems)")
        for id in PreviewPack.editions {
            XCTAssertFalse(pack.installed[id]?.placeholder ?? true, id)
            XCTAssertFalse((pack.installed[id]?.attribution ?? "").isEmpty, id)
        }
        XCTAssertTrue(pack.installed["sblgnt"]?.attribution?.contains("CC BY 4.0") == true)
        XCTAssertTrue(pack.installed["wlc"]?.attribution?.contains("CC BY 4.0") == true)
    }

    func testTheWLCReadsRightToLeftInTanakhOrder() throws {
        let pack = try real()
        XCTAssertEqual(pack.installed["wlc"]?.direction, "rtl")
        XCTAssertEqual(pack.installed["wlc"]?.bookOrder, "tanakh_bhs")
        let reading = pack.reading
        XCTAssertEqual(try reading.chapterNeighbors(resourceId: "wlc", chapterKey: "Mal.3").next, "Ps.1", "Malachi 3 → Psalms")
        XCTAssertNil(try reading.chapterNeighbors(resourceId: "wlc", chapterKey: "2Chr.36").next, "ends at 2 Chronicles")
        let model = ShellModel(pack: pack)
        XCTAssertTrue(model.textSource.resource("wlc").isRightToLeft)
        host(model)
        spin()
        // Column 1's second tab; then scroll over the edge of Malachi 3 into Psalms.
        model.focusColumn(1)
        model.run("tab.next")
        spin()
        type("Mal 3:24", model)
        spin()
        let stream = try XCTUnwrap(model.stream(for: "t2"))
        XCTAssertEqual(stream.topSegment, "Mal.3.24")
        XCTAssertTrue(stream.blocks.contains { $0.chapterKey == "Ps.1" }, "\(stream.blocks.map(\.chapterKey))")
        XCTAssertFalse(stream.blocks.contains { $0.chapterKey == "Mal.4" })
    }

    func testFollowersOnTheEditionsOwnAbsentVerses() throws {
        let model = ShellModel(pack: try real())
        host(model)
        spin()
        // Column 1 BSB and column 2 SBLGNT are linked.
        type("Rom 16:25", model)
        spin()
        XCTAssertEqual(model.stream(for: "t3")?.topSegment, "Rom.16.24", "SBLGNT ends Romans at 16:24")
        type("Matt 17:21", model)
        spin()
        // The BSB does not print it either: `locate` lands the driver beside it.
        let driver = try XCTUnwrap(model.stream(for: "t1")?.topSegment)
        XCTAssertTrue(["Matt.17.20", "Matt.17.22"].contains(driver), driver)
        let follower = try XCTUnwrap(model.stream(for: "t3")?.topSegment)
        XCTAssertTrue(["Matt.17.20", "Matt.17.22"].contains(follower), follower)
    }

    func testPsalmVersificationThroughALinkedWLC() throws {
        let pack = try real()
        let model = ShellModel(sessionJSON: ShellSeed.json, pack: pack)
        host(model)
        spin()
        // BSB Ps 3:1 is the WLC's 3:2.
        model.focusColumn(1)
        model.run("tab.open_edition", args: ShellCommandArgs(reference: "Ps.3.1", resourceId: "wlc"))
        spin(0.2)
        XCTAssertNotNil(model.reading)
        XCTAssertEqual(try pack.reading.chapterContent(resourceId: "wlc", chapterKey: "Ps.3").verses.first?.segmentId, "Ps.3.1")
        XCTAssertEqual(try pack.reading.chapterContent(resourceId: "bsb", chapterKey: "Ps.3").verses.first?.segmentId, "Ps.3.0")
    }

    func testASessionSavedOnThePlaceholderRestoresOnTheRealEditions() throws {
        let pack = try real()
        let directory = root.appendingPathComponent("store", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let file = UserStoreFile(directory: directory)

        // On the placeholder library: a highlight on the fixture verse, ink, column 2 at Acts 2
        // scrolled, a tab at a verse the BSB does not print.
        let before = ShellModel(store: file)
        host(before)
        spin()
        let stream = try XCTUnwrap(before.stream(for: "t1"))
        let layout = try XCTUnwrap(stream.surface.layout)
        before.run("tool.highlight")
        spin(0.1)
        stream.surface.select(display: layout.runMap.displayRange(fromCore: try XCTUnwrap(layout.coreRange(ofPhrase: "loved the world"))))
        stream.surface.dragEnded()
        before.run("tool.select")
        StoreLaunch.drawInk(before, stream: stream, on: "John.3.17")
        before.focusColumn(2)
        before.run("tab.pin")
        before.run("nav.navigate", args: ShellCommandArgs(column: 2, reference: "Acts.2.14"))
        spin(0.5)
        before.persistence?.saveNow()
        let saved = before.session
        windows.forEach { $0.close() }
        windows = []

        let after = ShellModel(store: file, pack: pack)
        XCTAssertNil(after.restored?.notice, "nothing set aside")
        host(after)
        spin(1)
        let bsb = try XCTUnwrap(after.stream(for: "t1"))
        XCTAssertEqual(bsb.topSegment, saved.workspaces[0].columns[0].tabs[0].scrollSegmentId ?? "John.3.16")
        XCTAssertEqual(after.stream(for: "t3")?.topSegment, "Acts.2.14")
        XCTAssertFalse(try XCTUnwrap(bsb.surface.layout).display.string.contains("Placeholder text"))
        let highlight = try XCTUnwrap(after.handle.highlightsIn(resourceId: "bsb", chapterKey: "John.3").first)
        let realLayout = try XCTUnwrap(bsb.surface.layout)
        let span = try XCTUnwrap(highlight.spans.first.flatMap(realLayout.segmentSpan))
        XCTAssertEqual(((realLayout.segmentTexts["John.3.16"] ?? "") as NSString).substring(with: NSRange(location: span.start, length: span.length)), "loved the world")
        XCTAssertEqual(after.highlights("t1").count, 1)
        XCTAssertEqual(after.handle.inkIn(resourceId: "bsb", chapterKey: "John.3").count, 1)
        // Restoring moved nothing the reader did not move.
        XCTAssertEqual(after.session.workspaces[0].columns[0].tabs[0].reference, saved.workspaces[0].columns[0].tabs[0].reference)
        XCTAssertEqual(after.session.workspaces[0].columns[1].tabs[0].reference, saved.workspaces[0].columns[1].tabs[0].reference)
    }

    func testAPlaceAtAnEndOfTheEditionRestoresWhereItWas() throws {
        let pack = try real()
        for verse in ["Rev.22.21", "Rev.22.2", "Gen.1.2", "John.21.25"] {
            let directory = root.appendingPathComponent("store-\(verse)", isDirectory: true)
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let file = UserStoreFile(directory: directory)
            let model = ShellModel(store: file, pack: pack)
            host(model)
            spin()
            model.focusColumn(1)
            model.run("tab.pin")
            model.run("nav.navigate", args: ShellCommandArgs(column: 1, reference: verse))
            spin(0.5)
            XCTAssertEqual(model.stream(for: "t1")?.topSegment, verse)
            model.persistence?.saveNow()
            windows.forEach { $0.close() }
            windows = []
            let back = ShellModel(store: file, pack: pack)
            host(back)
            spin(0.8)
            XCTAssertEqual(back.stream(for: "t1")?.topSegment, verse)
            XCTAssertEqual(back.activeWorkspace.columns[0].tabs[0].reference, verse)
            windows.forEach { $0.close() }
            windows = []
        }
    }

    func testAWLCHitOnABSBTabOpensTheWLCsOwnVerse() throws {
        let model = ShellModel(pack: try real())
        host(model)
        spin()
        // The WLC numbers Malachi's end 3:19–24; the BSB ends the chapter at 18.
        model.run("nav.navigate", args: ShellCommandArgs(reference: "Mal.3.19", resourceId: "wlc"))
        spin()
        let opened = try XCTUnwrap(model.activeTab)
        XCTAssertEqual(opened.contentId, "wlc")
        XCTAssertNil(model.readingError(for: opened))
        XCTAssertEqual(model.stream(for: opened.id)?.topSegment, "Mal.3.19")
        XCTAssertEqual(model.stream(for: opened.id)?.surface.layout?.content.isRightToLeft, true)
    }

    func testASearchHitOpensTheVerseOverTheRealEditions() throws {
        let model = ShellModel(pack: try real())
        host(model)
        spin()
        search("#only begotten son of", model)
        search("#in the beginning", model)
        let row = try XCTUnwrap(textRows(model).first, "the phrase is found")
        model.runRow(row)
        spin()
        XCTAssertEqual(model.activeTab?.reference, row.args.reference)
    }
}
