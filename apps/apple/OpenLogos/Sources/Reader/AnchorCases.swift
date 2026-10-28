// The selections B005 drives through the surface (docs/evidence/B005.md), one table for the
// launch argument `-OPENLOGOS_CASE <name>`, the unit tests and the screenshots. Each case names
// a pane of the "B005 anchors" scenario (p1 BSB John 3:16–17, p2 WLC Genesis 1:1–2, p3 N1904
// John 3:16–17, note: the bidi study note in the notes dock) and a selection in core UTF-16
// units or as a phrase, with the anchors the core must return. Expected values are the
// conformance fixtures' own numbers where a fixture has them (hebrew-cantillation-genesis-1,
// cross-verse-selection-john-3, bidi-isolation-note) and are derived from the text otherwise.
import Foundation

struct AnchorCase: Identifiable {
    enum Target {
        /// A UTF-16 range in the pane's joined core text.
        case core(Int, Int)
        /// The first occurrence of a phrase in the core text.
        case phrase(String)
        /// A UTF-16 range in the display string (verse numbers included), to put the text view's
        /// selection where a user cannot: inside a grapheme cluster.
        case display(Int, Int)
    }

    struct Expected {
        let segmentId: String
        let exact: String
        /// Token ids, or nil when the case does not pin them.
        let tokens: [String]?
    }

    let name: String
    let paneID: String
    let target: Target
    let expected: [Expected]
    /// The surface had to widen the range to grapheme clusters.
    var snaps = false
    var note = ""

    var id: String { name }

    static let all: [AnchorCase] = [
        AnchorCase(
            name: "cross-verse",
            paneID: "p1",
            target: .core(113, 164),
            expected: [
                Expected(segmentId: "John.3.16", exact: "but have eternal life.", tokens: ["bsb:44003016024", "bsb:44003016025", "bsb:44003016026", "bsb:44003016027"]),
                Expected(segmentId: "John.3.17", exact: "For God did not send His Son", tokens: ["bsb:44003017001", "bsb:44003017002", "bsb:44003017003", "bsb:44003017004", "bsb:44003017005", "bsb:44003017006", "bsb:44003017007"]),
            ],
            note: "fixture cross-verse-selection-john-3, paragraph_selections[0]"
        ),
        AnchorCase(
            name: "bsb-eternal-life",
            paneID: "p1",
            target: .core(122, 135),
            expected: [Expected(segmentId: "John.3.16", exact: "eternal life.", tokens: ["bsb:44003016026", "bsb:44003016027"])],
            note: "fixture selection 'eternal life.' at the end of 16"
        ),
        AnchorCase(
            name: "hebrew-elohim",
            paneID: "p2",
            target: .core(21, 30),
            expected: [Expected(segmentId: "Gen.1.1", exact: "אֱלֹהִ֑ים", tokens: ["wlc:01001001003"])],
            note: "fixture selection: third word of 1:1 with etnachta"
        ),
        AnchorCase(
            name: "hebrew-cross-verse",
            paneID: "p2",
            // Token 7 of 1:1 starts at scalar 56 (the fixture's token table); 1:2 starts at 66.
            target: .core(56, 70),
            expected: [
                Expected(segmentId: "Gen.1.1", exact: "הָאָֽרֶץ׃", tokens: ["wlc:01001001007"]),
                Expected(segmentId: "Gen.1.2", exact: "וְהָ", tokens: ["wlc:01001002001"]),
            ],
            note: "last word of 1:1 through the first two letters of 1:2, across the joiner"
        ),
        AnchorCase(
            name: "hebrew-inside-cluster",
            paneID: "p2",
            // Display offsets: the marker "1" plus a thin space is 2 units, so bet is at 2 and its
            // sheva and dagesh at 3 and 4; 3..<5 starts between bet and its marks.
            target: .display(3, 5),
            expected: [Expected(segmentId: "Gen.1.1", exact: "בְּ", tokens: ["wlc:01001001001"])],
            snaps: true,
            note: "fixture selection 'between bet and its dagesh', snapped {0,3}: the surface widens to the cluster"
        ),
        AnchorCase(
            name: "hebrew-maqqef",
            paneID: "p2",
            // Gen.1.2 starts at core 66 (65 units of 1:1 and the joiner); the maqqef is scalar 51.
            target: .core(117, 118),
            expected: [Expected(segmentId: "Gen.1.2", exact: "־", tokens: [])],
            note: "fixture selection: maqqef only, not a token"
        ),
        AnchorCase(
            name: "greek-monogene",
            paneID: "p3",
            target: .phrase("μονογενῆ"),
            expected: [Expected(segmentId: "John.3.16", exact: "μονογενῆ", tokens: ["n1904:44003016012"])],
            note: "N1904 word with perispomeni; minted whitespace token"
        ),
        AnchorCase(
            name: "greek-cross-verse",
            paneID: "p3",
            target: .phrase("αἰώνιον. οὐ γὰρ"),
            expected: [
                Expected(segmentId: "John.3.16", exact: "αἰώνιον.", tokens: ["n1904:44003016025"]),
                Expected(segmentId: "John.3.17", exact: "οὐ γὰρ", tokens: ["n1904:44003017001", "n1904:44003017002"]),
            ],
            note: "last word of 16 through the first two of 17; diacritics on both sides"
        ),
        AnchorCase(
            name: "greek-diacritics-inside",
            paneID: "p3",
            target: .phrase("ἠγάπησεν"),
            expected: [Expected(segmentId: "John.3.16", exact: "ἠγάπησεν", tokens: ["n1904:44003016003"])],
            note: "precomposed breathing and accent (NFC in the pack): no cluster to split"
        ),
        AnchorCase(
            name: "note-emoji",
            paneID: "note",
            target: .core(126, 130),
            expected: [Expected(segmentId: "note-1", exact: "👍🏽", tokens: [])],
            note: "fixture selection: thumbs up with skin tone, 2 scalars, 4 UTF-16 units"
        ),
        AnchorCase(
            name: "note-emoji-split",
            paneID: "note",
            target: .display(128, 130),
            expected: [Expected(segmentId: "note-1", exact: "👍🏽", tokens: [])],
            snaps: true,
            note: "fixture selection 'between the emoji base and its modifier', snapped {126,128} scalars"
        ),
        AnchorCase(
            name: "note-isolated-word",
            paneID: "note",
            target: .core(19, 33),
            expected: [Expected(segmentId: "note-1", exact: "\u{2067}בְּרֵאשִׁ֖ית\u{2069}", tokens: [])],
            note: "fixture selection: first Hebrew word with RLI and PDI"
        ),
        AnchorCase(
            name: "note-compare-to-end",
            paneID: "note",
            target: .core(88, 131),
            expected: [Expected(segmentId: "note-1", exact: "Compare \u{2068}אֱלֹהִ֑ים\u{2069} in the same verse 👍🏽.", tokens: [])],
            note: "fixture selection: from 'Compare' to the end, across an isolate and a surrogate pair"
        ),
    ]

