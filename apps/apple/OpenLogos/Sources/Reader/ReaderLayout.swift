// What the TextKit 2 reading surface lays out and how its offsets relate to the core's (B005,
// docs/adr/0001-rust-core-native-ui.md). The core measures a selection on the segments' texts
// joined by one joiner; the surface shows more than that (verse numbers, a thin space after
// each, paragraph breaks in long sets, the attribution line), so every character of the
// display string is either *core text* or a *non-core run*. RunMap translates UTF-16 offsets
// between the two: a selection end inside a verse number collapses to the number's edge, and
// a highlight the core hands back never covers a number. The core itself is given a Paragraph
// built here from the same verses, with the fixture's token layer or minted whitespace tokens.
import Foundation
#if canImport(UIKit)
import UIKit
typealias PlatformFont = UIFont
typealias PlatformColor = UIColor
#else
import AppKit
typealias PlatformFont = NSFont
typealias PlatformColor = NSColor
#endif

extension Passage.Verse: Equatable {
    static func == (lhs: Passage.Verse, rhs: Passage.Verse) -> Bool {
        lhs.id == rhs.id && lhs.text == rhs.text && lhs.number == rhs.number && lhs.tokens == rhs.tokens
    }
}

/// The verses of one resource as the surface shows them, with the edition the core's anchors
/// will name. Equality is by identity: a pane's content does not change under one id.
struct ReaderContent: Equatable {
    let id: String
    let editionId: String
    let contentRevision: String
    let verses: [Passage.Verse]
    let isRightToLeft: Bool
    let attribution: String
    /// Text between consecutive verses in the rendered run and in the core's joined text.
    var joiner = " "
    /// Long sets break into paragraphs every n verses with a non-core newline (TextKit 2 lays
    /// out per paragraph, so one 1,000-verse paragraph would defeat its viewport layout).
    var paragraphEvery: Int?

    init(content: PaneContent, paragraphEvery: Int? = nil) {
        id = content.id
        editionId = content.editionId.isEmpty ? content.id : content.editionId
        contentRevision = content.contentRevision
        verses = content.verses
        isRightToLeft = content.isRightToLeft
        attribution = content.license.hasPrefix(content.fullTitle)
            ? "Fixture: \(content.license)"
            : "Fixture: \(content.fullTitle). \(content.license)"
        self.paragraphEvery = paragraphEvery ?? (content.verses.count > 20 ? 8 : nil)
    }

    /// Content from a host's text source (B072's TextSource) rather than a prototype pane.
    init(id: String, editionId: String, contentRevision: String, verses: [Passage.Verse], isRightToLeft: Bool, attribution: String) {
        self.id = id
        self.editionId = editionId
        self.contentRevision = contentRevision
        self.verses = verses
        self.isRightToLeft = isRightToLeft
        self.attribution = attribution
        paragraphEvery = verses.count > 20 ? 8 : nil
    }

    static func == (lhs: ReaderContent, rhs: ReaderContent) -> Bool {
        lhs.id == rhs.id && lhs.verses.count == rhs.verses.count && lhs.paragraphEvery == rhs.paragraphEvery
    }
}

/// UTF-16 offsets of the display string that are not core text, and the translation both ways.
struct RunMap: Equatable {
    /// Non-core runs in display order, non-overlapping.
    let nonCore: [NSRange]

    /// The core range a display selection covers. An end inside a non-core run moves to the
    /// run's edge, so a selection that only covers a verse number is empty.
    func coreRange(fromDisplay range: NSRange) -> NSRange {
        let start = coreOffset(fromDisplay: range.location)
        let end = coreOffset(fromDisplay: range.location + range.length)
        return NSRange(location: start, length: max(0, end - start))
    }

    func coreOffset(fromDisplay offset: Int) -> Int {
        var removed = 0
        for run in nonCore {
            if run.location + run.length <= offset {
                removed += run.length
            } else if run.location < offset {
                // Strictly inside the run: both ends collapse to the same core position.
                return run.location - removed
            } else {
                break
            }
        }
        return offset - removed
    }

