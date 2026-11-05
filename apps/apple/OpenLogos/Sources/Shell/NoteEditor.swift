// B076: notes are editable in place. B074: the core keeps them (`note(id:)`, `putNote`), in
// the stored block form (specs/24 §2.1); this file converts at the edge. A notes tab and the
// notes dock show an NSTextView over the note's rich text: Type is the notes context's tool,
// and the well's Bold, Italic, Heading, List and Link passage act on the editor that last
// had focus (⌘B and ⌘I only while it has it). A passage link is accent-coloured, underlined
// text that opens the passage in a reading column when clicked. Note on it (a selection
// action) appends the quoted text and its passage as its own paragraph. Fonts are derived
// from the reading typography and each run's traits, so a size or theme change restyles the
// note without losing bold, italic or headings.
import SwiftUI
#if canImport(AppKit)
import AppKit

extension NSAttributedString.Key {
    /// A paragraph styled as a heading.
    static let openLogosNoteHeading = NSAttributedString.Key("org.openlogos.note.heading")
    /// A quoted selection (Note on it): indented, with a rule on the leading edge.
    static let openLogosNoteQuote = NSAttributedString.Key("org.openlogos.note.quote")
}

/// The notes being edited, by note id, and the editors showing them.
@MainActor
final class NoteStore {
    enum Trait { case bold, italic }

    weak var model: ShellModel?
    /// Each note's text as last edited (fonts restyled on display).
    private(set) var documents: [String: NSAttributedString] = [:]
    private var editors: [String: WeakEditor] = [:]
    /// The editor the well's note actions act on: the one that last had focus.
    private(set) weak var active: NoteTextView?

    static let linkScheme = "openlogos-passage"

    /// Whether a note editor has keyboard focus now.
    var editing: Bool {
        guard let active else { return false }
        return active.window?.firstResponder === active
    }

    /// The note's text: as last edited, else the core's note, else the seed text (saved on its
    /// first edit).
    func document(_ noteID: String) -> NSAttributedString {
        if let document = documents[noteID] { return document }
        let document: NSAttributedString
        if let stored = model?.handle.note(id: noteID) {
            document = Self.attributed(stored)
        } else {
            let paragraphs = model?.textSource.text(resourceID: noteID, reference: nil)?.content.verses.map(\.text) ?? []
            document = NSAttributedString(string: paragraphs.joined(separator: "\n"))
        }
        documents[noteID] = document
        return document
    }

    // MARK: Saving into the core (B074)

    /// Notes edited since they were last put, saved when typing pauses (300 ms) or the
    /// editor loses focus.
    private var pending: Set<String> = []
    private var putTimer: Timer?
    static let putDelay: TimeInterval = 0.3

    private func schedulePut(_ noteID: String) {
        pending.insert(noteID)
        putTimer?.invalidate()
        putTimer = Timer.scheduledTimer(withTimeInterval: Self.putDelay, repeats: false) { [weak self] _ in
            MainActor.assumeIsolated { self?.flush() }
        }
    }

    /// Puts every pending note into the core now.
    func flush() {
        putTimer?.invalidate()
        putTimer = nil
        guard !pending.isEmpty, let model else { return }
        let ids = pending
        pending = []
        for id in ids.sorted() {
            guard let document = documents[id] else { continue }
            do {
                try model.handle.putNote(note: Self.storeNote(id: id, from: document))
            } catch {
                model.show(toast: "\(error)")
            }
        }
        model.storeChanged()
    }

    var hasPending: Bool { !pending.isEmpty }

    // MARK: Editors

    func register(_ editor: NoteTextView) {
        editors[editor.paneID] = WeakEditor(editor: editor)
    }

    func focused(_ editor: NoteTextView) {
        active = editor
    }

    /// An editor's text changed: it becomes the note's text, and the note's other editors
    /// show it.
    func didEdit(_ editor: NoteTextView) {
        guard let storage = editor.textStorage else { return }
        documents[editor.noteID] = NSAttributedString(attributedString: storage)
        schedulePut(editor.noteID)
        for other in editors.values.compactMap(\.editor) where other !== editor && other.noteID == editor.noteID {
            other.load(documents[editor.noteID]!)
        }
    }

    /// The editor of a pane (a notes tab's id, or `dock`).
    func editor(pane: String) -> NoteTextView? { editors[pane]?.editor }

