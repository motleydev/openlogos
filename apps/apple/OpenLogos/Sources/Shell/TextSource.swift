// B072: where a tab's text comes from. One small host-side interface, resource id and
// reference in, display text and segments out, so B071's pack replaces the fixture source
// without touching the shell. Until then the texts are the conformance fixtures the core
// ships (FixtureTexts): BSB John 3:16–17, WLC Genesis 1:1–2, SBLGNT John 3:16 and the study
// note. A resource with no text for a passage answers nil and the tab shows the placeholder.
import Foundation

/// A resource's names, whether or not it has text for a passage.
struct ResourceInfo: Equatable {
    let id: String
    /// Tab labels: `BSB`.
    let shortName: String
    /// The identity row: `Berean Standard Bible`.
    let name: String
    let isRightToLeft: Bool
}

/// What a tab shows.
struct SourceText {
    let resource: ResourceInfo
    let content: ReaderContent
}

protocol TextSource {
    func resource(_ id: String) -> ResourceInfo
    /// The text of `resourceID` at `reference` (OSIS, `John.3.16`), or nil when there is none.
    func text(resourceID: String, reference: String?) -> SourceText?
}

/// The conformance fixtures, by resource and chapter.
struct FixtureTextSource: TextSource {
    private struct Entry {
        let fixture: String
        let chapter: String?
    }

    private static let resources: [String: ResourceInfo] = [
        "bsb": ResourceInfo(id: "bsb", shortName: "BSB", name: "Berean Standard Bible", isRightToLeft: false),
        "sblgnt": ResourceInfo(id: "sblgnt", shortName: "SBLGNT", name: "SBL Greek New Testament", isRightToLeft: false),
        "wlc": ResourceInfo(id: "wlc", shortName: "WLC", name: "Westminster Leningrad Codex", isRightToLeft: true),
        "fixture:note:1": ResourceInfo(id: "fixture:note:1", shortName: "Note", name: "Study note", isRightToLeft: false),
    ]

    private static let entries: [String: Entry] = [
        "bsb": Entry(fixture: "cross-verse-selection-john-3", chapter: "John.3"),
        "sblgnt": Entry(fixture: "greek-diacritics-john-3-sblgnt", chapter: "John.3"),
        "wlc": Entry(fixture: "hebrew-cantillation-genesis-1", chapter: "Gen.1"),
        "fixture:note:1": Entry(fixture: "bidi-isolation-note", chapter: nil),
    ]

    func resource(_ id: String) -> ResourceInfo {
        Self.resources[id] ?? ResourceInfo(id: id, shortName: id, name: id, isRightToLeft: false)
    }

    func text(resourceID: String, reference: String?) -> SourceText? {
        guard let entry = Self.entries[resourceID] else { return nil }
        if let chapter = entry.chapter {
            guard let reference, ShellReference.chapter(of: reference) == chapter else { return nil }
        }
        let info = resource(resourceID)
        guard let passage = FixtureTexts.passage(id: entry.fixture, heading: info.name, rtl: info.isRightToLeft) else { return nil }
        let content = ReaderContent(
            // One layout per opened passage: navigating rebuilds it, as a real chapter load would.
            id: "\(resourceID)|\(reference ?? "note")",
            editionId: passage.editionId.isEmpty ? resourceID : passage.editionId,
            contentRevision: passage.contentRevision,
            verses: passage.verses,
            isRightToLeft: info.isRightToLeft,
            attribution: passage.attribution.isEmpty ? "" : "Fixture: \(info.name). \(passage.attribution)"
        )
        return SourceText(resource: info, content: content)
    }
}

/// Reference labels for chrome. The core parses and formats (`formatHuman`); tab labels use a
/// short book form the core has no formatter for yet.
enum ShellReference {
    /// `John.3.16` → `John.3`.
    static func chapter(of osis: String) -> String {
        osis.split(separator: ".").prefix(2).joined(separator: ".")
    }

    /// `John.3.16` → `Jn 3` (tab labels).
    static func shortChapter(_ osis: String?) -> String {
        guard let osis else { return "" }
        let parts = osis.split(separator: ".")
        guard parts.count >= 2 else { return osis }
        return "\(shortBook(String(parts[0]))) \(parts[1])"
    }

    /// `John.3.16` → `Jn 3:16` (toasts).
    static func short(_ osis: String) -> String {
        let parts = osis.split(separator: ".")
        switch parts.count {
        case 3...: return "\(shortBook(String(parts[0]))) \(parts[1]):\(parts[2])"
        case 2: return "\(shortBook(String(parts[0]))) \(parts[1])"
        default: return osis
        }
    }

    /// `John.3.16` → `John 3:16` through the core.
    static func human(_ osis: String?) -> String {
        guard let osis else { return "" }
        let parts = osis.split(separator: ".")
        guard parts.count >= 2, let chapter = UInt32(parts[1]) else { return osis }
        let reference = Reference(book: String(parts[0]), chapter: chapter, verse: parts.count > 2 ? UInt32(parts[2]) : nil, subverse: nil)
        return (try? formatHuman(reference: reference)) ?? osis
    }

    private static let shortBooks: [String: String] = [
        "Gen": "Gen", "Exod": "Ex", "Lev": "Lev", "Num": "Num", "Deut": "Dt", "Ps": "Ps", "Prov": "Prov", "Isa": "Isa",
        "Matt": "Mt", "Mark": "Mk", "Luke": "Lk", "John": "Jn", "Acts": "Acts", "Rom": "Rom", "1Cor": "1 Cor",
        "2Cor": "2 Cor", "Gal": "Gal", "Eph": "Eph", "Phil": "Phil", "Col": "Col", "Heb": "Heb", "Jas": "Jas",
        "1John": "1 Jn", "Rev": "Rev",
    ]

    static func shortBook(_ book: String) -> String { shortBooks[book] ?? book }
}
