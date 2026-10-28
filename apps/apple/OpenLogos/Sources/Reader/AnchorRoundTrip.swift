// The round trip B005 proves (docs/adr/0001-rust-core-native-ui.md): a UI selection, as the
// text view reports it in UTF-16 units of the display string, becomes core anchors through the
// UniFFI surface (`selectionToAnchors`) and comes back as UTF-16 ranges (`anchorToUtf16`) that
// the surface paints. The surface never rounds a selection to a verse; it does widen a range
// that ends inside a grapheme cluster (between a Hebrew letter and its marks, or between an
// emoji and its skin-tone modifier) outwards to the cluster, with the platform's own notion of
// composed character sequences, and says so. A selection is identical when the painted text,
// the anchors' quoted text and the selected core text agree character for character.
import Foundation
import os

/// What one selection became.
struct ReaderSelection: Equatable {
    let paneID: String
    /// The display range the surface used, after any widening to grapheme clusters.
    let displayRange: NSRange
    /// Whether the text view's range had to be widened to cluster boundaries.
    let snapped: Bool
    /// The range the core measured, in UTF-16 units of the joined core text.
    let coreRange: NSRange
    let anchors: [Anchor]
    /// The display ranges painted back, one per anchor, from `anchorToUtf16`.
    let highlights: [NSRange]
    /// The anchors' exact texts, joined by the content's joiner.
    let text: String
    /// Painted text == quoted text == selected core text, segment by segment.
    let identical: Bool
    /// Wall-clock time of the two core calls and the offset mapping, in milliseconds.
    let elapsedMilliseconds: Double

    /// Segment ids in order, "John.3.16–17" style when they share a chapter.
    var reference: String {
        let ids = anchors.map(\.segmentId)
        guard let first = ids.first, let last = ids.last, first != last else { return ids.first ?? "" }
        let head = first.split(separator: ".").dropLast().joined(separator: ".")
        if last.hasPrefix(head + ".") {
            return "\(first)–\(last.split(separator: ".").last ?? "")"
        }
        return "\(first)–\(last)"
    }

    /// One line for the readout and the evidence: anchors with their scalar ranges and tokens.
    var summary: String {
        let parts = anchors.map { anchor -> String in
            let tokens = anchor.tokens.isEmpty ? "no tokens" : "\(anchor.tokens.count) token\(anchor.tokens.count == 1 ? "" : "s") \(anchor.tokens.first ?? "")\(anchor.tokens.count > 1 ? "…\(anchor.tokens.last?.suffix(3) ?? "")" : "")"
            return "\(anchor.segmentId) [\(anchor.range.start),\(anchor.range.end)) “\(anchor.quote?.exact ?? "")” \(tokens)"
        }
        let verdict = identical ? "round trip identical" : "ROUND TRIP DIFFERS"
        let snap = snapped ? ", widened to grapheme clusters" : ""
        return "\(parts.joined(separator: " · ")) — \(verdict)\(snap), \(String(format: "%.2f", elapsedMilliseconds)) ms"
    }

    /// The JSON line the evidence log records.
    var logLine: [String: Any] {
        [
            "pane": paneID,
            "display": [displayRange.location, displayRange.location + displayRange.length],
            "snapped": snapped,
            "core": [coreRange.location, coreRange.location + coreRange.length],
            "anchors": anchors.map { anchor -> [String: Any] in
                [
                    "segment_id": anchor.segmentId,
                    "edition_id": anchor.editionId,
                    "content_revision": anchor.contentRevision,
                    "range": [anchor.range.start, anchor.range.end],
                    "exact": anchor.quote?.exact ?? "",
                    "tokens": anchor.tokens,
                ]
            },
            "highlights": highlights.map { [$0.location, $0.location + $0.length] },
            "identical": identical,
            "elapsed_ms": elapsedMilliseconds,
        ]
    }
}

enum AnchorRoundTrip {
    static let signposter = OSSignposter(subsystem: "org.openlogos.app", category: "reader")

