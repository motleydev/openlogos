// B005: the round trips through the real TextKit 2 surface, hosted in the app on the iPad
// simulator (OpenLogosTests) and natively on the Mac (OpenLogosMacTests). Each AnchorCase puts
// the platform text view's selection where a user would (or, for the cluster cases, where a
// user cannot), lets the surface run selectionToAnchors and anchorToUtf16, and asserts the
// anchors, the painted ranges and the verdict. RunMap is tested on its own first.
import XCTest
@testable import OpenLogos

#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

@MainActor
final class ReaderRoundTripTests: XCTestCase {
    // MARK: Run map

    func testRunMapCollapsesVerseNumbersBothWays() {
        // Display: "16 " (3 units, non-core) + "abc" + " " + "17 " (non-core) + "def"
        let map = RunMap(nonCore: [NSRange(location: 0, length: 3), NSRange(location: 7, length: 3)])
        XCTAssertEqual(map.coreRange(fromDisplay: NSRange(location: 3, length: 3)), NSRange(location: 0, length: 3))
        XCTAssertEqual(map.coreRange(fromDisplay: NSRange(location: 1, length: 2)), NSRange(location: 0, length: 0), "a verse number alone is empty")
        XCTAssertEqual(map.coreRange(fromDisplay: NSRange(location: 4, length: 7)), NSRange(location: 1, length: 4), "bc + joiner + d; the number collapses")
        XCTAssertEqual(map.displayRange(fromCore: NSRange(location: 0, length: 3)), NSRange(location: 3, length: 3))
        XCTAssertEqual(map.displayRange(fromCore: NSRange(location: 4, length: 3)), NSRange(location: 10, length: 3), "the start skips the second number")
        XCTAssertEqual(map.displayRange(fromCore: NSRange(location: 0, length: 4)), NSRange(location: 3, length: 4), "the end stops before the second number")
        XCTAssertEqual(map.coreText(ofDisplay: "16 abc 17 def"), "abc def")
    }

    func testLayoutBuildsTheCoreParagraphTheConformanceFixtureDescribes() {
        let layout = ReaderLayout(content: ReaderContent(content: PrototypeFixtures.bsbJohn3))
        XCTAssertEqual(layout.paragraph.editionId, "bsb")
        XCTAssertEqual(layout.paragraph.contentRevision, "bsb-2024")
        XCTAssertEqual(layout.paragraph.joiner, " ")
        XCTAssertEqual(layout.paragraph.segments.map(\.segmentId), ["John.3.16", "John.3.17"])
        XCTAssertEqual(layout.paragraph.segments[0].tokens.count, 27, "the fixture's token layer, not minted")
        XCTAssertEqual(layout.runMap.coreText(ofDisplay: layout.display.string), layout.coreText, "the display minus its non-core runs is what the core was given")
        XCTAssertTrue(layout.display.string.hasPrefix("16\u{2009}For God"))
        XCTAssertEqual(layout.segmentOffsets["John.3.17"], 136)
    }

    func testMintedTokensFollowTheCoreIdFormat() {
        let tokens = ReaderLayout.mintedTokens(edition: "n1904", segmentId: "John.3.17", text: "οὐ γὰρ ἀπέστειλεν")
        XCTAssertEqual(tokens.map(\.id), ["n1904:44003017001", "n1904:44003017002", "n1904:44003017003"])
        XCTAssertEqual(tokens.map { Int($0.start) }, [0, 3, 7])
        XCTAssertEqual(tokens.last?.end, 17)
        XCTAssertTrue(ReaderLayout.mintedTokens(edition: "x", segmentId: "note-1", text: "a b").isEmpty)
        // Every minted id parses in the core (TokenId::parse is the authority).
        let hebrew = ReaderLayout.mintedTokens(edition: "wlc", segmentId: "Gen.1.2", text: "עַל־פְּנֵ֣י תְה֑וֹם")
        XCTAssertEqual(hebrew.map(\.id), ["wlc:01001002001", "wlc:01001002002"])
    }

    // MARK: The surface

    private func surface(for content: PaneContent, paneID: String) -> (ReaderSurface, PlatformTextView) {
        let surface = ReaderSurface(paneID: paneID, onSelection: { _ in }, onAction: { _, _ in }, onMeasurement: { _ in })
        #if canImport(UIKit)
        let view = surface.makeTextView()
        view.frame = CGRect(x: 0, y: 0, width: 600, height: 800)
        #else
        let scroll = surface.makeScrollView()
        scroll.frame = CGRect(x: 0, y: 0, width: 600, height: 800)
        let view = surface.textView!
        view.frame = CGRect(x: 0, y: 0, width: 600, height: 800)
        #endif
        surface.apply(content: ReaderContent(content: content), padding: 20)
        return (surface, view)
    }