    /// An editor on screen showing the note (a released workspace's editor may linger).
    private func editor(for noteID: String) -> NoteTextView? {
        if let active, active.noteID == noteID, active.window != nil { return active }
        return editors.values.compactMap(\.editor).first { $0.noteID == noteID && $0.window != nil }
    }

    // MARK: Actions

    /// Bold or Italic: on the selection, or for what is typed next.
    func toggle(_ trait: Trait) {
        guard let view = active, let storage = view.textStorage else { return }
        let mask: NSFontDescriptor.SymbolicTraits = trait == .bold ? .bold : .italic
        let range = view.selectedRange()
        if range.length == 0 {
            let font = (view.typingAttributes[.font] as? NSFont) ?? view.typography.body
            view.typingAttributes[.font] = Self.font(font, toggling: mask, on: !font.fontDescriptor.symbolicTraits.contains(mask))
            return
        }
        let first = (storage.attribute(.font, at: range.location, effectiveRange: nil) as? NSFont) ?? view.typography.body
        let on = !first.fontDescriptor.symbolicTraits.contains(mask)
        view.edit(range) {
            storage.enumerateAttribute(.font, in: range) { value, run, _ in
                let font = (value as? NSFont) ?? view.typography.body
                storage.addAttribute(.font, value: Self.font(font, toggling: mask, on: on), range: run)
            }
        }
    }

    /// Heading: the selected paragraphs become a heading, or body text again.
    func toggleHeading() {
        guard let view = active, let storage = view.textStorage else { return }
        let paragraphs = (storage.string as NSString).paragraphRange(for: view.selectedRange())
        let on = paragraphs.location >= storage.length || storage.attribute(.openLogosNoteHeading, at: paragraphs.location, effectiveRange: nil) == nil
        view.edit(paragraphs) {
            if on {
                storage.addAttribute(.openLogosNoteHeading, value: true, range: paragraphs)
            } else {
                storage.removeAttribute(.openLogosNoteHeading, range: paragraphs)
            }
            view.restyle(paragraphs)
        }
        if on { view.typingAttributes[.openLogosNoteHeading] = true } else { view.typingAttributes[.openLogosNoteHeading] = nil }
    }

    /// List: each selected paragraph gains a `• ` bullet, or loses it.
    func toggleList() {
        guard let view = active, let storage = view.textStorage else { return }
        let text = storage.string as NSString
        let whole = text.paragraphRange(for: view.selectedRange())
        var starts: [Int] = []
        text.enumerateSubstrings(in: whole, options: [.byParagraphs, .substringNotRequired]) { _, range, _, _ in starts.append(range.location) }
        if starts.isEmpty { starts = [whole.location] }
        let bulleted = starts.allSatisfy { text.length >= $0 + 2 && text.substring(with: NSRange(location: $0, length: 2)) == "• " }
        view.edit(whole) {
            for start in starts.reversed() {
                if bulleted {
                    storage.deleteCharacters(in: NSRange(location: start, length: 2))
                } else if !(text.length >= start + 2 && storage.string.dropFirst(start).hasPrefix("• ")) {
                    storage.insert(NSAttributedString(string: "• ", attributes: view.typingAttributes), at: start)
                }
            }
        }
    }

    /// Link passage: the selected words link to the passage, or its name is inserted as a link.
    func insertLink(to reference: String) {
        guard let view = active, let storage = view.textStorage, let url = Self.url(reference) else { return }
        let range = view.selectedRange()
        if range.length > 0 {
            view.edit(range) { storage.addAttribute(.link, value: url, range: range) }
        } else {
            var attributes = view.typingAttributes
            attributes[.link] = url
            let link = NSMutableAttributedString(string: ShellReference.humanRange(reference), attributes: attributes)
            attributes[.link] = nil
            link.append(NSAttributedString(string: " ", attributes: attributes))
            view.edit(range, replacement: link)
        }
    }

