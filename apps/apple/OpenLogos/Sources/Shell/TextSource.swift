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
    /// BCP 47 language tag (`en`, `grc`, `hbo`), registered with the core
    /// (`setResourceLanguages`) so original-language tabs get the original tools.
    var language = "en"
}

/// What a tab shows.
struct SourceText {
    let resource: ResourceInfo
    let content: ReaderContent
}

protocol TextSource {
    /// Every resource's short name by id, registered with the core (`setResourceTitles`).
    var resourceTitles: [String: String] { get }
    /// Every resource's language tag by id (`setResourceLanguages`).
    var resourceLanguages: [String: String] { get }
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
        "sblgnt": ResourceInfo(id: "sblgnt", shortName: "SBLGNT", name: "SBL Greek New Testament", isRightToLeft: false, language: "grc"),
        "wlc": ResourceInfo(id: "wlc", shortName: "WLC", name: "Westminster Leningrad Codex", isRightToLeft: true, language: "hbo"),
        "fixture:note:1": ResourceInfo(id: "fixture:note:1", shortName: "Note", name: "Study note", isRightToLeft: false),
    ]

    private static let entries: [String: Entry] = [
        "bsb": Entry(fixture: "cross-verse-selection-john-3", chapter: "John.3"),
        "sblgnt": Entry(fixture: "greek-diacritics-john-3-sblgnt", chapter: "John.3"),
        "wlc": Entry(fixture: "hebrew-cantillation-genesis-1", chapter: "Gen.1"),
        "fixture:note:1": Entry(fixture: "bidi-isolation-note", chapter: nil),
    ]

    var resourceTitles: [String: String] { Self.resources.mapValues(\.shortName) }
    var resourceLanguages: [String: String] { Self.resources.mapValues(\.language) }

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

/// Reference labels for chrome, all from the core: `formatShort` (`Jn 3:16`, tab labels,
/// palette rows, toasts) and `formatHuman` (`John 3:16`, the identity row).
enum ShellReference {
    /// `John.3.16` → `John.3`.
    static func chapter(of osis: String) -> String {
        osis.split(separator: ".").prefix(2).joined(separator: ".")
    }

    /// `John.3.16` → `Jn 3:16`.
    static func short(_ reference: String) -> String {
        (try? formatShort(reference: reference)) ?? reference
    }

    /// `John.3.16–17` (ReaderSelection.reference) → `John 3:16–17`; one verse as `human`.
    static func humanRange(_ reference: String) -> String {
        let parts = reference.split(separator: "–", maxSplits: 1).map(String.init)
        guard parts.count == 2 else { return human(reference) }
        let tail = parts[1].contains(".") ? human(parts[1]) : parts[1]
        return "\(human(parts[0]))–\(tail)"
    }

    /// `grc` → `Greek`, for the lexicon tab.
    static func languageName(_ tag: String) -> String {
        switch tag {
        case "grc", "el": return "Greek"
        case "hbo", "he", "heb": return "Hebrew"
        case "arc": return "Aramaic"
        case "en": return "English"
        default: return tag
        }
    }

    /// `John.3.16` → `John 3:16`.
    static func human(_ osis: String?) -> String {
        guard let osis else { return "" }
        let parts = osis.split(separator: ".")
        guard parts.count >= 2, let chapter = UInt32(parts[1]) else { return osis }
        let reference = Reference(book: String(parts[0]), chapter: chapter, verse: parts.count > 2 ? UInt32(parts[2]) : nil, subverse: nil)
        return (try? formatHuman(reference: reference)) ?? osis
    }
}
