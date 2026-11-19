// The four checks B061 asks of every host, against the generated Swift bindings: the API
// version, a citation parse, a UTF-16 selection round trip through an Anchor on the Hebrew
// cantillation fixture, and the conformance suite. Fixture texts and expected offsets come
// from the core itself through conformanceFixturesJson(), so Swift asserts the same numbers
// the Rust and Kotlin tests do.
import Foundation
import OpenLogosCore
import XCTest

final class CoreHostTests: XCTestCase {
    func testApiVersionMatchesTheCore() {
        XCTAssertEqual(apiVersion(), "0.1.6")
        XCTAssertEqual(runConformance().apiVersion, apiVersion())
    }

    func testParsesACitation() throws {
        let set = try parseReference(text: "Jn 3:16, 18; Rom 8:28-30")
        XCTAssertEqual(set.apiVersion, apiVersion())
        XCTAssertEqual(set.osis, "John.3.16;John.3.18;Rom.8.28-Rom.8.30")
        XCTAssertEqual(set.human, "John 3:16, 18; Rom 8:28–30")
        XCTAssertEqual(set.ranges.count, 3)
        XCTAssertEqual(
            set.ranges[0].start,
            Reference(book: "John", chapter: 3, verse: 16, subverse: nil)
        )
        XCTAssertEqual(set.ranges[1].osis, "John.3.18")
        XCTAssertEqual(set.ranges[2].start.verse, 28)
        XCTAssertEqual(set.ranges[2].end.verse, 30)
        XCTAssertEqual(set.ranges[2].human, "Rom 8:28–30")
        XCTAssertTrue(set.flags.isEmpty)

        XCTAssertEqual(try formatOsis(reference: set.ranges[0].start), "John.3.16")
        XCTAssertEqual(try formatHuman(reference: set.ranges[0].start), "John 3:16")

        let flagged = try parseReference(text: "Ps 23")
        XCTAssertEqual(flagged.flags, [.chapterOnly(range: 0)])

        XCTAssertThrowsError(try parseReference(text: "Nope 3:16")) { error in
            guard case ReferenceError.UnknownBook(let name) = error else {
                return XCTFail("unexpected error \(error)")
            }
            XCTAssertEqual(name, "Nope")
        }
    }

    func testMapsAVerseThroughTheFixtureTable() throws {
        let psalm = try parseReference(text: "Ps 9:1").ranges[0].start
        let result = try mapVerse(reference: psalm, fromScheme: "eng", toScheme: "lxx", rows: [])
        XCTAssertEqual(result.status, .exact)
        XCTAssertEqual(result.targets, [Reference(book: "Ps", chapter: 9, verse: 2, subverse: nil)])
        XCTAssertEqual(result.path, ["eng", "org", "lxx"])

        let genesis = try parseReference(text: "Gen 1:1").ranges[0].start
        let unmapped = try mapVerse(reference: genesis, fromScheme: "eng", toScheme: "org", rows: [])
        XCTAssertEqual(unmapped.status, .unmapped)
        XCTAssertTrue(unmapped.targets.isEmpty)
    }

