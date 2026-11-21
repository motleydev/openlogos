// B078: a continuous reading window laid out (specs/23 §7). A window is a run of the core's
// chapter blocks (`ReadingHandle.chapterContent`) shown as one text, so selection, anchors,
// the context menu and Find work across it as they do on one passage. Each block is a heading
// (the book's title on its first chapter, the chapter label, the placeholder notice) and its
// verses: the label as a verse number (none for a Psalm title), a new paragraph at
// `paragraph_start`, a line of its own for poetry, the title styled for a superscription,
// placeholder verses muted. Headings, notices and line breaks are non-core runs; the verses
// joined by the joiner are the core text, so one window is one Paragraph for the core.
//
// Blocks are joined by the joiner (core) and a line break (non-core), so the display of a
// window is the blocks' displays joined by that separator: adding a block above or below, or
// releasing one, is one insertion or deletion at an end of the text storage (`Edit`), and
// the text already laid out stays as it is.
import Foundation
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// One chapter block's place in a window's display string.
struct ReaderBlockSpan: Equatable {
    let chapterKey: String
    /// `Jn 3`, for the sticky header.
    let shortLabel: String
    /// From the heading's first character to the block's last (the separator before it excluded).
    let range: NSRange
    let headingRange: NSRange
    let placeholder: Bool
}

/// Where a highlight sits, independent of the window: a UTF-16 range inside a segment's text.
struct SegmentSpan: Equatable, Hashable {
    let segmentId: String
    let start: Int
    let length: Int
}

extension NSAttributedString.Key {
    /// The chapter key of every character of a block (B078).
    static let openLogosChapter = NSAttributedString.Key("org.openlogos.chapter")
    /// Marks a block's heading run.
    static let openLogosHeading = NSAttributedString.Key("org.openlogos.heading")
}

/// The text a block shows when its edition is a placeholder (specs/23 §5).
enum ReadingNotice {
    static let placeholder = "Placeholder text — this edition is not installed"
}

extension ReaderLayout {
    /// The separator between two blocks: the joiner (core) and a line break (non-core).
    static let blockBreak = "\n"

    /// A window of chapter blocks. `attribution` is a caption line after a block whose real
    /// verses need one (the fixtures' licences).
    init(blocks chapters: [ReadingChapter], id: String, editionId: String, isRightToLeft: Bool,
         typography: ReaderTypography = .current(), attribution: (ReadingChapter) -> String? = { _ in nil }) {
        self.init(pieces: chapters.map { ReaderBlockPiece(chapter: $0, editionId: editionId, isRightToLeft: isRightToLeft, typography: typography, attribution: attribution($0)) },
                  id: id, editionId: editionId, isRightToLeft: isRightToLeft)
    }

