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
        let verses = chapters.flatMap { chapter in
            chapter.verses.map { Passage.Verse(id: $0.segmentId, number: $0.label, text: $0.text, tokens: $0.tokens.map { Passage.Token(id: $0.id, start: $0.start, end: $0.end) }) }
        }
        var content = ReaderContent(id: id, editionId: editionId, contentRevision: chapters.first?.contentRevision ?? "",
                                    verses: verses, isRightToLeft: isRightToLeft, attribution: "")
        content.paragraphEvery = nil
        self.content = content
        let style = ChapterStyle(typography: typography, isRightToLeft: isRightToLeft)

        let display = NSMutableAttributedString()
        var nonCore: [NSRange] = []
        var markers: [(String, NSRange)] = []
        var core = ""
        var offsets: [String: Int] = [:]
        var texts: [String: String] = [:]
        var segments: [Segment] = []
        var blocks: [ReaderBlockSpan] = []
        var order: [String] = []
        var verseRanges: [String: NSRange] = [:]
        var placeholders: Set<String> = []

        func appendNonCore(_ text: String, _ attributes: [NSAttributedString.Key: Any]) {
            let range = NSRange(location: display.length, length: (text as NSString).length)
            display.append(NSAttributedString(string: text, attributes: attributes))
            nonCore.append(range)
        }

        for (blockIndex, chapter) in chapters.enumerated() {
            if blockIndex > 0 {
                display.append(NSAttributedString(string: content.joiner, attributes: style.body(chapter: chapters[blockIndex - 1].chapterKey)))
                core += content.joiner
                appendNonCore(Self.blockBreak, style.body(chapter: chapters[blockIndex - 1].chapterKey))
            }
            let blockStart = display.length
            let key = chapter.chapterKey
            if let title = chapter.heading.bookTitle {
                appendNonCore(title + "\n", style.bookTitle(chapter: key))
            }
            appendNonCore(chapter.heading.chapterLabel + "\n", style.chapterLabel(chapter: key))
            if chapter.placeholder {
                appendNonCore(ReadingNotice.placeholder + "\n", style.notice(chapter: key))
            }
            let headingRange = NSRange(location: blockStart, length: display.length - blockStart)
            display.addAttribute(.openLogosHeading, value: key, range: headingRange)

            var previous: ReadingVerse?
            for verse in chapter.verses {
                let attributes = style.verse(verse, chapter: key)
                if let previous {
                    display.append(NSAttributedString(string: content.joiner, attributes: style.verse(previous, chapter: key)))
                    core += content.joiner
                    if verse.paragraphStart || verse.poetry || verse.superscription || previous.poetry || previous.superscription {
                        appendNonCore("\n", style.verse(previous, chapter: key))
                    }
                }
                let verseStart = display.length
                if !verse.label.isEmpty {
                    var marker = style.marker(verse, chapter: key)
                    marker[.openLogosMarker] = verse.segmentId
                    // A narrow no-break space: a line never ends between a number and its verse.
                    appendNonCore(verse.label + "\u{202F}", marker)
                    markers.append((verse.segmentId, NSRange(location: verseStart, length: display.length - verseStart)))
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
            }
            if let line = attribution(chapter), !line.isEmpty {
                appendNonCore("\n" + line, style.attribution(chapter: key))
            }
            blocks.append(ReaderBlockSpan(chapterKey: key, shortLabel: chapter.heading.shortLabel,
                                          range: NSRange(location: blockStart, length: display.length - blockStart),
                                          headingRange: headingRange, placeholder: chapter.placeholder))
        }

        self.display = display
        runMap = RunMap(nonCore: nonCore)
        paragraph = Paragraph(editionId: editionId, contentRevision: content.contentRevision, joiner: content.joiner, segments: segments)
        coreText = core
        segmentOffsets = offsets
        segmentTexts = texts
        self.markers = markers.map { (segmentId: $0.0, range: $0.1) }
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

    /// The display range from the first to the last of a run of segments, in document order.
    func displayRange(from first: String, to last: String) -> NSRange? {
        guard let a = verseRanges[first], let b = verseRanges[last] else { return nil }
        let lo = min(a.location, b.location), hi = max(NSMaxRange(a), NSMaxRange(b))
        return NSRange(location: lo, length: hi - lo)
    }
}

/// The attributes of a block's runs, from the reader's typography.
private struct ChapterStyle {
    let typography: ReaderTypography
    let isRightToLeft: Bool

    private func paragraph(indent: CGFloat = 0, before: CGFloat = 0, after: CGFloat? = nil) -> NSParagraphStyle {
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

    private var size: CGFloat { typography.body.pointSize }

    func body(chapter: String) -> [NSAttributedString.Key: Any] {
        [.font: typography.body, .foregroundColor: typography.text, .paragraphStyle: paragraph(), .openLogosChapter: chapter]
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
            return [.font: ReaderTypography.serif(size: size, italic: true), .foregroundColor: typography.secondary,
                    .paragraphStyle: paragraph(after: (size * 0.4).rounded()), .openLogosChapter: chapter]
        }
        if verse.poetry {
            return [.font: typography.body, .foregroundColor: color,
                    .paragraphStyle: paragraph(indent: (size * 1.5).rounded(), after: (typography.lineSpacing * 0.3).rounded()), .openLogosChapter: chapter]
        }
        return [.font: typography.body, .foregroundColor: color, .paragraphStyle: paragraph(), .openLogosChapter: chapter]
    }

    func marker(_ verse: ReadingVerse, chapter: String) -> [NSAttributedString.Key: Any] {
        var attributes = self.verse(verse, chapter: chapter)
        attributes[.font] = typography.marker
        attributes[.foregroundColor] = typography.secondary
        attributes[.baselineOffset] = typography.markerBaselineOffset
        return attributes
    }
}