    /// Every selection of every text fixture: the UTF-16 span a host would take from
    /// `String.utf16` goes in, the anchor carries the fixture's scalar range, exact text and
    /// token ids, and the span comes back unchanged.
    func testUtf16SelectionRoundTripsThroughAnAnchor() throws {
        var checked = 0
        for fixture in try textFixtures() {
            let segments = fixture.segments
            for selection in fixture.selections {
                guard let segment = segments[selection.segmentId] else {
                    return XCTFail("\(fixture.id): no segment \(selection.segmentId)")
                }
                let paragraph = Paragraph(
                    editionId: fixture.editionId,
                    contentRevision: fixture.contentRevision,
                    joiner: "",
                    segments: [segment]
                )
                let anchors = try selectionToAnchors(paragraph: paragraph, selection: selection.utf16)
                XCTAssertEqual(anchors.count, 1, "\(fixture.id): \(selection.name)")
                guard let anchor = anchors.first else { continue }
                XCTAssertEqual(anchor.apiVersion, apiVersion())
                XCTAssertEqual(anchor.segmentId, selection.segmentId)
                XCTAssertEqual(anchor.range, selection.scalar, "\(fixture.id): \(selection.name)")
                XCTAssertEqual(anchor.quote?.exact, selection.exact, "\(fixture.id): \(selection.name)")
                XCTAssertEqual(anchor.tokens, selection.tokens, "\(fixture.id): \(selection.name)")

                let back = try anchorToUtf16(anchor: anchor, text: segment.text)
                XCTAssertEqual(back, selection.utf16, "\(fixture.id): \(selection.name)")

                // What a TextKit host does with the span: NSString counts UTF-16 units.
                let nsText = segment.text as NSString
                let nsRange = NSRange(location: Int(back.start), length: Int(back.end - back.start))
                XCTAssertEqual(nsText.substring(with: nsRange), selection.exact)
                checked += 1
            }
        }
        XCTAssertGreaterThanOrEqual(checked, 10, "the fixtures carry at least ten selections")
    }

    func testHebrewSelectionIsTheThirdWord() throws {
        guard let fixture = try textFixtures().first(where: { $0.id == "hebrew-cantillation-genesis-1" }),
              let verse = fixture.segments["Gen.1.1"]
        else {
            return XCTFail("missing hebrew-cantillation-genesis-1 / Gen.1.1")
        }
        let paragraph = Paragraph(
            editionId: fixture.editionId,
            contentRevision: fixture.contentRevision,
            joiner: "",
            segments: [verse]
        )
        // אֱלֹהִ֑ים: UTF-16 units 21..<30 of the verse (no surrogates in Hebrew).
        let anchors = try selectionToAnchors(paragraph: paragraph, selection: Utf16Range(start: 21, end: 30))
        XCTAssertEqual(anchors.count, 1)
        XCTAssertEqual(anchors[0].tokens, ["wlc:01001001003"])
        XCTAssertEqual(anchors[0].quote?.exact, "אֱלֹהִ֑ים")
        XCTAssertEqual(try anchorToUtf16(anchor: anchors[0], text: verse.text), Utf16Range(start: 21, end: 30))

        // A second revision with a word inserted before the verse: the anchor follows its token id.
        let old = Revision(contentRevision: fixture.contentRevision, text: verse.text, tokens: verse.tokens)
        let shifted = verse.tokens.map {
            TokenSpan(id: $0.id, range: ScalarRange(start: $0.range.start + 4, end: $0.range.end + 4))
        }
        let new = Revision(contentRevision: "r2", text: "xxx " + verse.text, tokens: shifted)
        guard case .moved(let moved) = try reanchor(anchor: anchors[0], old: old, new: new) else {
            return XCTFail("expected a moved anchor")
        }
        XCTAssertEqual(moved.range, ScalarRange(start: 25, end: 34))
        XCTAssertEqual(moved.contentRevision, "r2")
    }

    func testConformanceSuitePasses() {
        let report = runConformance()
        let failures = report.cases.filter { !$0.passed }
            .map { "\($0.fixture) / \($0.name): expected \($0.expected), actual \($0.actual)" }
        XCTAssertEqual(report.failed, 0, failures.joined(separator: "\n"))
        XCTAssertGreaterThan(report.passed, 0)
        XCTAssertEqual(Int(report.passed), report.cases.count)
        print("conformance cases: \(report.passed) passed, \(report.failed) failed")
    }