    /// A window from laid-out blocks (a stream keeps them, so a block is laid out once while it
    /// stays near the reader): the pieces joined by their separators, ranges offset.
    init(pieces: [ReaderBlockPiece], id: String, editionId: String, isRightToLeft: Bool) {
        var content = ReaderContent(id: id, editionId: editionId, contentRevision: pieces.first?.contentRevision ?? "",
                                    verses: pieces.flatMap(\.verses), isRightToLeft: isRightToLeft, attribution: "")
        content.paragraphEvery = nil
        self.content = content

        let display = NSMutableAttributedString()
        var nonCore: [NSRange] = []
        var markers: [(segmentId: String, range: NSRange)] = []
        var core = 0
        var offsets: [String: Int] = [:]
        var texts: [String: String] = [:]
        var segments: [Segment] = []
        var blocks: [ReaderBlockSpan] = []
        var order: [String] = []
        var verseRanges: [String: NSRange] = [:]
        var placeholders: Set<String> = []
        var joined = ""

        for (index, piece) in pieces.enumerated() {
            if index > 0 {
                let previous = pieces[index - 1]
                // The joiner (core) and the line break (non-core), in the previous block's style.
                display.append(previous.separator)
                core += (content.joiner as NSString).length
                joined += content.joiner
                nonCore.append(NSRange(location: display.length - (ReaderLayout.blockBreak as NSString).length, length: (ReaderLayout.blockBreak as NSString).length))
            }
            let base = display.length
            display.append(piece.display)
            func shift(_ range: NSRange) -> NSRange { NSRange(location: range.location + base, length: range.length) }
            nonCore += piece.nonCore.map(shift)
            markers += piece.markers.map { (segmentId: $0.segmentId, range: shift($0.range)) }
            for (id, range) in piece.verseRanges { verseRanges[id] = shift(range) }
            for (id, offset) in piece.segmentOffsets { offsets[id] = offset + core }
            texts.merge(piece.segmentTexts) { a, _ in a }
            segments += piece.segments
            order += piece.order
            placeholders.formUnion(piece.placeholders)
            core += piece.coreLength
            joined += piece.coreText
            blocks.append(ReaderBlockSpan(chapterKey: piece.chapterKey, shortLabel: piece.shortLabel, range: shift(NSRange(location: 0, length: piece.display.length)),
                                          headingRange: shift(piece.headingRange), placeholder: piece.placeholder))
        }

        self.display = display
        runMap = RunMap(nonCore: nonCore)
        paragraph = Paragraph(editionId: editionId, contentRevision: content.contentRevision, joiner: content.joiner, segments: segments)
        coreText = joined
        segmentOffsets = offsets
        segmentTexts = texts
        self.markers = markers
        self.blocks = blocks
        segmentOrder = order
        self.verseRanges = verseRanges
        placeholderSegments = placeholders
    }

    // MARK: Spans

    /// Where a display range sits as segment spans (one per segment it covers).
    func spans(ofDisplay range: NSRange) -> [SegmentSpan] {
        let coreRange = runMap.coreRange(fromDisplay: range)
        guard coreRange.length > 0 else { return [] }
        var spans: [SegmentSpan] = []
        let end = NSMaxRange(coreRange)
        for id in segmentOrder.isEmpty ? content.verses.map(\.id) : segmentOrder {
            guard let start = segmentOffsets[id], let text = segmentTexts[id] else { continue }
            let length = (text as NSString).length
            let lo = max(start, coreRange.location), hi = min(start + length, end)
            if hi > lo { spans.append(SegmentSpan(segmentId: id, start: lo - start, length: hi - lo)) }
            if start >= end { break }
        }
        return spans
    }

    /// The display ranges of spans in this layout; spans whose segment is not shown are skipped.
    func displayRanges(of spans: [SegmentSpan]) -> [NSRange] {
        spans.compactMap { span in
            guard let start = segmentOffsets[span.segmentId] else { return nil }
            return runMap.displayRange(fromCore: NSRange(location: start + span.start, length: span.length))
        }
    }

    /// B074: a span as the user store keeps it, in Unicode scalars of the segment's text.
    func storeSpan(_ span: SegmentSpan) -> StoreSegmentSpan? {
        guard let text = segmentTexts[span.segmentId] as NSString?, span.start >= 0, span.start + span.length <= text.length else { return nil }
        let start = text.substring(to: span.start).unicodeScalars.count
        let end = text.substring(to: span.start + span.length).unicodeScalars.count
        return end > start ? StoreSegmentSpan(segmentId: span.segmentId, start: UInt32(start), end: UInt32(end)) : nil
    }

    /// B074: a stored span (Unicode scalars) as UTF-16 in the segment's text; nil when the
    /// segment is not in this layout or the span runs past its text.
    func segmentSpan(_ span: StoreSegmentSpan) -> SegmentSpan? {
        guard let text = segmentTexts[span.segmentId] else { return nil }
        let scalars = text.unicodeScalars
        guard span.end > span.start, Int(span.end) <= scalars.count else { return nil }
        let lo = scalars.index(scalars.startIndex, offsetBy: Int(span.start)), hi = scalars.index(scalars.startIndex, offsetBy: Int(span.end))
        let start = text.utf16.distance(from: text.utf16.startIndex, to: lo), end = text.utf16.distance(from: text.utf16.startIndex, to: hi)
        return SegmentSpan(segmentId: span.segmentId, start: start, length: end - start)
    }