    /// Note on it: `“quoted text” — John 3:16` as a new quote paragraph at the end of the
    /// note (indented 12 with a 2 pt rule on the leading edge), the passage a link.
    func appendQuote(_ text: String, reference: String, to noteID: String) {
        let quote = NSMutableAttributedString(string: "“\(text)” — ", attributes: [.openLogosNoteQuote: true])
        let first = reference.split(separator: "–").first.map(String.init) ?? reference
        var link: [NSAttributedString.Key: Any] = [.openLogosNoteQuote: true]
        link[.link] = Self.url(first)
        quote.append(NSAttributedString(string: ShellReference.humanRange(reference), attributes: link))
        if let view = editor(for: noteID), let storage = view.textStorage {
            let end = NSRange(location: storage.length, length: 0)
            let paragraph = NSMutableAttributedString(string: storage.length == 0 || storage.string.hasSuffix("\n") ? "" : "\n")
            paragraph.append(quote)
            view.edit(end, replacement: paragraph)
            view.restyle(NSRange(location: end.location, length: paragraph.length))
            view.scrollToEndOfDocument(nil)
        } else {
            let document = NSMutableAttributedString(attributedString: document(noteID))
            if document.length > 0 { document.append(NSAttributedString(string: "\n")) }
            document.append(quote)
            documents[noteID] = document
            schedulePut(noteID)
        }
    }

    // MARK: The stored form (specs/24 §2.1)

    /// A stored note as editor text: a paragraph per block, a heading or quote mark on the
    /// paragraph (its line break included), a `• ` bullet for a list item, bold and italic as
    /// font traits (restyled to the reading size when shown) and passage links.
    static func attributed(_ note: StoreNote) -> NSAttributedString {
        let text = NSMutableAttributedString()
        let base = NSFont.systemFont(ofSize: 13)
        for (index, block) in note.blocks.enumerated() {
            var paragraph: [NSAttributedString.Key: Any] = [:]
            switch block.kind {
            case .heading: paragraph[.openLogosNoteHeading] = true
            case .quote: paragraph[.openLogosNoteQuote] = true
            case .listItem, .paragraph: break
            }
            let start = text.length
            if block.kind == .listItem { text.append(NSAttributedString(string: "• ", attributes: paragraph)) }
            for span in block.spans where !span.text.isEmpty {
                var attributes = paragraph
                var traits: NSFontDescriptor.SymbolicTraits = []
                if span.bold { traits.insert(.bold) }
                if span.italic { traits.insert(.italic) }
                if !traits.isEmpty { attributes[.font] = NSFont(descriptor: base.fontDescriptor.withSymbolicTraits(traits), size: base.pointSize) ?? base }
                if let link = span.link { attributes[.link] = url(link) }
                text.append(NSAttributedString(string: span.text, attributes: attributes))
            }
            if index < note.blocks.count - 1 { text.append(NSAttributedString(string: "\n", attributes: paragraph)) }
            if text.length > start, !paragraph.isEmpty { text.addAttributes(paragraph, range: NSRange(location: start, length: text.length - start)) }
        }
        return text
    }

    /// Editor text as a stored note: each paragraph a block (heading, quote, `• ` list item or
    /// paragraph) and its runs as spans, adjacent runs with the same marks merged. Anything
    /// else the text holds (pasted colours, fonts) is not stored.
    static func storeNote(id: String, from text: NSAttributedString) -> StoreNote {
        let string = text.string as NSString
        var blocks: [StoreNoteBlock] = []
        var location = 0
        repeat {
            let line = string.range(of: "\n", range: NSRange(location: location, length: string.length - location))
            let end = line.location == NSNotFound ? string.length : line.location
            var content = NSRange(location: location, length: end - location)
            // An empty paragraph's marks are on its line break; the last one, with none, is a
            // plain paragraph.
            let probe = content.length > 0 ? content.location : (line.location != NSNotFound ? line.location : text.length)
            let marked = { (key: NSAttributedString.Key) in probe < text.length && text.attribute(key, at: probe, effectiveRange: nil) != nil }
            var kind = StoreNoteBlockKind.paragraph
            if marked(.openLogosNoteHeading) {
                kind = .heading
            } else if marked(.openLogosNoteQuote) {
                kind = .quote
            } else if string.substring(with: content).hasPrefix("• ") {
                kind = .listItem
                content = NSRange(location: content.location + 2, length: content.length - 2)
            }
            blocks.append(StoreNoteBlock(kind: kind, spans: spans(of: text, in: content, heading: kind == .heading)))
            location = end + 1
        } while location <= string.length
        return StoreNote(id: id, blocks: blocks)
    }