    // B070: the workspace shell through the UniFFI handle.
    func testShellCommandsUndoAndPalette() throws {
        let shell = ShellHandle.fixture()
        XCTAssertEqual(shell.session().workspaces.count, 2)
        let outcome = try shell.apply(commandId: "column.new", args: ShellCommandArgs(), hasSelection: false)
        XCTAssertTrue(outcome.handled && outcome.recorded)
        XCTAssertEqual(shell.session().workspaces[0].columns.map(\.number), [1, 2, 3, 4])
        XCTAssertEqual(shell.session().undoTitle, "New column")
        _ = try shell.undo()
        XCTAssertEqual(shell.session().workspaces[0].columns.count, 3)
        XCTAssertThrowsError(try shell.apply(commandId: "column.focus", args: ShellCommandArgs(column: 9), hasSelection: false)) { error in
            guard case ShellCommandError.Disabled(_, let kind, _) = error else { return XCTFail("\(error)") }
            XCTAssertEqual(kind, "no_such_column")
        }
        let context = ShellPaletteContext(hasSelection: false, platform: .mac)
        let result = shell.paletteQuery(text: "Jn 3:16 @2", context: context)
        XCTAssertEqual(result.targetColumn, 2)
        let first = try XCTUnwrap(result.groups.first?.items.first)
        XCTAssertEqual(first.section, .passage)
        let navigated = try shell.apply(commandId: first.commandId, args: first.args, hasSelection: false)
        XCTAssertEqual(navigated.followedTabIds, ["t1", "t4"])
        XCTAssertEqual(shell.paletteQuery(text: "#loved", context: context).groups.first?.section, .text)
        XCTAssertEqual(shellRegistry().first { $0.id == "tab.new" }?.shortcuts.mac, ["Alt+T", "Cmd+T"])
        let records = shell.records()
        let reopened = try ShellHandle.fromRecords(sessionJson: records.sessionJson, workspaceJsons: records.workspaceJsons)
        XCTAssertEqual(reopened.session().workspaces, shell.session().workspaces)
        // Settings live in the session; a scroll position does not bump the revision.
        _ = try shell.apply(commandId: "view.line_spacing", args: ShellCommandArgs(number: 1.7), hasSelection: false)
        XCTAssertEqual(shell.session().reading.lineSpacing, 1.7)
        let revision = shell.session().revision
        _ = try shell.apply(commandId: "tab.record_scroll", args: ShellCommandArgs(tabId: "t3", segmentId: "John.3.16", scalar: 4), hasSelection: false)
        XCTAssertEqual(shell.session().revision, revision)
        XCTAssertEqual(layoutConstants().measureCapStandardPt, 640)
        // Short labels and registered resource names.
        XCTAssertEqual(try formatShort(reference: "John 3:16"), "Jn 3:16")
        shell.setResourceTitles(titles: ["bsb": "BSB"])
        XCTAssertEqual(shell.session().workspaces[0].columns[0].tabs[0].resourceTitle, "BSB")
        XCTAssertEqual(shell.session().workspaces[0].columns[0].tabs[0].chapterLabel, "Jn 3")
        XCTAssertEqual(shell.paletteQuery(text: "@bsb jn", context: context).groups.first?.items.first?.title, "BSB · Jn 3:16")
        XCTAssertThrowsError(try shell.apply(commandId: "tab.focus", args: ShellCommandArgs(), hasSelection: false)) { error in
            guard case ShellCommandError.Disabled(_, _, let reason) = error else { return XCTFail("\(error)") }
            XCTAssertEqual(reason, "Choose a tab")
        }
    }