    /// The block holding a display offset (the separator before a block counts as its own).
    func block(at offset: Int) -> ReaderBlockSpan? {
        blocks.last { $0.range.location <= offset + 2 } ?? blocks.first
    }

    /// The segment a display offset shows: the verse holding it, or in a heading the block's
    /// first verse.
    func segment(at offset: Int) -> String? {
        guard display.length > 0 else { return nil }
        let index = min(max(0, offset), display.length - 1)
        if display.attribute(.openLogosHeading, at: index, effectiveRange: nil) != nil,
           let block = blocks.first(where: { NSLocationInRange(index, $0.headingRange) }) {
            return segmentOrder.first { (verseRanges[$0]?.location ?? -1) >= NSMaxRange(block.headingRange) }
        }
        if let segment = display.attribute(.openLogosSegment, at: index, effectiveRange: nil) as? String { return segment }
        if let marker = display.attribute(.openLogosMarker, at: index, effectiveRange: nil) as? String { return marker }
        return segmentOrder.last { (verseRanges[$0]?.location ?? .max) <= index } ?? segmentOrder.first
    }

    /// The segment that holds a verse: itself, or the bridge or split verse whose numbers do.
    func segment(holding segmentID: String) -> String? {
        if verseRanges[segmentID] != nil { return segmentID }
        let parts = segmentID.split(separator: ".")
        guard parts.count == 3, let verse = Int(parts[2]) else { return nil }
        let chapter = parts.prefix(2).joined(separator: ".") + "."
        return segmentOrder.first { id in
            guard id.hasPrefix(chapter) else { return false }
            let numbers = id.dropFirst(chapter.count).split(separator: "-").compactMap { Int($0.prefix { $0.isNumber }) }
            guard let low = numbers.first else { return false }
            return (low...(numbers.last ?? low)).contains(verse)
        }
    }

    /// B071: where a verse lands when its edition may not print it (specs/23 §4): the segment
    /// holding it, else the last verse of the chapter numbered at or below it, else the
    /// chapter's first; nil when the chapter is not in the window.
    func segment(landingOn segmentID: String) -> String? {
        if let held = segment(holding: segmentID) { return held }
        let parts = segmentID.split(separator: ".")
        guard parts.count == 3, let verse = Int(parts[2]) else { return nil }
        let chapter = parts.prefix(2).joined(separator: ".") + "."
        let verses = segmentOrder.filter { $0.hasPrefix(chapter) }
        return verses.last { id in
            Int(id.dropFirst(chapter.count).prefix { $0.isNumber }).map { $0 <= verse } ?? false
        } ?? verses.first
    }

    /// The display range from the first to the last of a run of segments, in document order.
    func displayRange(from first: String, to last: String) -> NSRange? {
        guard let a = verseRanges[first], let b = verseRanges[last] else { return nil }
        let lo = min(a.location, b.location), hi = max(NSMaxRange(a), NSMaxRange(b))
        return NSRange(location: lo, length: hi - lo)
    }
}

/// One chapter block laid out on its own: its display, non-core runs, markers and verse
/// ranges from its first character, and its core text from its first verse.
struct ReaderBlockPiece {
    let chapterKey: String
    let shortLabel: String
    let contentRevision: String
    let placeholder: Bool
    let display: NSAttributedString
    /// The joiner and line break that follow this block when another comes after it.
    let separator: NSAttributedString
    let nonCore: [NSRange]
    let markers: [(segmentId: String, range: NSRange)]
    let headingRange: NSRange
    let verseRanges: [String: NSRange]
    let segmentOffsets: [String: Int]
    let segmentTexts: [String: String]
    let coreText: String
    let coreLength: Int
    let segments: [Segment]
    let order: [String]
    let placeholders: Set<String>
    let verses: [Passage.Verse]