    private static func spans(of text: NSAttributedString, in range: NSRange, heading: Bool) -> [StoreNoteSpan] {
        var spans: [StoreNoteSpan] = []
        guard range.length > 0 else { return spans }
        text.enumerateAttributes(in: range) { attributes, run, _ in
            let font = attributes[.font] as? NSFont
            let traits = font?.fontDescriptor.symbolicTraits ?? []
            // A heading's own semibold is not a bold mark: only a heavier run is.
            let bold = heading ? heavierThanHeading(font) : traits.contains(.bold)
            let span = StoreNoteSpan(text: (text.string as NSString).substring(with: run), bold: bold, italic: traits.contains(.italic),
                                     link: attributes[.link].flatMap(reference))
            if let last = spans.last, last.bold == span.bold, last.italic == span.italic, last.link == span.link {
                spans[spans.count - 1].text += span.text
            } else {
                spans.append(span)
            }
        }
        return spans
    }

    private static func heavierThanHeading(_ font: NSFont?) -> Bool {
        guard let font else { return false }
        guard let weight = (font.fontDescriptor.object(forKey: .traits) as? [NSFontDescriptor.TraitKey: Any])?[.weight] as? CGFloat else {
            return font.fontDescriptor.symbolicTraits.contains(.bold)
        }
        return weight > NSFont.Weight.semibold.rawValue + 0.01
    }

    // MARK: Helpers

    static func url(_ reference: String) -> URL? { URL(string: "\(linkScheme):\(reference)") }

    static func reference(_ link: Any) -> String? {
        let url = (link as? URL) ?? (link as? String).flatMap(URL.init(string:))
        guard let url, url.scheme == linkScheme else { return nil }
        return url.absoluteString.dropFirst(linkScheme.count + 1).removingPercentEncoding
    }

    static func font(_ font: NSFont, toggling trait: NSFontDescriptor.SymbolicTraits, on: Bool) -> NSFont {
        var traits = font.fontDescriptor.symbolicTraits
        if on { traits.insert(trait) } else { traits.remove(trait) }
        return NSFont(descriptor: font.fontDescriptor.withSymbolicTraits(traits), size: font.pointSize) ?? font
    }

    private struct WeakEditor { weak var editor: NoteTextView? }
}

/// The fonts and colours a note is drawn in, from the shell's reading typography.
struct NoteTypography: Equatable {
    let size: CGFloat
    let lineSpacing: CGFloat
    let text: UInt32
    let accent: UInt32
    var rules: UInt32 = 0xE4E1DA

    var body: NSFont { Self.serif(size, weight: .regular) }
    var heading: NSFont { Self.serif((size * 1.25).rounded(), weight: .semibold) }

    /// The quote paragraph's indent and the list's hanging indent.
    static let quoteIndent: CGFloat = 12
    var bulletIndent: CGFloat { ("• " as NSString).size(withAttributes: [.font: body]).width }

    static func serif(_ size: CGFloat, weight: NSFont.Weight) -> NSFont {
        let base = NSFont.systemFont(ofSize: size, weight: weight)
        return base.fontDescriptor.withDesign(.serif).flatMap { NSFont(descriptor: $0, size: size) } ?? base
    }
}

/// The note editor's text view: it tells the store when it takes focus.
final class NoteTextView: NSTextView {
    var paneID = ""
    var noteID = ""
    weak var store: NoteStore?
    var onFocus: () -> Void = {}
    var typography = NoteTypography(size: 19, lineSpacing: 1.5, text: 0x1C1B19, accent: 0x2F5BD3)

    override func becomeFirstResponder() -> Bool {
        let accepted = super.becomeFirstResponder()
        if accepted {
            store?.focused(self)
            onFocus()
        }
        return accepted
    }

    /// B074: a note leaving focus is saved into the core now.
    override func resignFirstResponder() -> Bool {
        let resigned = super.resignFirstResponder()
        if resigned { store?.flush() }
        return resigned
    }

    override func mouseDown(with event: NSEvent) {
        onFocus()
        super.mouseDown(with: event)
    }

    // MARK: B077 context menu

    /// The OpenLogos menu for a passage link or the note's text; ⌥-right-click keeps the
    /// platform's (spelling, Look Up, Services).
    override func menu(for event: NSEvent) -> NSMenu? {
        guard let model = store?.model, !event.modifierFlags.contains(.option) else { return super.menu(for: event) }
        if window?.firstResponder !== self { window?.makeFirstResponder(self) }
        onFocus()
        return model.nsMenu(for: hit(at: characterIndexForInsertion(at: convert(event.locationInWindow, from: nil))), editor: self)
    }