    /// The display range a core range paints: a start skips the runs before it (a verse number
    /// before its verse), an end stops before the runs after it.
    func displayRange(fromCore range: NSRange) -> NSRange {
        let start = displayOffset(fromCore: range.location, isStart: true)
        let end = displayOffset(fromCore: range.location + range.length, isStart: false)
        return NSRange(location: start, length: max(0, end - start))
    }

    func displayOffset(fromCore offset: Int, isStart: Bool) -> Int {
        var display = offset
        for run in nonCore {
            if isStart ? run.location <= display : run.location < display {
                display += run.length
            } else {
                break
            }
        }
        return display
    }

    /// The display text with every non-core run removed: what the core was given.
    func coreText(ofDisplay display: String) -> String {
        let string = display as NSString
        var result = ""
        var cursor = 0
        for run in nonCore {
            result += string.substring(with: NSRange(location: cursor, length: run.location - cursor))
            cursor = run.location + run.length
        }
        result += string.substring(from: cursor)
        return result
    }
}

/// Fonts and spacing the surface lays out with: Dynamic Type styles in the system serif.
struct ReaderTypography {
    let body: PlatformFont
    let marker: PlatformFont
    let caption: PlatformFont
    let lineSpacing: CGFloat
    let markerBaselineOffset: CGFloat
    /// Text and verse-number colours; the system label colours unless a host theme sets them.
    var text: PlatformColor = .readerPrimary
    var secondary: PlatformColor = .readerSecondary
    /// Distinguishes two typographies for relayout (B072: size steps, spacing and theme).
    var key = "dynamic"

    /// A fixed size in the system serif (New York) at a line-height multiple, in a host
    /// theme's colours (B072's tokens). The spacing follows `current()`: 1.5 → 0.45 × size.
    static func fixed(size: CGFloat, lineHeight: CGFloat, text: PlatformColor, secondary: PlatformColor, key: String) -> ReaderTypography {
        let marker = (size * 0.6).rounded()
        let caption = (size * 0.68).rounded()
        return ReaderTypography(
            body: serif(size: size),
            marker: serif(size: marker),
            caption: serif(size: caption),
            lineSpacing: (size * (lineHeight - 1.05)).rounded(),
            markerBaselineOffset: (size * 0.35).rounded(),
            text: text,
            secondary: secondary,
            key: key
        )
    }

    private static func serif(size: CGFloat) -> PlatformFont {
        let base = PlatformFont.systemFont(ofSize: size)
        guard let descriptor = base.fontDescriptor.withDesign(.serif) else { return base }
        #if canImport(UIKit)
        return PlatformFont(descriptor: descriptor, size: size)
        #else
        return PlatformFont(descriptor: descriptor, size: size) ?? base
        #endif
    }

    static func current() -> ReaderTypography {
        let body = serif(.title3)
        return ReaderTypography(
            body: body,
            marker: serif(.caption2),
            caption: serif(.caption1),
            // About 1.5 line height, as the B004 prototype's ScaledMetric leading (9 at 20 pt).
            lineSpacing: (body.pointSize * 0.45).rounded(),
            markerBaselineOffset: (body.pointSize * 0.35).rounded()
        )
    }

    private static func serif(_ style: PlatformFont.TextStyle) -> PlatformFont {
        let base = PlatformFont.preferredFont(forTextStyle: style)
        guard let descriptor = base.fontDescriptor.withDesign(.serif) else { return base }
        #if canImport(UIKit)
        return PlatformFont(descriptor: descriptor, size: 0)
        #else
        return PlatformFont(descriptor: descriptor, size: 0) ?? base
        #endif
    }
}