    private func content(for paneID: String) -> PaneContent {
        switch paneID {
        case "p1": return PrototypeFixtures.bsbJohn3
        case "p2": return PrototypeFixtures.wlcGenesis1
        case "p3": return PrototypeFixtures.greekJohn3
        default: return PrototypeFixtures.studyNote
        }
    }

    func testTheSurfaceUsesTextKit2() {
        let (surface, _) = surface(for: PrototypeFixtures.bsbJohn3, paneID: "p1")
        XCTAssertTrue(surface.usesTextKit2, "the text view must have an NSTextLayoutManager")
    }

    func testEveryAnchorCaseRoundTripsThroughTheTextView() throws {
        var surfaces: [String: (ReaderSurface, PlatformTextView)] = [:]
        var lines: [String] = []
        for anchorCase in AnchorCase.all {
            let pair = surfaces[anchorCase.paneID] ?? surface(for: content(for: anchorCase.paneID), paneID: anchorCase.paneID)
            surfaces[anchorCase.paneID] = pair
            let (surface, _) = pair
            guard let layout = surface.layout, let range = anchorCase.displayRange(in: layout) else {
                XCTFail("\(anchorCase.name): no display range"); continue
            }
            surface.select(display: range)
            let kept = surface.selectedRange
            let selection = surface.current
            if let problem = anchorCase.mismatch(selection, asked: range) {
                XCTFail(problem + " (text view kept \(kept), error \(surface.lastError ?? "none"))")
            }
            lines.append("\(anchorCase.name): asked \(range), text view kept \(kept), \(selection?.summary ?? "no selection")")
            // The painted text is the anchors' text, character for character.
            if let selection {
                let display = layout.display.string as NSString
                XCTAssertEqual(selection.highlights.map { display.substring(with: $0) }, anchorCase.expected.map(\.exact), anchorCase.name)
            }
        }
        let report = lines.joined(separator: "\n")
        print("B005 round trips:\n\(report)")
        let attachment = XCTAttachment(string: report)
        attachment.name = "b005-roundtrips.txt"
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    func testASelectionOverAVerseNumberAloneYieldsNoAnchors() {
        let (surface, _) = surface(for: PrototypeFixtures.bsbJohn3, paneID: "p1")
        guard let layout = surface.layout, let marker = layout.markers.last else { return XCTFail("no markers") }
        surface.select(display: marker.range)
        XCTAssertNil(surface.current)
        XCTAssertNil(surface.lastError)
    }

    func testSelectionEndingInsideAVerseNumberStopsAtItsEdge() {
        let (surface, _) = surface(for: PrototypeFixtures.bsbJohn3, paneID: "p1")
        guard let layout = surface.layout, let marker = layout.markers.last else { return XCTFail("no markers") }
        // From "life." through the joiner and one unit into "17 ".
        let start = layout.runMap.displayRange(fromCore: NSRange(location: 130, length: 5)).location
        surface.select(display: NSRange(location: start, length: marker.range.location + 1 - start))
        guard let selection = surface.current else { return XCTFail("no selection: \(surface.lastError ?? "")") }
        XCTAssertEqual(selection.anchors.map(\.segmentId), ["John.3.16"])
        XCTAssertEqual(selection.anchors[0].quote?.exact, "life.")
        XCTAssertTrue(selection.identical)
    }

    func testTheSyntheticSetLaysOutAndSelectsAtTheEnd() {
        let (surface, _) = surface(for: PrototypeFixtures.synthetic(verses: 1000), paneID: "p1")
        guard let layout = surface.layout else { return XCTFail("no layout") }
        XCTAssertEqual(layout.paragraph.segments.count, 1000)
        XCTAssertEqual(layout.paragraph.segments.last?.segmentId, "John.10.100")
        let core = layout.coreText as NSString
        surface.select(display: layout.runMap.displayRange(fromCore: NSRange(location: core.length - 13, length: 13)))
        guard let selection = surface.current else { return XCTFail("no selection: \(surface.lastError ?? "")") }
        XCTAssertEqual(selection.anchors.map(\.segmentId), ["John.10.100"])
        XCTAssertTrue(selection.identical)
        XCTAssertLessThan(selection.elapsedMilliseconds, 50, "two core calls over 1,000 segments")
        print("B005 synthetic selection: \(selection.summary)")
    }
}