    /// A passage link at the index (or just before it), else the selected text, if any.
    func hit(at index: Int) -> NoteContextHit {
        if let storage = textStorage, storage.length > 0 {
            for candidate in [index, index - 1] where candidate >= 0 && candidate < storage.length {
                var range = NSRange(location: 0, length: 0)
                if let link = storage.attribute(.link, at: candidate, effectiveRange: &range), let reference = NoteStore.reference(link) {
                    return .link(reference, range)
                }
            }
        }
        let selected = selectedRange()
        return .text(selected.length > 0 ? (string as NSString).substring(with: selected) : nil)
    }

    /// The menu key or VO-⇧-M: the menu for the caret's link or the selection, at the caret.
    func showKeyboardMenu() -> Bool {
        guard let model = store?.model else { return false }
        let selected = selectedRange()
        let hit = selected.length > 0 ? NoteContextHit.text((string as NSString).substring(with: selected)) : hit(at: selected.location)
        guard let menu = model.nsMenu(for: hit, editor: self) else { return false }
        let caret = firstRect(forCharacterRange: NSRange(location: selected.location, length: 0), actualRange: nil)
        let point = window.map { convert($0.convertPoint(fromScreen: caret.origin), from: nil) } ?? .zero
        menu.popUp(positioning: nil, at: NSPoint(x: point.x, y: point.y + (isFlipped ? 4 : -4)), in: self)
        return true
    }

    override func accessibilityPerformShowMenu() -> Bool {
        showKeyboardMenu() || super.accessibilityPerformShowMenu()
    }

    /// Replaces the text with a document, restyled.
    func load(_ document: NSAttributedString) {
        guard let storage = textStorage else { return }
        let selection = selectedRange()
        storage.setAttributedString(document)
        restyle(NSRange(location: 0, length: storage.length))
        setSelectedRange(NSRange(location: min(selection.location, storage.length), length: 0))
    }

    /// A change the undo stack and the store hear about.
    func edit(_ range: NSRange, replacement: NSAttributedString? = nil, _ change: () -> Void = {}) {
        guard let storage = textStorage, shouldChangeText(in: range, replacementString: replacement?.string) else { return }
        storage.beginEditing()
        if let replacement {
            storage.replaceCharacters(in: range, with: replacement)
            restyle(NSRange(location: range.location, length: replacement.length))
            setSelectedRange(NSRange(location: range.location + replacement.length, length: 0))
        }
        change()
        storage.endEditing()
        didChangeText()
    }

    /// Fonts from each run's traits and the heading mark; colours from the theme.
    func restyle(_ range: NSRange) {
        guard let storage = textStorage, range.length > 0, range.location + range.length <= storage.length else { return }
        storage.beginEditing()
        storage.enumerateAttributes(in: range) { attributes, run, _ in
            let heading = attributes[.openLogosNoteHeading] != nil
            let traits = (attributes[.font] as? NSFont)?.fontDescriptor.symbolicTraits.intersection([.bold, .italic]) ?? []
            var font = heading ? typography.heading : typography.body
            if !traits.isEmpty { font = NSFont(descriptor: font.fontDescriptor.withSymbolicTraits(font.fontDescriptor.symbolicTraits.union(traits)), size: font.pointSize) ?? font }
            storage.addAttributes([.font: font, .foregroundColor: PlatformColor(hex: typography.text)], range: run)
        }
        // Paragraph styles: spacing, a hanging indent for `• ` items, the quote's indent.
        let text = storage.string as NSString
        text.enumerateSubstrings(in: text.paragraphRange(for: range), options: [.byParagraphs, .substringNotRequired]) { _, paragraphRange, enclosing, _ in
            let style = NSMutableParagraphStyle()
            style.lineHeightMultiple = self.typography.lineSpacing
            style.paragraphSpacing = self.typography.size * 0.4
            if paragraphRange.length > 0, storage.attribute(.openLogosNoteQuote, at: paragraphRange.location, effectiveRange: nil) != nil {
                style.firstLineHeadIndent = NoteTypography.quoteIndent
                style.headIndent = NoteTypography.quoteIndent
            } else if text.substring(with: paragraphRange).hasPrefix("• ") {
                style.headIndent = self.typography.bulletIndent
            }
            storage.addAttribute(.paragraphStyle, value: style, range: enclosing)
        }
        storage.endEditing()
        needsDisplay = true
    }