    static func named(_ name: String) -> AnchorCase? { all.first { $0.name == name } }

    /// The display range this case selects in `layout`, or nil when a phrase is not in the text.
    func displayRange(in layout: ReaderLayout) -> NSRange? {
        switch target {
        case .core(let start, let end):
            return layout.runMap.displayRange(fromCore: NSRange(location: start, length: end - start))
        case .phrase(let phrase):
            return layout.coreRange(ofPhrase: phrase).map { layout.runMap.displayRange(fromCore: $0) }
        case .display(let start, let end):
            return NSRange(location: start, length: end - start)
        }
    }

    /// Why `selection` does not match, or nil when it does. `asked` is the display range the
    /// case put on the text view; a cluster case expects the final range to be wider, whether
    /// the text view widened it itself (NSTextView, UITextView) or the surface did.
    func mismatch(_ selection: ReaderSelection?, asked: NSRange) -> String? {
        guard let selection else { return "\(name): no selection" }
        var problems: [String] = []
        let widened = selection.displayRange != asked || selection.snapped
        if selection.anchors.count != expected.count {
            problems.append("expected \(expected.count) anchors, got \(selection.anchors.count)")
        }
        for (anchor, want) in zip(selection.anchors, expected) {
            if anchor.segmentId != want.segmentId { problems.append("segment \(anchor.segmentId) ≠ \(want.segmentId)") }
            if anchor.quote?.exact != want.exact { problems.append("exact “\(anchor.quote?.exact ?? "")” ≠ “\(want.exact)”") }
            if let tokens = want.tokens, anchor.tokens != tokens { problems.append("tokens \(anchor.tokens) ≠ \(tokens)") }
        }
        if !selection.identical { problems.append("round trip not identical") }
        if widened != snaps { problems.append("widened \(widened) (text view kept \(selection.displayRange), surface snapped \(selection.snapped)), expected \(snaps)") }
        return problems.isEmpty ? nil : "\(name): " + problems.joined(separator: "; ")
    }
}