    // B076: the contextual tool well through the UniFFI handle.
    func testToolWellFollowsFocus() throws {
        let shell = ShellHandle.fixture()
        let tab = ShellCommandContext(hasSelection: false, focus: .tab)
        let well = shell.toolWell(context: tab)
        XCTAssertEqual(well.context, "translation")
        XCTAssertEqual(well.groups.map(\.kind), ["tools", "ink"])
        XCTAssertEqual(well.groups[0].items.map(\.icon), ["select", "highlight", "pen", "highlighter-pen", "eraser", "collect"])
        _ = try shell.applyIn(commandId: "tool.pen", args: ShellCommandArgs(), context: tab)
        let dock = ShellCommandContext(hasSelection: true, focus: .notesDock)
        let notes = shell.toolWell(context: dock)
        XCTAssertEqual([notes.context, notes.tool, notes.effectiveTool], ["notes", "pen", "type"])
        XCTAssertEqual(notes.groups.last?.kind, "selection")
        XCTAssertThrowsError(try shell.applyIn(commandId: "tool.eraser", args: ShellCommandArgs(), context: dock)) { error in
            guard case ShellCommandError.Disabled(_, let kind, let reason) = error else { return XCTFail("\(error)") }
            XCTAssertEqual(kind, "wrong_context")
            XCTAssertEqual(reason, "Eraser works on Bible text")
        }
        _ = try shell.apply(commandId: "column.focus", args: ShellCommandArgs(column: 2), hasSelection: false)
        XCTAssertEqual(shell.session().toolContext, "original")
        XCTAssertEqual(shell.session().effectiveTool, "pen")
        let rows = shell.paletteQuery(text: ":Jn 3:16", context: ShellPaletteContext(hasSelection: false, platform: .mac, focus: .notesDock))
        XCTAssertEqual(rows.groups.first?.items.map(\.commandId), ["nav.navigate", "note.link_passage"])
        let opened = try shell.apply(commandId: "tab.open_lexicon", args: ShellCommandArgs(text: "ἀγάπη"), hasSelection: false)
        XCTAssertTrue(opened.recorded)
        XCTAssertEqual(shell.session().workspaces[0].columns[2].tabs[0].lexiconWord, "ἀγάπη")
        XCTAssertEqual(shell.session().workspaces[0].activeColumnNumber, 2, "focus stays where the word was clicked")
    }

    // B077: the context menu through the UniFFI handle.
    func testContextMenuComesFromTheCore() throws {
        let shell = ShellHandle.fixture()
        let none = ShellCommandContext(hasSelection: false, focus: .tab)
        let menu = try shell.contextMenu(
            target: .word(text: "ἠγάπησεν", reference: "John.3.16", segmentId: nil, scalarStart: nil, scalarEnd: nil, resourceId: nil, tabId: "t3"),
            context: none)
        XCTAssertEqual(menu.targetKind, "word")
        XCTAssertEqual(menu.sections.map(\.label), ["Search", "Open", "Mark", "Copy"])
        let rows = menu.sections.flatMap(\.items)
        XCTAssertNil(rows.first { $0.label == "Word study" }?.disabledText)
        let compare = try XCTUnwrap(rows.first { $0.label == "Compare in bsb" })
        XCTAssertEqual(compare.args.resourceId, "bsb")
        let focus = try XCTUnwrap(menu.focus)
        XCTAssertEqual(focus.args.tabId, "t3")
        _ = try shell.applyIn(commandId: focus.commandId, args: focus.args, context: menu.context)
        XCTAssertTrue(try shell.applyIn(commandId: compare.commandId, args: compare.args, context: menu.context).recorded)
        XCTAssertEqual(shell.session().workspaces[0].activeColumnNumber, 2)
        let tab = try shell.contextMenu(target: .tab(tabId: "t2"), context: none)
        let pin = tab.sections[2].items[0]
        XCTAssertEqual(pin.label, "Pin")
        XCTAssertTrue(pin.active)
        XCTAssertEqual(pin.disabledText, "The tab is already pinned")
        XCTAssertThrowsError(try shell.contextMenu(target: .column(column: 9), context: none)) { error in
            guard case ShellCommandError.InvalidDocument(let detail) = error else { return XCTFail("\(error)") }
            XCTAssertTrue(detail.contains("There is no column 9"))
        }
    }

