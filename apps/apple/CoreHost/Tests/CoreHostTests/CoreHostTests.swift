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
        XCTAssertEqual(apiVersion(), "0.1.2")
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
