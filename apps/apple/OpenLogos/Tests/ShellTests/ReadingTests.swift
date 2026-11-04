// B078: continuous reading in the Mac shell (specs/23 §7), hosted in the app over the real
// UniFFI handles: a typed passage opens its chapter with the passage at the top, flashed and
// marked; the column scrolls on across chapters and books both ways without the text moving
// under the eye; the header, tab label and identity row follow; linked tabs follow verse for
// verse through versification; Back and Forward; the not-in-edition state; placeholder text
// cannot be quoted or collected; highlights stay on their verses as blocks come and go; only
// a window of chapters is ever in the text view.
import AppKit
import SwiftUI
import XCTest
@testable import OpenLogos

@MainActor
final class ReadingTests: XCTestCase {
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

    /// Types a passage in the palette and runs its first row, as the owner will.
    private func type(_ text: String, _ model: ShellModel) {
        model.focusPalette()
        model.paletteText = text
        model.paletteTextChanged()
        model.runSelectedRow()
        spin(0.3)
    }

    private func origin(_ stream: ReadingStream) -> CGFloat { stream.container?.scrollView.contentView.bounds.minY ?? -1 }

    /// Where a segment's first line is on screen, below the top of the visible text.
    private func screenY(_ stream: ReadingStream, _ segment: String) -> CGFloat? {
        stream.top(of: segment).map { $0 - origin(stream) }
    }

    /// `John.3.16` → ("John.3", 16).
    private func place(_ segment: String?) -> (chapter: String, verse: Int)? {
        guard let segment else { return nil }
        let parts = segment.split(separator: ".")
        guard parts.count == 3, let verse = Int(parts[2].prefix { $0.isNumber }) else { return nil }
        return ("\(parts[0]).\(parts[1])", verse)
    }

    private func at(_ stream: ReadingStream, _ chapter: String, atLeast verse: Int) -> Bool {
        guard let place = place(stream.topSegment) else { return false }
        return place.chapter == chapter && place.verse >= verse
    }

    private func shownTab(_ model: ShellModel, column: Int) -> ShellTab? {
        let c = model.activeWorkspace.columns[column - 1]
        return c.tabs.first { $0.id == c.shownTabId }
    }

    /// Scrolls by `step` until `done` or `limit` steps, checking every step that a verse on
    /// screen moved by exactly the step (within 2 pt), blocks loading and unloading included.
    private func scroll(_ stream: ReadingStream, by step: CGFloat, limit: Int, file: StaticString = #filePath, line: UInt = #line,
                        until done: (ReadingStream) -> Bool) -> Int {
        var steps = 0
        var worst: CGFloat = 0
        while steps < limit, !done(stream) {
            let anchor = stream.topSegment ?? ""
            let before = screenY(stream, anchor)
            stream.scrollAsReader(by: step)
            steps += 1
            if let before, let after = screenY(stream, anchor) {
                worst = max(worst, abs((before - step) - after))
            }
            XCTAssertLessThanOrEqual(stream.blocks.count, ReadingStream.hardMaxBlocks, "only a window of chapters", file: file, line: line)
        }
        XCTAssertLessThanOrEqual(worst, 2, "the text never jumps under the eye (worst \(worst) pt)", file: file, line: line)
        return steps
    }

    // MARK: Opening a passage