    /// The quote paragraphs' 2 pt rule on the leading edge, in the theme's rule colour.
    override func drawBackground(in rect: NSRect) {
        super.drawBackground(in: rect)
        guard let storage = textStorage, storage.length > 0, let manager = textLayoutManager,
              let content = manager.textContentManager else { return }
        PlatformColor(hex: typography.rules).setFill()
        storage.enumerateAttribute(.openLogosNoteQuote, in: NSRange(location: 0, length: storage.length)) { value, range, _ in
            guard value != nil,
                  let start = content.location(content.documentRange.location, offsetBy: range.location),
                  let end = content.location(start, offsetBy: range.length),
                  let textRange = NSTextRange(location: start, end: end) else { return }
            var box = NSRect.null
            manager.enumerateTextSegments(in: textRange, type: .standard, options: []) { _, frame, _, _ in
                box = box.union(frame)
                return true
            }
            guard !box.isNull else { return }
            let origin = textContainerOrigin
            NSRect(x: origin.x, y: box.minY + origin.y, width: 2, height: box.height).fill()
        }
    }

    func apply(_ next: NoteTypography) {
        guard next != typography else { return }
        typography = next
        linkTextAttributes = [
            .foregroundColor: PlatformColor(hex: next.accent),
            .underlineStyle: NSUnderlineStyle.single.rawValue,
            .cursor: NSCursor.pointingHand,
        ]
        insertionPointColor = PlatformColor(hex: next.text)
        var typing = typingAttributes
        typing[.font] = next.body
        typing[.foregroundColor] = PlatformColor(hex: next.text)
        typingAttributes = typing
        if let storage = textStorage { restyle(NSRange(location: 0, length: storage.length)) }
    }
}

/// A note in a notes tab or the dock.
struct NoteEditorView: NSViewRepresentable {
    let store: NoteStore
    let noteID: String
    let paneID: String
    let typography: NoteTypography
    var horizontalPadding: CGFloat = ShellTokens.Spacing.xl
    var onFocus: () -> Void = {}

    func makeCoordinator() -> Coordinator { Coordinator(store: store) }

    func makeNSView(context: Context) -> NSScrollView {
        let view = NoteTextView(usingTextLayoutManager: true)
        view.paneID = paneID
        view.noteID = noteID
        view.store = store
        view.isRichText = true
        view.isEditable = true
        view.isSelectable = true
        view.allowsUndo = true
        view.drawsBackground = false
        view.isAutomaticQuoteSubstitutionEnabled = false
        view.textContainerInset = NSSize(width: horizontalPadding, height: 20)
        view.textContainer?.lineFragmentPadding = 0
        view.textContainer?.widthTracksTextView = true
        view.isVerticallyResizable = true
        view.isHorizontallyResizable = false
        view.autoresizingMask = [.width]
        view.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        view.delegate = context.coordinator
        view.setAccessibilityIdentifier("note-\(paneID)")
        view.setAccessibilityLabel("Note")
        view.apply(typography)
        view.load(store.document(noteID))
        store.register(view)
        let scroll = NSScrollView()
        scroll.drawsBackground = false
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.documentView = view
        configure(view)
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        guard let view = scroll.documentView as? NoteTextView else { return }
        configure(view)
        view.textContainerInset = NSSize(width: horizontalPadding, height: 20)
        view.apply(typography)
    }

    private func configure(_ view: NoteTextView) {
        view.onFocus = onFocus
    }

    @MainActor
    final class Coordinator: NSObject, NSTextViewDelegate {
        let store: NoteStore
        init(store: NoteStore) { self.store = store }

        func textDidChange(_ notification: Notification) {
            guard let view = notification.object as? NoteTextView else { return }
            store.didEdit(view)
        }

        func textView(_ textView: NSTextView, clickedOnLink link: Any, at charIndex: Int) -> Bool {
            guard let reference = NoteStore.reference(link) else { return false }
            store.model?.openPassageLink(reference)
            return true
        }
    }
}
#else
/// iPad keeps B072's read-only note until B075 brings the editor there.
@MainActor
final class NoteStore {
    enum Trait { case bold, italic }
    weak var model: ShellModel?
    var editing: Bool { false }
    func toggle(_ trait: Trait) {}
    func toggleHeading() {}
    func toggleList() {}
    func insertLink(to reference: String) {}
    func appendQuote(_ text: String, reference: String, to noteID: String) {}
    func flush() {}
}
#endif