    init(chapter: ReadingChapter, editionId: String, isRightToLeft: Bool, typography: ReaderTypography, attribution: String?, joiner: String = " ") {
        let style = ChapterStyle(typography: typography, isRightToLeft: isRightToLeft)
        let key = chapter.chapterKey
        let display = NSMutableAttributedString()
        var nonCore: [NSRange] = []
        var markers: [(segmentId: String, range: NSRange)] = []
        var core = ""
        var offsets: [String: Int] = [:]
        var texts: [String: String] = [:]
        var segments: [Segment] = []
        var order: [String] = []
        var verseRanges: [String: NSRange] = [:]
        var placeholders: Set<String> = []

        func appendNonCore(_ text: String, _ attributes: [NSAttributedString.Key: Any]) {
            let range = NSRange(location: display.length, length: (text as NSString).length)
            display.append(NSAttributedString(string: text, attributes: attributes))
            nonCore.append(range)
        }

        if let title = chapter.heading.bookTitle {
            appendNonCore(title + "\n", style.bookTitle(chapter: key))
        }
        appendNonCore(chapter.heading.chapterLabel + "\n", style.chapterLabel(chapter: key))
        if chapter.placeholder {
            appendNonCore(ReadingNotice.placeholder + "\n", style.notice(chapter: key))
        }
        let headingRange = NSRange(location: 0, length: display.length)
        display.addAttribute(.openLogosHeading, value: key, range: headingRange)

        var previous: ReadingVerse?
        var previousAttributes: [NSAttributedString.Key: Any] = [:]
        for verse in chapter.verses {
            let attributes = style.verse(verse, chapter: key)
            if let previous {
                display.append(NSAttributedString(string: joiner, attributes: previousAttributes))
                core += joiner
                if verse.paragraphStart || verse.poetry || verse.superscription || previous.poetry || previous.superscription {
                    appendNonCore("\n", previousAttributes)
                }
            }
            let verseStart = display.length
            if !verse.label.isEmpty {
                var marker = style.marker(verse, attributes: attributes)
                marker[.openLogosMarker] = verse.segmentId
                // A narrow no-break space: a line never ends between a number and its verse.
                appendNonCore(verse.label + "\u{202F}", marker)
                markers.append((segmentId: verse.segmentId, range: NSRange(location: verseStart, length: display.length - verseStart)))
            }
            var text = attributes
            text[.openLogosSegment] = verse.segmentId
            display.append(NSAttributedString(string: verse.text, attributes: text))
            verseRanges[verse.segmentId] = NSRange(location: verseStart, length: display.length - verseStart)
            order.append(verse.segmentId)
            if verse.placeholder { placeholders.insert(verse.segmentId) }
            offsets[verse.segmentId] = (core as NSString).length
            texts[verse.segmentId] = verse.text
            core += verse.text
            let tokens = verse.tokens.isEmpty
                ? ReaderLayout.mintedTokens(edition: editionId, segmentId: verse.segmentId, text: verse.text).map { TokenSpan(id: $0.id, range: ScalarRange(start: $0.start, end: $0.end)) }
                : verse.tokens.map { TokenSpan(id: $0.id, range: ScalarRange(start: $0.start, end: $0.end)) }
            segments.append(Segment(segmentId: verse.segmentId, text: verse.text, tokens: tokens))
            previous = verse
            previousAttributes = attributes
        }
        if let attribution, !attribution.isEmpty {
            appendNonCore("\n" + attribution, style.attribution(chapter: key))
        }
        let separator = NSMutableAttributedString(string: joiner + ReaderLayout.blockBreak, attributes: style.body(chapter: key))

        chapterKey = key
        shortLabel = chapter.heading.shortLabel
        contentRevision = chapter.contentRevision
        placeholder = chapter.placeholder
        self.display = display
        self.separator = separator
        self.nonCore = nonCore
        self.markers = markers
        self.headingRange = headingRange
        self.verseRanges = verseRanges
        segmentOffsets = offsets
        segmentTexts = texts
        coreText = core
        coreLength = (core as NSString).length
        self.segments = segments
        self.order = order
        self.placeholders = placeholders
        verses = chapter.verses.map { Passage.Verse(id: $0.segmentId, number: $0.label, text: $0.text, tokens: $0.tokens.map { Passage.Token(id: $0.id, start: $0.start, end: $0.end) }) }
    }
}

/// The attributes of a block's runs, from the reader's typography.
private struct ChapterStyle {
    let typography: ReaderTypography
    let isRightToLeft: Bool
    /// The three verse styles, made once per block.
    private let prose: NSParagraphStyle
    private let poetryStyle: NSParagraphStyle
    private let titleStyle: NSParagraphStyle
    private let titleFont: PlatformFont

