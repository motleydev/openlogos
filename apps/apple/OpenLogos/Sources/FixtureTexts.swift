// The texts the shell reads come from the core itself: conformanceFixturesJson() returns the
// conformance fixtures crates/conformance ships (public-domain WLC Hebrew, CC0 Berean Standard
// Bible English, a CC0 study note with bidi isolates). No bundled or downloaded Scripture.
// The generated OpenLogosCore.swift is compiled into this module (project.yml), so its
// functions and records need no import here; CoreHost, a package, imports them instead.
import Foundation

/// One passage to show: a heading, its verses in order, and where the text comes from.
struct Passage: Identifiable {
    struct Verse: Identifiable {
        let id: String // the segment id, e.g. "Gen.1.1"
        let number: String
        let text: String
    }

    let id: String
    let heading: String
    let verses: [Verse]
    let attribution: String
    let isRightToLeft: Bool
}

enum FixtureTexts {
    /// The shape of a text fixture as the core serialises it (snake_case keys).
    private struct Fixture: Decodable {
        struct Segment: Decodable {
            let segmentId: String
            let text: String
        }

        let id: String
        let kind: String
        let license: String?
        let segments: [Segment]?
    }

    /// Fixture id to heading and reading direction. Anything else the core ships is ignored.
    private static let shown: [(id: String, heading: String, rtl: Bool)] = [
        ("hebrew-cantillation-genesis-1", "בְּרֵאשִׁית א", true),
        ("cross-verse-selection-john-3", "John 3", false),
        ("bidi-isolation-note", "A study note", false),
    ]

    static func passages() -> [Passage] {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let data = Data(conformanceFixturesJson().utf8)
        guard let fixtures = try? decoder.decode([Fixture].self, from: data) else { return [] }
        let byId = Dictionary(uniqueKeysWithValues: fixtures.map { ($0.id, $0) })
        return shown.compactMap { entry in
            guard let fixture = byId[entry.id], fixture.kind == "text" else { return nil }
            let verses = (fixture.segments ?? []).map { segment in
                // "Gen.1.1" -> "1"; a note segment such as "note-1" shows no number.
                let last = segment.segmentId.split(separator: ".").last.map(String.init) ?? ""
                let number = last.allSatisfy(\.isNumber) ? last : ""
                return Passage.Verse(id: segment.segmentId, number: number, text: segment.text)
            }
            return Passage(
                id: fixture.id,
                heading: entry.heading,
                verses: verses,
                attribution: fixture.license ?? "",
                isRightToLeft: entry.rtl
            )
        }
    }
}
