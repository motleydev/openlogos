// The texts the shell reads come from the core itself: conformanceFixturesJson() returns the
// conformance fixtures crates/conformance ships (public-domain WLC Hebrew, CC0 Berean Standard
// Bible English, a CC0 study note with bidi isolates). No bundled or downloaded Scripture.
// The generated OpenLogosCore.swift is compiled into this module (project.yml), so its
// functions and records need no import here; CoreHost, a package, imports them instead.
//
// B005: verses carry the fixture's token layer (id and scalar range) and a passage carries its
// edition id and content revision, so the reading surface can hand the core a Paragraph and
// get the same anchors the conformance suite asserts.
import Foundation

/// One passage to show: a heading, its verses in order, and where the text comes from.
struct Passage: Identifiable {
    /// A token of a verse: its id (`edition:BBCCCVVVWWW[s]`) and its range in Unicode scalar values.
    struct Token: Equatable {
        let id: String
        let start: UInt32
        let end: UInt32
    }

    struct Verse: Identifiable {
        let id: String // the segment id, e.g. "Gen.1.1"
        let number: String
        let text: String
        /// The token layer in text order; empty when the edition has none (then the surface
        /// mints whitespace tokens, see ReaderContent).
        var tokens: [Token] = []
    }

    let id: String
    let heading: String
    let verses: [Verse]
    let attribution: String
    let isRightToLeft: Bool
    /// The fixture's edition id and content revision, as the core's anchors name them.
    var editionId: String = ""
    var contentRevision: String = ""
}

enum FixtureTexts {
    /// The shape of a text fixture as the core serialises it (snake_case keys).
    private struct Fixture: Decodable {
        struct Token: Decodable {
            let id: String
            let start: UInt32
            let end: UInt32
        }

        struct Segment: Decodable {
            let segmentId: String
            let text: String
            let tokens: [Token]?
        }

        let id: String
        let kind: String
        let license: String?
        let editionId: String?
        let contentRevision: String?
        let segments: [Segment]?
    }

    /// Fixture id to heading and reading direction. Anything else the core ships is ignored:
    /// the bidi study-note fixture (with its test emoji) stays in the conformance suite only.
    private static let shown: [(id: String, heading: String, rtl: Bool)] = [
        ("hebrew-cantillation-genesis-1", "בְּרֵאשִׁית א", true),
        ("cross-verse-selection-john-3", "John 3", false),
    ]

    /// The fixture's license line without its URLs, so the attribution stays one short line
    /// ("Text: public domain (Westminster Leningrad Codex). OSHB markup: CC BY 4.0").
    private static func attribution(_ license: String?) -> String {
        guard let license else { return "" }
        let words = license.split(separator: " ").filter { !$0.hasPrefix("http") }
        var text = words.joined(separator: " ")
        while let last = text.last, ",;: ".contains(last) { text.removeLast() }
        return text
    }

    /// Every text fixture the core ships, by id, including the study note.
    private static let all: [String: Fixture] = {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let data = Data(conformanceFixturesJson().utf8)
        guard let fixtures = try? decoder.decode([Fixture].self, from: data) else { return [:] }
        return Dictionary(uniqueKeysWithValues: fixtures.filter { $0.kind == "text" }.map { ($0.id, $0) })
    }()

    /// One text fixture as a passage, or nil when the core does not ship it.
    static func passage(id: String, heading: String, rtl: Bool) -> Passage? {
        guard let fixture = all[id] else { return nil }
        let verses = (fixture.segments ?? []).map { segment in
            // "Gen.1.1" -> "1"; a note segment such as "note-1" shows no number.
            let last = segment.segmentId.split(separator: ".").last.map(String.init) ?? ""
            let number = last.allSatisfy(\.isNumber) ? last : ""
            let tokens = (segment.tokens ?? []).map { Passage.Token(id: $0.id, start: $0.start, end: $0.end) }
            return Passage.Verse(id: segment.segmentId, number: number, text: segment.text, tokens: tokens)
        }
        return Passage(
            id: fixture.id,
            heading: heading,
            verses: verses,
            attribution: attribution(fixture.license),
            isRightToLeft: rtl,
            editionId: fixture.editionId ?? "",
            contentRevision: fixture.contentRevision ?? ""
        )
    }

    static func passages() -> [Passage] {
        shown.compactMap { passage(id: $0.id, heading: $0.heading, rtl: $0.rtl) }
    }
}