    func testATypedPassageOpensItsChapterWithTheVerseAtTheTopFlashedAndMarked() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        type("Jn 3:16", model)
        let stream = try XCTUnwrap(model.stream(for: "t1"))
        XCTAssertEqual(stream.topSegment, "John.3.16")
        XCTAssertEqual(try XCTUnwrap(screenY(stream, "John.3.16")), ReadingStream.topMargin, accuracy: 1, "the verse's first line at the top")
        XCTAssertTrue(stream.blocks.map(\.chapterKey).contains("John.2") && stream.blocks.map(\.chapterKey).contains("John.4"), "the chapters around it")
        XCTAssertEqual(stream.container?.header.text, "Jn 3 · BSB")
        XCTAssertEqual(shownTab(model, column: 1)?.highlightRange, "John.3.16")
        XCTAssertTrue(stream.isFlashing, "flashed once")
        let mark = try XCTUnwrap(stream.surface.overlay.first)
        let layout = try XCTUnwrap(stream.surface.layout)
        XCTAssertEqual(mark.ranges, [layout.verseRanges["John.3.16"]!])
        spin(1.1)
        XCTAssertFalse(stream.isFlashing)
        let quiet = try XCTUnwrap(stream.surface.overlay.first?.color.alphaComponent)
        XCTAssertEqual(quiet, ReadingStream.quietMark, accuracy: 0.01, "then a quiet mark")
        let painted = stream.surface.storage?.attribute(.backgroundColor, at: layout.verseRanges["John.3.16"]!.location + 4, effectiveRange: nil)
        XCTAssertNotNil(painted, "the mark is painted")
        // The linked SBLGNT column went to the same verse, marked too.
        let greek = try XCTUnwrap(model.stream(for: "t3"))
        XCTAssertEqual(greek.topSegment, "John.3.16")
        XCTAssertEqual(shownTab(model, column: 2)?.highlightRange, "John.3.16")
        XCTAssertFalse(greek.surface.overlay.isEmpty)
        XCTAssertEqual(shownTab(model, column: 1).map { TabTitle.label($0) }, "BSB Jn 3")
    }

    func testRangesBooksChaptersAndOneChapterBooksOpenWhereTheyShould() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        let stream = try XCTUnwrap(model.stream(for: "t1"))

        type("John 3:16-18", model)
        XCTAssertEqual(shownTab(model, column: 1)?.highlightRange, "John.3.16-John.3.18")
        let layout = try XCTUnwrap(stream.surface.layout)
        XCTAssertEqual(stream.surface.overlay.first?.ranges, [layout.displayRange(from: "John.3.16", to: "John.3.18")!], "the three verses marked")

        type("Jude 3", model)
        XCTAssertEqual(stream.topSegment, "Jude.1.3")
        // Three short chapters were added above since (one a frame), each anchored to the pixel.
        XCTAssertEqual(try XCTUnwrap(screenY(stream, "Jude.1.3")), ReadingStream.topMargin, accuracy: 2)
        XCTAssertEqual(stream.container?.header.text, "Jude · BSB")

        type("Ps 3", model)
        XCTAssertEqual(stream.topSegment, "Ps.3.0", "a titled Psalm opens at its title")
        XCTAssertNil(shownTab(model, column: 1)?.highlightRange, "a chapter marks nothing")
        XCTAssertTrue(stream.surface.overlay.isEmpty)

        type("Genesis", model)
        XCTAssertEqual(stream.topSegment, "Gen.1.1", "a whole book opens at its first verse")
        XCTAssertEqual(stream.blocks.first?.chapterKey, "Gen.1", "nothing before Genesis 1")
        XCTAssertEqual(stream.container?.header.text, "Gen 1 · BSB")
    }

    func testAPassageNotInTheEditionShowsTheNotInEditionState() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        model.run("nav.navigate", args: ShellCommandArgs(reference: "John.22"))
        spin()
        let tab = try XCTUnwrap(shownTab(model, column: 1))
        XCTAssertEqual(model.readingError(for: tab), "John has no chapter 22")
        XCTAssertNil(model.stream(for: "t1")?.container?.window, "no text is shown, none of another edition's")
        model.run("nav.navigate", args: ShellCommandArgs(reference: "John.3.16"))
        spin()
        XCTAssertEqual(model.stream(for: "t1")?.topSegment, "John.3.16", "the next passage opens again")

        // A WLC tab on John: the book is not in the edition.
        model.run("tab.next")
        spin(0.2)
        model.run("nav.navigate", args: ShellCommandArgs(reference: "John.3.16"))
        spin(0.2)
        XCTAssertEqual(model.readingError(for: try XCTUnwrap(shownTab(model, column: 1))), "John is not in WLC")
    }

    // MARK: The continuous scroll

    func testScrollingCrossesChaptersAndBooksBothWaysWithoutMovingTheText() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        type("John 21:20", model)
        let stream = try XCTUnwrap(model.stream(for: "t1"))
        // Down: John 21 into Acts 1, a book heading on the way.
        let steps = scroll(stream, by: 40, limit: 200) { self.at($0, "Acts.1", atLeast: 3) }
        XCTAssertLessThan(steps, 200)
        XCTAssertEqual(stream.container?.header.text, "Acts 1 · BSB")
        XCTAssertEqual(model.liveLabels.byTab["t1"]?.chapterLabel, "Acts 1", "the tab label follows while the reader scrolls")
        XCTAssertEqual(model.liveLabels.byTab["t3"]?.chapterLabel, "Acts 1", "and the linked column's")
        XCTAssertEqual(stream.surface.layout?.blocks.first { $0.chapterKey == "Acts.1" }.map { stream.surface.layout!.display.attributedSubstring(from: $0.headingRange).string }, "Acts\nChapter 1\n\(ReadingNotice.placeholder)\n")
        spin(0.4)
        XCTAssertEqual(shownTab(model, column: 1).map { TabTitle.label($0) }, "BSB Acts 1", "the tab label follows")
        XCTAssertEqual(shownTab(model, column: 1)?.referenceLabel?.hasPrefix("Acts 1:"), true, "the identity row follows")
        XCTAssertEqual(shownTab(model, column: 2).map { TabTitle.label($0) }, "SBLGNT Acts 1", "the linked column followed")
        // Up: back through John 21 and John 20 into John 19.
        _ = scroll(stream, by: -40, limit: 400) { $0.topSegment?.hasPrefix("John.19.") == true }
        XCTAssertTrue(stream.topSegment?.hasPrefix("John.19.") == true)
        XCTAssertEqual(stream.container?.header.text, "Jn 19 · BSB")

        // Malachi 4 into Matthew 1 (the testaments), and Genesis 2 up into Genesis 1.
        type("Mal 4", model)
        _ = scroll(stream, by: 40, limit: 200) { self.at($0, "Matt.1", atLeast: 2) }
        XCTAssertTrue(at(stream, "Matt.1", atLeast: 2))
        XCTAssertEqual(stream.container?.header.text, "Mt 1 · BSB")
        type("Gen 2:1", model)
        _ = scroll(stream, by: -40, limit: 200) { self.place($0.topSegment).map { $0.chapter == "Gen.1" && $0.verse <= 20 } == true }
        XCTAssertEqual(place(stream.topSegment)?.chapter, "Gen.1")
    }

    func testOnlyAWindowOfChaptersIsEverRendered() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        type("Matt 1", model)
        let stream = try XCTUnwrap(model.stream(for: "t1"))
        var seen = Set<String>()
        var longest = 0
        var steps = 0
        var over = 0
        while seen.count < 32, steps < 4000 {
            stream.scrollAsReader(by: 120)
            // A frame: the loads and releases that can wait run one per frame.
            spin(0.018)
            steps += 1
            seen.formUnion(stream.blocks.map(\.chapterKey))
            longest = max(longest, stream.surface.storage?.length ?? 0)
            XCTAssertLessThanOrEqual(stream.blocks.count, ReadingStream.hardMaxBlocks)
            if stream.blocks.count > ReadingStream.maxBlocks { over += 1 }
        }
        spin(0.4)
        XCTAssertLessThanOrEqual(stream.blocks.count, ReadingStream.maxBlocks, "when the reader stops, the window goes back to five blocks")
        XCTAssertGreaterThan(over, 0, "while scrolling, up to eight (releases wait)")
        XCTAssertGreaterThanOrEqual(seen.count, 32, "Matthew into Mark and beyond")
        XCTAssertLessThan(longest, 40_000, "never the whole Bible: the longest window was \(longest) UTF-16 units")
        XCTAssertTrue(stream.edits.contains { if case .delete = $0 { return true } else { return false } }, "far blocks are released")
    }

    // MARK: Linked tabs

    func testLinkedTabsFollowVerseForVerseWithoutFeedingBack() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        type("John 3:1", model)
        let bsb = try XCTUnwrap(model.stream(for: "t1")), greek = try XCTUnwrap(model.stream(for: "t3"))
        var reports = model.recordedScrolls
        _ = scroll(bsb, by: 40, limit: 120) { self.at($0, "John.3", atLeast: 12) }
        let top = try XCTUnwrap(bsb.topSegment)
        XCTAssertEqual(greek.topSegment, top, "the SBLGNT column is at the same verse")
        XCTAssertEqual(try XCTUnwrap(screenY(greek, top)), ReadingStream.topMargin, accuracy: 1, "at the top, as typed passages are")
        XCTAssertGreaterThan(model.recordedScrolls, reports)
        reports = model.recordedScrolls
        spin(0.3)
        XCTAssertEqual(model.recordedScrolls, reports, "the follower's scroll is not reported back")

        // A WLC tab in the group follows by its own numbering: English Malachi 4 is Hebrew 3:19–24.
        model.run("column.focus", args: ShellCommandArgs(column: 2))
        model.run("tab.close")
        model.run("column.focus", args: ShellCommandArgs(column: 1))
        model.run("tab.next")
        model.run("tab.move_to_column", args: ShellCommandArgs(column: 2))
        model.run("column.focus", args: ShellCommandArgs(column: 2))
        model.run("tab.link", args: ShellCommandArgs(group: "1"))
        model.run("column.focus", args: ShellCommandArgs(column: 1))
        spin()
        XCTAssertEqual(shownTab(model, column: 2)?.contentId, "wlc")
        type("Mal 4:5", model)
        let hebrew = try XCTUnwrap(model.stream(for: try XCTUnwrap(shownTab(model, column: 2)).id))
        XCTAssertEqual(shownTab(model, column: 2)?.reference, "Mal.3.23")
        XCTAssertEqual(hebrew.topSegment, "Mal.3.23")
        XCTAssertEqual(hebrew.container?.header.text, "Mal 3 · WLC")
        type("Joel 2:28", model)
        let bsb2 = try XCTUnwrap(model.stream(for: "t1"))
        _ = scroll(bsb2, by: 40, limit: 60) { self.at($0, "Joel.2", atLeast: 30) }
        let english = try XCTUnwrap(place(bsb2.topSegment))
        XCTAssertEqual(english.chapter, "Joel.2")
        XCTAssertEqual(hebrew.topSegment, "Joel.3.\(english.verse - 27)", "English Joel 2:28–32 is Hebrew 3:1–5")
    }

    // MARK: History

    func testBackAndForwardReturnToTypedPassages() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        type("John 3:16", model)
        let stream = try XCTUnwrap(model.stream(for: "t1"))
        _ = scroll(stream, by: 40, limit: 40) { self.at($0, "John.3", atLeast: 25) }
        let left = try XCTUnwrap(stream.topSegment)
        spin(0.3)
        type("Jude 3", model)
        XCTAssertEqual(stream.topSegment, "Jude.1.3")
        model.run("nav.back")
        spin(0.2)
        XCTAssertEqual(stream.topSegment, left, "Back returns to where the reader was")
        XCTAssertNil(shownTab(model, column: 1)?.highlightRange)
        XCTAssertTrue(stream.surface.overlay.isEmpty, "the mark is cleared")
        model.run("nav.forward")
        spin(0.2)
        XCTAssertEqual(stream.topSegment, "Jude.1.3")
    }

    // MARK: Text

    func testBlocksCarryHeadingsParagraphsPoetryAndTitles() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        type("Ps 3", model)
        let layout = try XCTUnwrap(model.stream(for: "t1")?.surface.layout)
        let psalm = try XCTUnwrap(layout.blocks.first { $0.chapterKey == "Ps.3" })
        XCTAssertEqual(layout.display.attributedSubstring(from: psalm.headingRange).string, "Psalm 3\n\(ReadingNotice.placeholder)\n")
        let title = try XCTUnwrap(layout.verseRanges["Ps.3.0"])
        XCTAssertFalse(layout.markers.contains { $0.segmentId == "Ps.3.0" }, "a Psalm title has no verse number")
        let titleFont = try XCTUnwrap(layout.display.attribute(.font, at: title.location, effectiveRange: nil) as? NSFont)
        XCTAssertTrue(titleFont.fontDescriptor.symbolicTraits.contains(.italic), "the title is styled")
        let line = try XCTUnwrap(layout.verseRanges["Ps.3.2"])
        let style = try XCTUnwrap(layout.display.attribute(.paragraphStyle, at: line.location, effectiveRange: nil) as? NSParagraphStyle)
        XCTAssertGreaterThan(style.headIndent, 0, "poetry lines are indented")
        XCTAssertEqual((layout.display.string as NSString).substring(with: NSRange(location: line.location - 1, length: 1)), "\n", "a line of its own")
        XCTAssertEqual(layout.placeholderSegments.contains("Ps.3.1"), true)

        type("Genesis 1:1", model)
        let genesis = try XCTUnwrap(model.stream(for: "t1")?.surface.layout)
        let first = try XCTUnwrap(genesis.blocks.first)
        XCTAssertEqual(genesis.display.attributedSubstring(from: first.headingRange).string, "Genesis\nChapter 1\n\(ReadingNotice.placeholder)\n", "the book's title on its first chapter")
        XCTAssertFalse(genesis.display.attributedSubstring(from: genesis.blocks[1].headingRange).string.hasPrefix("Genesis"), "only there")
    }

    func testRealFixtureVersesKeepTheirTextAndPlaceholdersCannotBeQuoted() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        type("John 3:16", model)
        let stream = try XCTUnwrap(model.stream(for: "t1"))
        let layout = try XCTUnwrap(stream.surface.layout)
        XCTAssertEqual(layout.segmentTexts["John.3.16"]?.hasPrefix("For God so loved the world"), true)
        XCTAssertFalse(layout.placeholderSegments.contains("John.3.16"))
        XCTAssertFalse(layout.placeholderSegments.contains("John.3.17"))
        XCTAssertTrue(layout.placeholderSegments.contains("John.3.18"))
        let greek = try XCTUnwrap(model.stream(for: "t3")?.surface.layout)
        XCTAssertEqual(greek.segmentTexts["John.3.16"]?.hasPrefix("Οὕτως γὰρ ἠγάπησεν"), true)

        // A real verse: the selection actions work.
        stream.surface.select(display: layout.runMap.displayRange(fromCore: try XCTUnwrap(layout.coreRange(ofPhrase: "loved the world"))))
        XCTAssertFalse(model.selectionIsPlaceholder)
        XCTAssertNil(model.well.groups.flatMap(\.items).first { $0.commandId == "selection.highlight" }?.disabledText)
        // A placeholder verse: quoting and collecting are disabled, with words.
        let placeholder = try XCTUnwrap(layout.coreRange(ofPhrase: "Placeholder verse — John 3:18"))
        stream.surface.select(display: layout.runMap.displayRange(fromCore: placeholder))
        XCTAssertTrue(model.selectionIsPlaceholder)
        for id in ["selection.highlight", "selection.collect", "selection.note"] {
            XCTAssertEqual(model.well.groups.flatMap(\.items).first { $0.commandId == id }?.disabledText, ShellModel.placeholderReason, id)
        }
        XCTAssertFalse(model.run("selection.highlight"))
        XCTAssertEqual(model.toast, ShellModel.placeholderReason)
        XCTAssertNil(model.marks["t1"], "nothing highlighted")
        let tab = try XCTUnwrap(shownTab(model, column: 1)), column = model.activeWorkspace.columns[0]
        let menu = try XCTUnwrap(model.nsMenu(for: .selection(try XCTUnwrap(model.selection)), tab: tab, column: column))
        let highlight = try XCTUnwrap(menu.items.first { $0.title == "Highlight" })
        XCTAssertFalse(highlight.isEnabled)
        XCTAssertEqual(highlight.toolTip, ShellModel.placeholderReason)
        XCTAssertTrue(menu.items.contains { $0.title.hasPrefix("Copy") && $0.isEnabled }, "copying the text itself stays")
        // A highlight drag over placeholder text is refused too.
        model.run("tool.highlight")
        spin(0.1)
        stream.surface.select(display: layout.runMap.displayRange(fromCore: placeholder))
        stream.surface.dragEnded()
        XCTAssertNil(model.marks["t1"])
    }

    func testHighlightsAndInkStayOnTheirVersesAsBlocksComeAndGo() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        type("John 3:16", model)
        let stream = try XCTUnwrap(model.stream(for: "t1"))
        let layout = try XCTUnwrap(stream.surface.layout)
        model.run("tool.highlight")
        spin(0.1)
        stream.surface.select(display: layout.runMap.displayRange(fromCore: try XCTUnwrap(layout.coreRange(ofPhrase: "loved the world"))))
        stream.surface.dragEnded()
        model.run("tool.select")
        XCTAssertEqual(model.marks["t1"]?.count, 1)
        // A pen stroke under John 3:16's first line.
        model.run("tool.pen")
        spin(0.2)
        let sheet = model.inkSheet(for: "t1")
        let lineY = try XCTUnwrap(screenY(stream, "John.3.16")) + ReadingHeaderView.height + 22
        let samples = (0..<12).map { InkSample(x: 120 + Double($0) * 10, y: Double(lineY), pressure: nil, time: Double($0) * 0.01, altitude: nil, azimuth: nil) }
        sheet.commit(try XCTUnwrap(sheet.makeStroke(samples: samples)))
        model.run("tool.select")
        spin(0.3)
        let stroke = try XCTUnwrap(sheet.liveStrokes.first?.id)
        XCTAssertEqual(sheet.anchors[stroke]?.segment, "John.3.16", "the stroke is pinned to its verse")
        XCTAssertEqual(try XCTUnwrap(sheet.displacement(of: stroke)), 0, accuracy: 0.5)
        stream.scrollAsReader(by: 120)
        XCTAssertEqual(try XCTUnwrap(sheet.displacement(of: stroke)), -120, accuracy: 1, "it scrolls with the text")
        // Down until John 3 is released, then back up until it is loaded again.
        _ = scroll(stream, by: 200, limit: 400) { !$0.blocks.contains { $0.chapterKey == "John.3" } }
        XCTAssertFalse(stream.blocks.contains { $0.chapterKey == "John.3" })
        XCTAssertNil(sheet.displacement(of: stroke), "out of the window, the stroke is not drawn")
        _ = scroll(stream, by: -200, limit: 400) { $0.topSegment?.hasPrefix("John.3.") == true }
        let drawnAt = try XCTUnwrap(sheet.displacement(of: stroke)) + lineY
        XCTAssertEqual(drawnAt, try XCTUnwrap(screenY(stream, "John.3.16")) + ReadingHeaderView.height + 22, accuracy: 1, "back on its verse")
        spin(0.2)
        let back = try XCTUnwrap(stream.surface.layout)
        let storage = try XCTUnwrap(stream.surface.storage)
        let phrase = back.runMap.displayRange(fromCore: try XCTUnwrap(back.coreRange(ofPhrase: "loved the world")))
        var effective = NSRange()
        let color = storage.attribute(.backgroundColor, at: phrase.location + 1, longestEffectiveRange: &effective, in: NSRange(location: 0, length: storage.length))
        XCTAssertNotNil(color, "the highlight is back on its words")
        XCTAssertEqual((storage.string as NSString).substring(with: effective), "loved the world")
    }
}
