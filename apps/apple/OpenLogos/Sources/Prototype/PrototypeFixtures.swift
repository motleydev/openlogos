// Fixture data for the B004 prototype. Everything here is hard-coded and labelled "fixture" on
// screen. Only public-domain or CC licensed text appears: the core's conformance fixtures
// (WLC Genesis 1:1–2, public domain; Berean Standard Bible John 3:16–17, CC0) through
// FixtureTexts, plus the Berean Standard Bible for Genesis 1:1–2 (CC0), the King James Version
// (public domain) and the Greek text of John 3:16–17 typed for the prototype in the public-
// domain Byzantine text form. Nothing is fetched. Cross references, lexical data, search hits,
// collection items and vault paths are invented shapes for the interaction, not datasets, and
// carry no claim of accuracy.
import Foundation

/// How a pane navigates (specs/21 §2).
enum LinkMode: Equatable {
    case pinned
    case group(Int)
    case follows

    var label: String {
        switch self {
        case .pinned: return "Pinned"
        case .group(let n): return "Group \(n)"
        case .follows: return "Follows"
        }
    }

    /// A glyph beside the word so the state never rests on color alone.
    var symbol: String {
        switch self {
        case .pinned: return "pin"
        case .group: return "link"
        case .follows: return "arrow.turn.down.right"
        }
    }
}

/// What a pane shows: a resource at a local passage.
struct PaneContent: Identifiable, Equatable {
    enum Kind: Equatable { case scripture, commentary }

    let id: String
    let shortName: String
    let fullTitle: String
    let license: String
    /// "Book label" plus a reference, e.g. "Κατὰ Ἰωάννην 3:16–17": the identity row truncates
    /// the book label and never the reference.
    let passage: String
    var bookLabel: String { passage.split(separator: " ").dropLast().joined(separator: " ") }
    var reference: String { String(passage.split(separator: " ").last ?? "") }
    let verses: [Passage.Verse]
    let isRightToLeft: Bool
    let kind: Kind
    /// A resource the workspace references but that is not installed (specs/21 §1).
    let isInstalled: Bool
    /// A resource whose rights allow reference-only capture (specs/21 §11 "Read-only").
    let isReferenceOnly: Bool

    static func == (lhs: PaneContent, rhs: PaneContent) -> Bool { lhs.id == rhs.id }
}

struct CollectionItem: Identifiable {
    enum Kind: String { case passage, excerpt, observation, crossReference, marginSheet }
    enum Rights: String { case quotable, metadataOnly }
    let id: String
    let kind: Kind
    let resource: String
    let locator: String
    let text: String
    let rights: Rights
}

struct CrossReferenceFixture: Identifiable {
    let id: String
    let target: String
    let kind: String
    let source: String
    let preview: String
    let installed: Bool
}

struct SearchHit: Identifiable {
    let id: String
    let reference: String
    let edition: String
    let before: String
    let match: String
    let after: String
}

enum PrototypeFixtures {
    // MARK: Texts

    static let bsbJohn3: PaneContent = {
        let core = FixtureTexts.passages().first { $0.id == "cross-verse-selection-john-3" }
        return PaneContent(
            id: "bsb-john-3",
            shortName: "BSB",
            fullTitle: "Berean Standard Bible",
            license: core?.attribution ?? "CC0 1.0",
            passage: "John 3:16–17",
            verses: core?.verses ?? [],
            isRightToLeft: false,
            kind: .scripture,
            isInstalled: true,
            isReferenceOnly: false
        )
    }()

    static let wlcGenesis1: PaneContent = {
        let core = FixtureTexts.passages().first { $0.id == "hebrew-cantillation-genesis-1" }
        return PaneContent(
            id: "wlc-gen-1",
            shortName: "WLC",
            fullTitle: "Westminster Leningrad Codex",
            license: core?.attribution ?? "public domain",
            passage: "בראשית א׳ 1–2",
            verses: core?.verses ?? [],
            isRightToLeft: true,
            kind: .scripture,
            isInstalled: true,
            isReferenceOnly: false
        )
    }()

    static let bsbGenesis1 = PaneContent(
        id: "bsb-gen-1",
        shortName: "BSB",
        fullTitle: "Berean Standard Bible",
        license: "CC0 1.0 (Berean Standard Bible)",
        passage: "Genesis 1:1–2",
        verses: [
            .init(id: "Gen.1.1", number: "1", text: "In the beginning God created the heavens and the earth."),
            .init(id: "Gen.1.2", number: "2", text: "Now the earth was formless and void, and darkness was over the surface of the deep. And the Spirit of God was hovering over the surface of the waters."),
        ],
        isRightToLeft: false,
        kind: .scripture,
        isInstalled: true,
        isReferenceOnly: false
    )

    static let kjvJohn3 = PaneContent(
        id: "kjv-john-3",
        shortName: "KJV",
        fullTitle: "King James Version (1769 Blayney text)",
        license: "public domain",
        passage: "John 3:16–17",
        verses: [
            .init(id: "John.3.16", number: "16", text: "For God so loved the world, that he gave his only begotten Son, that whosoever believeth in him should not perish, but have everlasting life."),
            .init(id: "John.3.17", number: "17", text: "For God sent not his Son into the world to condemn the world; but that the world through him might be saved."),
        ],
        isRightToLeft: false,
        kind: .scripture,
        isInstalled: true,
        isReferenceOnly: false
    )