    // B078: continuous reading through the UniFFI handles, and the shell following a scroll.
    func testContinuousReadingStreamsChaptersAndTheShellFollows() throws {
        let reading = ReadingHandle.placeholder()
        XCTAssertEqual(reading.editions().map(\.resourceId), ["bsb", "sblgnt", "wlc"])
        let location = try reading.locate(resourceId: "bsb", input: "Jn 3:16-18")
        XCTAssertEqual([location.chapterKey, location.segmentId], ["John.3", "John.3.16"])
        XCTAssertEqual(location.highlightRange, "John.3.16-John.3.18")
        XCTAssertEqual(try reading.chapterNeighbors(resourceId: "bsb", chapterKey: "Mal.4").next, "Matt.1")
        let psalm = try reading.chapterContent(resourceId: "bsb", chapterKey: "Ps.3")
        XCTAssertEqual(psalm.heading.chapterLabel, "Psalm 3")
        XCTAssertTrue(psalm.verses[0].superscription)
        let genesis = try reading.chapterContent(resourceId: "wlc", chapterKey: "Gen.1")
        XCTAssertFalse(genesis.verses[0].placeholder)
        XCTAssertEqual(genesis.verses[2].text, "טקסט ממלא מקום — 1:3")
        XCTAssertThrowsError(try reading.locate(resourceId: "wlc", input: "John 3:16")) { error in
            guard case ReadingCallError.Refused(let kind, let reason) = error else { return XCTFail("\(error)") }
            XCTAssertEqual(kind, "book_not_in_edition")
            XCTAssertEqual(reason, "John is not in WLC")
        }
        XCTAssertEqual(referenceForSegment(segmentId: "Num.26.3-4"), "Num.26.3")
        let joel = try parseReference(text: "Joel 2:28").ranges[0].start
        XCTAssertEqual(try canonMapVerse(reference: joel, fromScheme: "eng", toScheme: "org").targets,
                       [Reference(book: "Joel", chapter: 3, verse: 1, subverse: nil)])

        let shell = ShellHandle.fixture()
        shell.setResourceSchemes(schemes: reading.schemes())
        let scrolled = try shell.apply(commandId: "tab.record_scroll",
                                       args: ShellCommandArgs(tabId: "t1", segmentId: "John.3.17", scalar: 0), hasSelection: false)
        XCTAssertEqual(scrolled.positions.map(\.tabId), ["t1", "t3", "t4"])
        XCTAssertEqual(scrolled.positions[0].referenceLabel, "Jn 3:17")
        XCTAssertFalse(scrolled.recorded)
        XCTAssertEqual(shell.session().workspaces[0].columns[0].tabs[0].referenceLabel, "Jn 3:17")
        _ = try shell.apply(commandId: "nav.navigate", args: ShellCommandArgs(reference: "Joel.2.28-Joel.2.29"), hasSelection: false)
        XCTAssertEqual(shell.session().workspaces[0].columns[0].tabs[0].highlightRange, "Joel.2.28-Joel.2.29")
        XCTAssertEqual(shell.session().workspaces[0].columns[1].tabs[0].highlightRange, "Joel.3.1-Joel.3.2")
    }