    init(typography: ReaderTypography, isRightToLeft: Bool) {
        self.typography = typography
        self.isRightToLeft = isRightToLeft
        let size = typography.body.pointSize
        prose = Self.paragraph(typography, isRightToLeft)
        poetryStyle = Self.paragraph(typography, isRightToLeft, indent: (size * 1.5).rounded(), after: (typography.lineSpacing * 0.3).rounded())
        titleStyle = Self.paragraph(typography, isRightToLeft, after: (size * 0.4).rounded())
        titleFont = ReaderTypography.serif(size: size, italic: true)
    }

    private static func paragraph(_ typography: ReaderTypography, _ isRightToLeft: Bool, indent: CGFloat = 0, before: CGFloat = 0, after: CGFloat? = nil) -> NSParagraphStyle {
        let style = NSMutableParagraphStyle()
        style.lineSpacing = typography.lineSpacing
        style.paragraphSpacing = after ?? typography.lineSpacing * 1.5
        style.paragraphSpacingBefore = before
        style.firstLineHeadIndent = indent
        style.headIndent = indent
        style.alignment = .natural
        style.baseWritingDirection = isRightToLeft ? .rightToLeft : .leftToRight
        return style
    }

    private func paragraph(indent: CGFloat = 0, before: CGFloat = 0, after: CGFloat? = nil) -> NSParagraphStyle {
        Self.paragraph(typography, isRightToLeft, indent: indent, before: before, after: after)
    }

    private var size: CGFloat { typography.body.pointSize }

    func body(chapter: String) -> [NSAttributedString.Key: Any] {
        [.font: typography.body, .foregroundColor: typography.text, .paragraphStyle: prose, .openLogosChapter: chapter]
    }

    func bookTitle(chapter: String) -> [NSAttributedString.Key: Any] {
        [.font: ReaderTypography.serif(size: (size * 1.6).rounded(), weight: .semibold), .foregroundColor: typography.text,
         .paragraphStyle: paragraph(before: (size * 1.6).rounded(), after: (size * 0.3).rounded()), .openLogosChapter: chapter]
    }

    func chapterLabel(chapter: String) -> [NSAttributedString.Key: Any] {
        [.font: ReaderTypography.serif(size: (size * 0.8).rounded(), weight: .semibold), .foregroundColor: typography.secondary,
         .kern: 0.6, .paragraphStyle: paragraph(before: (size * 1.2).rounded(), after: (size * 0.5).rounded()), .openLogosChapter: chapter]
    }

    func notice(chapter: String) -> [NSAttributedString.Key: Any] {
        [.font: ReaderTypography.serif(size: typography.caption.pointSize, italic: true), .foregroundColor: typography.secondary,
         .paragraphStyle: paragraph(after: (size * 0.6).rounded()), .openLogosChapter: chapter]
    }

    func attribution(chapter: String) -> [NSAttributedString.Key: Any] {
        [.font: typography.caption, .foregroundColor: typography.secondary, .paragraphStyle: paragraph(before: (size * 0.4).rounded()), .openLogosChapter: chapter]
    }

    func verse(_ verse: ReadingVerse, chapter: String) -> [NSAttributedString.Key: Any] {
        let color = verse.placeholder ? typography.secondary : typography.text
        if verse.superscription {
            return [.font: titleFont, .foregroundColor: typography.secondary, .paragraphStyle: titleStyle, .openLogosChapter: chapter]
        }
        return [.font: typography.body, .foregroundColor: color, .paragraphStyle: verse.poetry ? poetryStyle : prose, .openLogosChapter: chapter]
    }

    /// A verse number in its verse's paragraph.
    func marker(_ verse: ReadingVerse, attributes: [NSAttributedString.Key: Any]) -> [NSAttributedString.Key: Any] {
        var marker = attributes
        marker[.font] = typography.marker
        marker[.foregroundColor] = typography.secondary
        marker[.baselineOffset] = typography.markerBaselineOffset
        return marker
    }
}
