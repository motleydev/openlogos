// B076: notes are editable in memory (nothing is persisted until B074). A notes tab and the
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

    func document(_ noteID: String) -> NSAttributedString {
        if let document = documents[noteID] { return document }
        let paragraphs = model?.textSource.text(resourceID: noteID, reference: nil)?.content.verses.map(\.text) ?? []
        let document = NSAttributedString(string: paragraphs.joined(separator: "\n"))
        documents[noteID] = document
        return document
    }

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

    /// Note on it: `> “quoted text” — John 3:16` as a new paragraph at the end of the note,
    /// the passage a link.
    func appendQuote(_ text: String, reference: String, to noteID: String) {
        let quote = NSMutableAttributedString(string: "> “\(text)” — ")
        let first = reference.split(separator: "–").first.map(String.init) ?? reference
        var link: [NSAttributedString.Key: Any] = [:]
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
        }
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

    var body: NSFont { Self.serif(size, weight: .regular) }
    var heading: NSFont { Self.serif(size + 4, weight: .semibold) }

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

    override func mouseDown(with event: NSEvent) {
        onFocus()
        super.mouseDown(with: event)
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
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineHeightMultiple = typography.lineSpacing
        paragraph.paragraphSpacing = typography.size * 0.4
        storage.beginEditing()
        storage.enumerateAttributes(in: range) { attributes, run, _ in
            let heading = attributes[.openLogosNoteHeading] != nil
            let traits = (attributes[.font] as? NSFont)?.fontDescriptor.symbolicTraits.intersection([.bold, .italic]) ?? []
            var font = heading ? typography.heading : typography.body
            if !traits.isEmpty { font = NSFont(descriptor: font.fontDescriptor.withSymbolicTraits(font.fontDescriptor.symbolicTraits.union(traits)), size: font.pointSize) ?? font }
            storage.addAttributes([.font: font, .foregroundColor: PlatformColor(hex: typography.text), .paragraphStyle: paragraph], range: run)
        }
        storage.endEditing()
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
}
#endif