extension NSAttributedString.Key {
    /// The segment id of a verse's core text run.
    static let openLogosSegment = NSAttributedString.Key("org.openlogos.segment")
    /// The segment id a verse marker introduces (for the VoiceOver rotor).
    static let openLogosMarker = NSAttributedString.Key("org.openlogos.marker")
}

/// One resource laid out: the attributed display string, the run map, and the core's
/// Paragraph with where each segment sits in the joined core text.
struct ReaderLayout {
    let content: ReaderContent
    let display: NSAttributedString
    let runMap: RunMap
    let paragraph: Paragraph
    /// The segments' texts joined by the joiner: what the core measures the selection on.
    let coreText: String
    /// UTF-16 offset of each segment within `coreText`.
    let segmentOffsets: [String: Int]
    let segmentTexts: [String: String]
    /// Display ranges of the verse markers in order, with the segment each introduces.
    let markers: [(segmentId: String, range: NSRange)]

    init(content: ReaderContent, typography: ReaderTypography = .current()) {
        self.content = content
        let paragraphStyle = NSMutableParagraphStyle()
        paragraphStyle.lineSpacing = typography.lineSpacing
        paragraphStyle.paragraphSpacing = typography.lineSpacing * 1.5
        paragraphStyle.alignment = .natural
        paragraphStyle.baseWritingDirection = content.isRightToLeft ? .rightToLeft : .leftToRight
        let bodyAttributes: [NSAttributedString.Key: Any] = [
            .font: typography.body,
            .foregroundColor: typography.text,
            .paragraphStyle: paragraphStyle,
        ]
        let markerAttributes: [NSAttributedString.Key: Any] = [
            .font: typography.marker,
            .foregroundColor: typography.secondary,
            .paragraphStyle: paragraphStyle,
            .baselineOffset: typography.markerBaselineOffset,
        ]
        let captionAttributes: [NSAttributedString.Key: Any] = [
            .font: typography.caption,
            .foregroundColor: typography.secondary,
            .paragraphStyle: paragraphStyle,
        ]

        let display = NSMutableAttributedString()
        var nonCore: [NSRange] = []
        var markers: [(String, NSRange)] = []
        var core = ""
        var offsets: [String: Int] = [:]
        var texts: [String: String] = [:]
        var segments: [Segment] = []

        func appendNonCore(_ text: String, _ attributes: [NSAttributedString.Key: Any]) {
            let range = NSRange(location: display.length, length: (text as NSString).length)
            display.append(NSAttributedString(string: text, attributes: attributes))
            nonCore.append(range)
        }

        for (index, verse) in content.verses.enumerated() {
            if index > 0 {
                display.append(NSAttributedString(string: content.joiner, attributes: bodyAttributes))
                core += content.joiner
                if let every = content.paragraphEvery, index % every == 0 {
                    appendNonCore("\n", bodyAttributes)
                }
            }
            if !verse.number.isEmpty {
                var attributes = markerAttributes
                attributes[.openLogosMarker] = verse.id
                let start = display.length
                appendNonCore(verse.number + "\u{2009}", attributes)
                markers.append((verse.id, NSRange(location: start, length: display.length - start)))
            }
            var attributes = bodyAttributes
            attributes[.openLogosSegment] = verse.id
            display.append(NSAttributedString(string: verse.text, attributes: attributes))
            offsets[verse.id] = (core as NSString).length
            texts[verse.id] = verse.text
            core += verse.text
            let tokens = verse.tokens.isEmpty
                ? ReaderLayout.mintedTokens(edition: content.editionId, segmentId: verse.id, text: verse.text)
                : verse.tokens
            segments.append(Segment(
                segmentId: verse.id,
                text: verse.text,
                tokens: tokens.map { TokenSpan(id: $0.id, range: ScalarRange(start: $0.start, end: $0.end)) }
            ))
        }
        if !content.attribution.isEmpty {
            appendNonCore("\n\n" + content.attribution, captionAttributes)
        }

        self.display = display
        runMap = RunMap(nonCore: nonCore)
        paragraph = Paragraph(
            editionId: content.editionId,
            contentRevision: content.contentRevision,
            joiner: content.joiner,
            segments: segments
        )
        coreText = core
        segmentOffsets = offsets
        segmentTexts = texts
        self.markers = markers.map { (segmentId: $0.0, range: $0.1) }
    }