    // B074: the user store through the UniFFI handle: a worked session with a note, a
    // highlight (one refused on placeholder text) and a stroke, saved and restored.
    func testUserStoreSavesAndRestoresTheSessionAndAnnotations() throws {
        let reading = ReadingHandle.placeholder()
        let shell = ShellHandle.fixture()
        XCTAssertEqual(storeConstants().saveDebounceMs, 1000)
        _ = try shell.apply(commandId: "tab.move", args: ShellCommandArgs(column: 2, tabId: "t2", index: 0), hasSelection: false)
        _ = try shell.apply(commandId: "tab.record_scroll",
                            args: ShellCommandArgs(tabId: "t3", segmentId: "John.3.17", scalar: 4), hasSelection: false)
        try shell.putNote(note: StoreNote(id: "fixture:note:1", blocks: [
            StoreNoteBlock(kind: .heading, spans: [
                StoreNoteSpan(text: "See "),
                StoreNoteSpan(text: "Jn 3:16", link: "John.3.16"),
            ]),
        ]))
        func highlight(_ segment: String) -> StoreHighlight {
            StoreHighlight(resourceId: "bsb", contentRevision: "bsb-2024",
                           spans: [StoreSegmentSpan(segmentId: segment, start: 0, end: 4)], color: "#F5D90A")
        }
        XCTAssertThrowsError(try shell.addHighlight(highlight: highlight("John.3.2"), reading: reading)) { error in
            guard case StoreCallError.Refused(let kind, _) = error else { return XCTFail("\(error)") }
            XCTAssertEqual(kind, "placeholder_text")
        }
        XCTAssertEqual(try shell.addHighlight(highlight: highlight("John.3.16"), reading: reading), "h1")
        let stroke = InkStroke(id: "", tool: .pen, brushVersion: "pf-1.2.3/pen-1", width: 4, color: "#000000", opacity: 1,
                               points: [InkPoint(x: 0, y: -2, pressure: nil, elapsedMs: 0, altitude: nil, azimuth: nil),
                                        InkPoint(x: 8, y: 3, pressure: 0.4, elapsedMs: 10, altitude: nil, azimuth: nil)])
        XCTAssertEqual(try shell.addInk(mark: StoreInkMark(resourceId: "bsb", segmentId: "John.3.16", stroke: stroke)), "k2")
        XCTAssertEqual(shell.changeCount(), 5)
        let saved = shell.exportStore()

        let restored = ShellHandle.fixture()
        let report = try restored.importStore(json: saved)
        XCTAssertTrue(report.sessionRestored)
        XCTAssertEqual([report.notes, report.highlights, report.ink], [1, 1, 1])
        XCTAssertTrue(report.issues.isEmpty)
        XCTAssertEqual(restored.exportStore(), saved)
        let column = restored.session().workspaces[0].columns[1]
        XCTAssertEqual(column.tabs.map(\.id), ["t2", "t3"])
        XCTAssertEqual(column.tabs[1].scrollSegmentId, "John.3.17")
        XCTAssertEqual(restored.note(id: "fixture:note:1")?.blocks[0].spans[1].link, "John.3.16")
        XCTAssertEqual(restored.highlightsIn(resourceId: "bsb", chapterKey: "John.3").map(\.id), ["h1"])
        XCTAssertEqual(restored.inkIn(resourceId: "bsb", chapterKey: "John.3")[0].stroke.points.count, 2)
        XCTAssertThrowsError(try restored.importStore(json: "{")) { error in
            guard case StoreCallError.Refused(let kind, _) = error else { return XCTFail("\(error)") }
            XCTAssertEqual(kind, "not_json")
        }
    }

    // MARK: - Fixture decoding

    struct Span: Decodable { let start: UInt32; let end: UInt32 }
    struct FixtureToken: Decodable { let id: String; let start: UInt32; let end: UInt32 }
    struct FixtureSegment: Decodable { let segmentId: String; let text: String; let tokens: [FixtureToken] }
    struct FixtureSelection: Decodable {
        let name: String
        let segmentId: String
        let scalar: Span
        let utf16: Span
        let exact: String
        let tokens: [String]
    }
    struct TextFixture: Decodable {
        let id: String
        let kind: String
        let editionId: String?
        let contentRevision: String?
        let segments: [FixtureSegment]?
        let selections: [FixtureSelection]?
    }

    struct LoadedFixture {
        let id: String
        let editionId: String
        let contentRevision: String
        let segments: [String: Segment]
        let selections: [(name: String, segmentId: String, scalar: ScalarRange, utf16: Utf16Range, exact: String, tokens: [String])]
    }

    func textFixtures() throws -> [LoadedFixture] {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let data = Data(conformanceFixturesJson().utf8)
        let all = try decoder.decode([TextFixture].self, from: data)
        return all.filter { $0.kind == "text" }.map { fixture in
            var segments: [String: Segment] = [:]
            for segment in fixture.segments ?? [] {
                segments[segment.segmentId] = Segment(
                    segmentId: segment.segmentId,
                    text: segment.text,
                    tokens: segment.tokens.map {
                        TokenSpan(id: $0.id, range: ScalarRange(start: $0.start, end: $0.end))
                    }
                )
            }
            return LoadedFixture(
                id: fixture.id,
                editionId: fixture.editionId ?? "",
                contentRevision: fixture.contentRevision ?? "",
                segments: segments,
                selections: (fixture.selections ?? []).map {
                    (
                        name: $0.name,
                        segmentId: $0.segmentId,
                        scalar: ScalarRange(start: $0.scalar.start, end: $0.scalar.end),
                        utf16: Utf16Range(start: $0.utf16.start, end: $0.utf16.end),
                        exact: $0.exact,
                        tokens: $0.tokens
                    )
                }
            )
        }
    }
}