    /// A display selection through the core and back. Nil for a caret or a selection that
    /// covers no core text (a verse number alone); throws what the core throws.
    static func run(layout: ReaderLayout, paneID: String, displayRange: NSRange) throws -> ReaderSelection? {
        let string = layout.display.string as NSString
        guard displayRange.length > 0, displayRange.location + displayRange.length <= string.length else { return nil }
        let state = signposter.beginInterval("selection-roundtrip")
        let started = CFAbsoluteTimeGetCurrent()

        // The platform's grapheme clusters (composed character sequences) bound the selection.
        let snapped = string.rangeOfComposedCharacterSequences(for: displayRange)
        let core = layout.runMap.coreRange(fromDisplay: snapped)
        guard core.length > 0 else {
            signposter.endInterval("selection-roundtrip", state)
            return nil
        }

        let anchors = try selectionToAnchors(
            paragraph: layout.paragraph,
            selection: Utf16Range(start: UInt32(core.location), end: UInt32(core.location + core.length))
        )
        var highlights: [NSRange] = []
        var painted: [String] = []
        for anchor in anchors {
            guard let text = layout.segmentTexts[anchor.segmentId], let offset = layout.segmentOffsets[anchor.segmentId] else { continue }
            let back = try anchorToUtf16(anchor: anchor, text: text)
            let coreRange = NSRange(location: offset + Int(back.start), length: Int(back.end - back.start))
            let display = layout.runMap.displayRange(fromCore: coreRange)
            highlights.append(display)
            painted.append(string.substring(with: display))
        }
        let elapsed = (CFAbsoluteTimeGetCurrent() - started) * 1000
        signposter.endInterval("selection-roundtrip", state)

        // The selected core text, segment by segment (the joiner belongs to no segment).
        let coreString = layout.coreText as NSString
        var selectedPerSegment: [String] = []
        for segment in layout.paragraph.segments {
            guard let offset = layout.segmentOffsets[segment.segmentId] else { continue }
            let segmentRange = NSRange(location: offset, length: (segment.text as NSString).length)
            let overlap = NSIntersectionRange(segmentRange, core)
            if overlap.length > 0 { selectedPerSegment.append(coreString.substring(with: overlap)) }
        }
        let quoted = anchors.map { $0.quote?.exact ?? "" }
        let identical = quoted == painted && quoted == selectedPerSegment

        let selection = ReaderSelection(
            paneID: paneID,
            displayRange: snapped,
            snapped: snapped != displayRange,
            coreRange: core,
            anchors: anchors,
            highlights: highlights,
            text: quoted.joined(separator: layout.content.joiner),
            identical: identical,
            elapsedMilliseconds: elapsed
        )
        RoundTripLog.append(selection.logLine)
        return selection
    }
}

/// One JSON line per round trip in b005-roundtrips.jsonl, when the app was launched with
/// `-OPENLOGOS_LOG 1`, so the evidence can quote what the surface did. The file lives in the
/// app container's Documents on iOS (scripts/b005-shots.sh ipad-log reads it) and in the
/// directory `-OPENLOGOS_LOG_DIR` names on the Mac, so nothing is written into the user's own
/// Documents folder there.
enum RoundTripLog {
    static let enabled = UserDefaults.standard.bool(forKey: "OPENLOGOS_LOG")

    static var url: URL? {
        if let directory = UserDefaults.standard.string(forKey: "OPENLOGOS_LOG_DIR") {
            try? FileManager.default.createDirectory(atPath: directory, withIntermediateDirectories: true)
            return URL(fileURLWithPath: directory).appendingPathComponent("b005-roundtrips.jsonl")
        }
        #if os(macOS)
        return nil
        #else
        return FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first?
            .appendingPathComponent("b005-roundtrips.jsonl")
        #endif
    }

    static func append(_ line: [String: Any], to name: String? = nil) {
        guard enabled, var url else { return }
        if let name { url = url.deletingLastPathComponent().appendingPathComponent(name) }
        guard let data = try? JSONSerialization.data(withJSONObject: line, options: [.sortedKeys, .withoutEscapingSlashes]) else { return }
        if let handle = try? FileHandle(forWritingTo: url) {
            handle.seekToEndOfFile()
            handle.write(data)
            handle.write(Data("\n".utf8))
            try? handle.close()
        } else {
            try? (data + Data("\n".utf8)).write(to: url)
        }
    }
}