    /// The core UTF-16 range of the first occurrence of `phrase` in the core text.
    func coreRange(ofPhrase phrase: String) -> NSRange? {
        let range = (coreText as NSString).range(of: phrase)
        return range.location == NSNotFound ? nil : range
    }

    // MARK: Minted tokens

    /// USFM book numbers for the OSIS-style book ids the core uses (`BookId`), as the token id
    /// format needs them. The core's `TokenId::parse` is the authority; this table only mints
    /// prototype ids for texts whose fixtures carry no token layer.
    static let bookNumbers: [String: Int] = {
        let order = [
            "Gen", "Exod", "Lev", "Num", "Deut", "Josh", "Judg", "Ruth", "1Sam", "2Sam", "1Kgs", "2Kgs",
            "1Chr", "2Chr", "Ezra", "Neh", "Esth", "Job", "Ps", "Prov", "Eccl", "Song", "Isa", "Jer",
            "Lam", "Ezek", "Dan", "Hos", "Joel", "Amos", "Obad", "Jonah", "Mic", "Nah", "Hab", "Zeph",
            "Hag", "Zech", "Mal",
        ]
        var table: [String: Int] = [:]
        for (index, book) in order.enumerated() { table[book] = index + 1 }
        // USFM skips 40.
        let newTestament = [
            "Matt", "Mark", "Luke", "John", "Acts", "Rom", "1Cor", "2Cor", "Gal", "Eph", "Phil", "Col",
            "1Thess", "2Thess", "1Tim", "2Tim", "Titus", "Phlm", "Heb", "Jas", "1Pet", "2Pet", "1John",
            "2John", "3John", "Jude", "Rev",
        ]
        for (index, book) in newTestament.enumerated() { table[book] = index + 41 }
        return table
    }()

    /// Whitespace-separated words with their punctuation, as the BSB fixture tokenizes, with
    /// ids minted from the segment id (`Book.C.V`). Empty when the segment id is not a verse.
    static func mintedTokens(edition: String, segmentId: String, text: String) -> [Passage.Token] {
        let parts = segmentId.split(separator: ".")
        guard parts.count == 3, let book = bookNumbers[String(parts[0])],
              let chapter = Int(parts[1]), let verse = Int(parts[2]), chapter <= 999, verse <= 999
        else { return [] }
        var tokens: [Passage.Token] = []
        var start: UInt32?
        var offset: UInt32 = 0
        func close(at end: UInt32) {
            guard let begin = start, tokens.count < 999 else { return }
            let id = String(format: "%@:%02d%03d%03d%03d", edition, book, chapter, verse, tokens.count + 1)
            tokens.append(Passage.Token(id: id, start: begin, end: end))
            start = nil
        }
        for scalar in text.unicodeScalars {
            if scalar.properties.isWhitespace {
                close(at: offset)
            } else if start == nil {
                start = offset
            }
            offset += 1
        }
        close(at: offset)
        return tokens
    }
}

extension PlatformColor {
    static var readerPrimary: PlatformColor {
        #if canImport(UIKit)
        return .label
        #else
        return .labelColor
        #endif
    }

    static var readerSecondary: PlatformColor {
        #if canImport(UIKit)
        return .secondaryLabel
        #else
        return .secondaryLabelColor
        #endif
    }

    /// The anchor highlight: a translucent marker tone that does not depend on the selection.
    static var readerHighlight: PlatformColor {
        #if canImport(UIKit)
        return UIColor.systemYellow.withAlphaComponent(0.45)
        #else
        return NSColor.systemYellow.withAlphaComponent(0.45)
        #endif
    }
}