    /// The exact verse bytes of the Nestle 1904 pack B064 built (`text` table, verse ids
    /// 44003016 and 44003017), pasted unchanged; no Greek is typed from memory here.
    static let greekJohn3 = PaneContent(
        id: "n1904-john-3",
        shortName: "N1904",
        fullTitle: "Nestle 1904 Greek New Testament",
        license: "Nestle 1904 Greek New Testament via CenterBLC/N1904 (MACULA Greek, CC BY 4.0), source S73",
        passage: "Κατὰ Ἰωάννην 3:16–17",
        verses: [
            .init(id: "John.3.16", number: "16", text: "Οὕτως γὰρ ἠγάπησεν ὁ Θεὸς τὸν κόσμον, ὥστε τὸν Υἱὸν τὸν μονογενῆ ἔδωκεν, ἵνα πᾶς ὁ πιστεύων εἰς αὐτὸν μὴ ἀπόληται ἀλλ’ ἔχῃ ζωὴν αἰώνιον."),
            .init(id: "John.3.17", number: "17", text: "οὐ γὰρ ἀπέστειλεν ὁ Θεὸς τὸν Υἱὸν εἰς τὸν κόσμον ἵνα κρίνῃ τὸν κόσμον, ἀλλ’ ἵνα σωθῇ ὁ κόσμος δι’ αὐτοῦ."),
        ],
        isRightToLeft: false,
        kind: .scripture,
        isInstalled: true,
        isReferenceOnly: false
    )

    /// A commentary the workspace remembers but that is not installed: the unavailable-source
    /// placeholder (specs/21 §1, §11). No text is shown for it, so none is quoted here.
    static let calvinJohnMissing = PaneContent(
        id: "calvin-john-missing",
        shortName: "Calvin, John",
        fullTitle: "John Calvin, Commentary on the Gospel according to John (Pringle translation)",
        license: "public domain (translation 1847)",
        passage: "John 3:16",
        verses: [],
        isRightToLeft: false,
        kind: .commentary,
        isInstalled: false,
        isReferenceOnly: false
    )

    /// A view-only resource: capture yields reference metadata, not quotation (specs/21 §11).
    static let referenceOnlyJohn3 = PaneContent(
        id: "ref-only-john-3",
        shortName: "Borrowed ed.",
        fullTitle: "Borrowed edition (fixture: view-only rights)",
        license: "view only; quotation not permitted (fixture)",
        passage: "John 3:16–17",
        verses: kjvJohn3.verses,
        isRightToLeft: false,
        kind: .scripture,
        isInstalled: true,
        isReferenceOnly: true
    )

    // MARK: Invented interaction shapes

    static let collectionName = "John 3 study"

    static let collectionItems: [CollectionItem] = [
        .init(id: "c1", kind: .passage, resource: "BSB", locator: "John 3:16", text: "For God so loved the world that He gave His one and only Son…", rights: .quotable),
        .init(id: "c2", kind: .excerpt, resource: "N1904", locator: "John 3:16, μονογενῆ", text: "τὸν υἱὸν αὐτοῦ τὸν μονογενῆ", rights: .quotable),
        .init(id: "c3", kind: .observation, resource: "Me", locator: "on John 3:16", text: "\"One and only\" (BSB) versus \"only begotten\" (KJV): compare renderings before teaching this.", rights: .quotable),
        .init(id: "c4", kind: .crossReference, resource: "Borrowed ed.", locator: "1 John 4:9", text: "reference only: quotation not permitted in this edition", rights: .metadataOnly),
    ]

    static let crossReferences: [CrossReferenceFixture] = [
        .init(id: "x1", target: "Romans 5:8", kind: "imported cross reference", source: "fixture list", preview: "But God proves His love for us in this: While we were still sinners, Christ died for us.", installed: true),
        .init(id: "x2", target: "1 John 4:9", kind: "imported cross reference", source: "fixture list", preview: "This is how God's love was revealed among us: God sent His one and only Son into the world, so that we might live through Him.", installed: true),
        .init(id: "x3", target: "Genesis 22:2", kind: "thematic connection", source: "fixture editorial scheme", preview: "", installed: false),
    ]

    static let searchHits: [SearchHit] = [
        .init(id: "s1", reference: "John 3:16", edition: "BSB", before: "He gave His ", match: "one and only", after: " Son, that everyone who believes"),
        .init(id: "s2", reference: "John 1:14", edition: "BSB", before: "the glory of the ", match: "one and only", after: " Son from the Father"),
        .init(id: "s3", reference: "John 1:18", edition: "BSB", before: "but the ", match: "one and only", after: " Son, who is Himself God"),
        .init(id: "s4", reference: "1 John 4:9", edition: "BSB", before: "God sent His ", match: "one and only", after: " Son into the world"),
    ]

    /// The capture target for the Selection view: the BSB words and the Greek tokens they align
    /// to in this fixture. Alignment is a fixture, not a dataset.
    static let selectedWords = "one and only"
    static let alignedToken = "μονογενῆ"
    static let lemma = "μονογενής"
    static let transliteration = "monogenēs"
    static let morphologySummary = "adjective, accusative singular masculine; describes \"Son\""
    static let morphologyCode = "A-ASM"
    static let renderings: [(String, Int)] = [("one and only", 5), ("only", 3), ("only child", 1)]
}
