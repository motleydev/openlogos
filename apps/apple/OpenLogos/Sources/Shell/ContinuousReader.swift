// B078: continuous reading on the Mac (specs/23 §7). A reading tab shows its edition as one
// stream of chapters: a typed passage opens its chapter with the passage at the top, flashed
// once and quietly marked, and the reader scrolls on into the chapters before and after.
// Only a window of chapter blocks is in the text view (the one in view and up to two each
// side, ChapterLayout.swift); nearing an end loads the next block, far blocks are released,
// and the verse at the top stays where it is on screen while blocks come and go (the clip
// view's origin moves by what was inserted or removed above). A sticky header names the
// block in view (`Jn 3 · BSB`). The verse at the top is reported to the core as the reader
// scrolls (`tab.record_scroll`, quiet); linked tabs are scrolled to the verse the core maps
// for them, without animation and without reporting it back. The reading column is keyed on
// the tab and its edition: a reference changed by scrolling never rebuilds it.
#if os(macOS)
import AppKit
import SwiftUI

/// The SwiftUI face: the container with the header and the reading surface.
struct ContinuousReaderView: NSViewRepresentable {
    @ObservedObject var model: ShellModel
    let tab: ShellTab
    let column: ShellColumn
    let horizontalPadding: CGFloat

    func makeCoordinator() -> ReadingStream {
        ReadingStream(tabID: tab.id, resourceID: tab.contentId, reading: model.reading)
    }

    func makeNSView(context: Context) -> ReadingContainerView {
        let stream = context.coordinator
        let view = stream.makeView()
        configure(stream)
        model.register(stream)
        stream.start(tab: tab, revision: model.session.revision)
        return view
    }

    func updateNSView(_ view: ReadingContainerView, context: Context) {
        let stream = context.coordinator
        configure(stream)
        model.register(stream)
        stream.update(tab: tab, revision: model.session.revision)
    }

    private func configure(_ stream: ReadingStream) {
        let surface = stream.surface
        let tabID = tab.id
        let model = model
        let column = column
        let tab = tab
        surface.onSelection = { model.selectionChanged($0, tabID: tabID) }
        surface.onAction = { action, selection in
            guard let selection else { return }
            model.selectionChanged(selection, tabID: tabID)
            switch action {
            case .highlight: model.run("selection.highlight")
            case .collect: model.run("selection.collect")
            case .study: model.run("selection.study")
            default: model.show(toast: "\(action.rawValue): not in this preview yet")
            }
        }
        surface.onDragEnd = { mode, selection in model.dragEnded(mode, selection) }
        surface.onActivate = { model.focusColumn(column.number) }
        surface.onContextMenu = { hit in model.nsMenu(for: hit, tab: tab, column: column) }
        surface.dragMode = model.dragMode
        let info = model.textSource.resource(tab.contentId)
        stream.edition = (shortName: info.shortName, isRightToLeft: info.isRightToLeft)
        stream.theme = model.theme
        stream.attribution = { chapter in model.attribution(for: chapter) }
        stream.onTopSegment = { segment in model.recordScroll(tabID: tabID, segmentID: segment) }
        stream.onSettle = { model.scrollSettled() }
        stream.setTypography(model.typography, padding: horizontalPadding)
        surface.setMarks(model.readerMarks(tabID))
    }
}

/// The header bar over the text and the surface's scroll view under it.
final class ReadingContainerView: NSView {
    weak var stream: ReadingStream?
    let header = ReadingHeaderView()
    let scrollView: NSScrollView

    init(scrollView: NSScrollView) {
        self.scrollView = scrollView
        super.init(frame: .zero)
        addSubview(scrollView)
        addSubview(header)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    override var isFlipped: Bool { true }

    override func layout() {
        super.layout()
        let height = ReadingHeaderView.height
        header.frame = NSRect(x: 0, y: 0, width: bounds.width, height: height)
        scrollView.frame = NSRect(x: 0, y: height, width: bounds.width, height: max(0, bounds.height - height))
        stream?.containerDidLayout()
    }
}

/// `Jn 3 · BSB`: the block in view, over a 1 pt rule.
final class ReadingHeaderView: NSView {
    static let height: CGFloat = 28
    let label = NSTextField(labelWithString: "")
    private let rule = NSView()
    private var padding: CGFloat = 24

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        rule.wantsLayer = true
        label.lineBreakMode = .byTruncatingTail
        label.setAccessibilityIdentifier("reading-header")
        addSubview(label)
        addSubview(rule)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    override var isFlipped: Bool { true }

    func set(chapter: String, edition: String, theme: ShellTokens.Theme, padding: CGFloat) {
        self.padding = padding
        let text = NSMutableAttributedString(string: chapter, attributes: [
            .font: NSFont.systemFont(ofSize: ShellTokens.chromeSize, weight: .semibold),
            .foregroundColor: PlatformColor(hex: theme.text),
        ])
        text.append(NSAttributedString(string: " · \(edition)", attributes: [
            .font: NSFont.systemFont(ofSize: ShellTokens.chromeSize),
            .foregroundColor: PlatformColor(hex: theme.chrome),
        ]))
        if label.attributedStringValue != text { label.attributedStringValue = text }
        label.setAccessibilityLabel("\(chapter), \(edition)")
        layer?.backgroundColor = PlatformColor(hex: theme.page).cgColor
        rule.layer?.backgroundColor = PlatformColor(hex: theme.rules).cgColor
        needsLayout = true
    }

    var text: String { label.stringValue }

    override func layout() {
        super.layout()
        let size = label.intrinsicContentSize
        label.frame = NSRect(x: padding, y: (bounds.height - size.height) / 2, width: max(0, bounds.width - 2 * padding), height: size.height)
        rule.frame = NSRect(x: 0, y: bounds.height - ShellTokens.rule, width: bounds.width, height: ShellTokens.rule)
    }
}

/// One reading tab's window of chapters over its surface.
@MainActor
final class ReadingStream: NSObject {
    let tabID: String
    private(set) var resourceID: String
    let reading: ReadingHandle
    let surface: ReaderSurface
    private(set) var container: ReadingContainerView?

    /// The window, in reading order.
    private(set) var blocks: [ReadingChapter] = []
    /// The reference the core last gave this tab, and the session revision it came with: a
    /// session older than that is not a navigation, whatever it says.
    private(set) var knownReference: String?
    private var knownRevision: UInt64 = 0
    /// The typed verses (`verse_start`, `verse_end`), marked until the tab's mark is cleared.
    private(set) var highlight: (start: String, end: String)?
    private var flashLevel: CGFloat = 0
    private var flashTimer: Timer?

    var edition: (shortName: String, isRightToLeft: Bool) = ("", false)
    var theme: ShellTokens.Theme = ShellTokens.light
    var attribution: (ReadingChapter) -> String? = { _ in nil }
    var onTopSegment: (String) -> Void = { _ in }
    var onSettle: () -> Void = {}

    /// This many blocks: the one in view and two each side. Short chapters (Psalm 117) can
    /// need more to fill the screens around the reader; never more than `hardMaxBlocks`.
    static let maxBlocks = 5
    static let hardMaxBlocks = 8
    /// The typed verse's first line sits this far below the top of the text.
    static let topMargin: CGFloat = 8

    private var typography: ReaderTypography = .current()
    private var padding: CGFloat = 24
    private var layoutWidth: CGFloat = 0
    /// A jump waiting for the view to have a size.
    private var pendingJump: (segment: String, flash: Bool)?
    /// The segment at the top, as last reported or set, and how far its first line is below
    /// the top of the visible text (what a relayout keeps).
    private(set) var topSegment: String?
    private var topOffset: CGFloat = 0
    /// The segment last reported, or set by the stream itself (a jump, a follow).
    private var reportedTop: String?
    /// Set while the stream scrolls itself: those scrolls are not the reader's.
    private var programmatic = 0
    private var maintaining = false
    private var settleTimer: Timer?
    private var serial = 0
    /// The last error `locate` raised (the column shows the not-in-edition state for it).
    private(set) var lastError: String?

    /// Window edits since the stream opened (the tests and the measurement read them).
    private(set) var edits: [ReaderLayoutEdit] = []

    init(tabID: String, resourceID: String, reading: ReadingHandle) {
        self.tabID = tabID
        self.resourceID = resourceID
        self.reading = reading
        surface = ReaderSurface(paneID: tabID, onSelection: { _ in }, onAction: { _, _ in }, onMeasurement: { _ in })
        surface.keepsLayoutExact = true
        super.init()
        surface.revealOverride = { [weak self] range in self?.reveal(range) }
    }

    func makeView() -> ReadingContainerView {
        let scroll = surface.makeScrollView()
        let view = ReadingContainerView(scrollView: scroll)
        view.stream = self
        container = view
        scroll.contentView.postsBoundsChangedNotifications = true
        NotificationCenter.default.addObserver(self, selector: #selector(boundsChanged(_:)), name: NSView.boundsDidChangeNotification, object: scroll.contentView)
        return view
    }

    // MARK: Tab state

    /// The tab as the view first shows it: where it was left (its scroll anchor), else its
    /// passage, with the typed verses marked (not flashed again).
    func start(tab: ShellTab, revision: UInt64) {
        knownRevision = revision
        open(reference: tab.reference, segment: tab.scrollSegmentId, highlightRange: tab.highlightRange, flash: false)
    }

    /// A render of the column. Only a newer session whose passage is not the one this stream
    /// knows is a navigation (Back, Forward, undo, a restored session).
    func update(tab: ShellTab, revision: UInt64) {
        if tab.contentId != resourceID {
            resourceID = tab.contentId
            blocks = []
            knownRevision = revision
            open(reference: tab.reference, segment: tab.scrollSegmentId, highlightRange: tab.highlightRange, flash: false)
            return
        }
        guard revision >= knownRevision else { return }
        if tab.reference != knownReference {
            knownRevision = revision
            open(reference: tab.reference, segment: tab.scrollSegmentId, highlightRange: tab.highlightRange, flash: tab.highlightRange != nil)
        } else if tab.highlightRange == nil, highlight != nil {
            clearHighlight()
        }
        updateHeader()
    }

    /// The core moved the tab: a navigation (`scrolled: false`) jumps and flashes; a follow
    /// (`scrolled: true`) scrolls to the mapped verse without animation.
    func apply(_ position: ShellTabPosition, revision: UInt64) {
        knownRevision = max(knownRevision, revision)
        if position.scrolled {
            knownReference = position.reference
            if !position.driver { follow(segment: position.segmentId ?? position.reference) }
        } else {
            open(reference: position.reference, segment: position.segmentId, highlightRange: position.highlightRange, flash: position.highlightRange != nil)
        }
    }

    /// Records what the core now says the tab's passage is, after a scroll this stream reported.
    func acknowledge(reference: String, revision: UInt64) {
        knownReference = reference
        knownRevision = max(knownRevision, revision)
    }

    // MARK: Opening

    /// Opens `reference` (OSIS): its chapter with the chapters around it, `segment` (or the
    /// passage's first segment) at the top, the highlight range marked and, for a typed
    /// passage, flashed once.
    func open(reference: String?, segment: String?, highlightRange: String?, flash: Bool) {
        programmatic += 1
        defer { programmatic -= 1 }
        knownReference = reference
        guard let reference else { return }
        let location: ReadingLocation
        do {
            location = try reading.locate(resourceId: resourceID, input: reference)
            lastError = nil
        } catch ReadingCallError.Refused(_, let reason) {
            lastError = reason
            return
        } catch {
            lastError = "\(error)"
            return
        }
        if let highlightRange, let marked = try? reading.locate(resourceId: resourceID, input: highlightRange) {
            highlight = (marked.verseStart, marked.verseEnd)
        } else {
            highlight = nil
        }
        let target = segment ?? location.segmentId
        let chapter = ShellReference.chapter(of: target)
        if !blocks.contains(where: { $0.chapterKey == chapter }) {
            load(around: (try? reading.chapterContent(resourceId: resourceID, chapterKey: chapter)) ?? (try? reading.chapterContent(resourceId: resourceID, chapterKey: location.chapterKey)))
        } else {
            relayout(.none)
        }
        jump(to: target, flash: flash)
    }

    /// A linked tab's move: the verse at the top, no flash, not reported back.
    func follow(segment: String) {
        programmatic += 1
        defer { programmatic -= 1 }
        let chapter = ShellReference.chapter(of: segment)
        if !blocks.contains(where: { $0.chapterKey == chapter }) {
            guard let block = try? reading.chapterContent(resourceId: resourceID, chapterKey: chapter) else { return }
            load(around: block)
        }
        jump(to: segment, flash: false)
    }

    /// A fresh window: the chapter and one block each side.
    private func load(around chapter: ReadingChapter?) {
        guard let chapter else { return }
        var window = [chapter]
        if let prev = chapter.prev, let block = try? reading.chapterContent(resourceId: resourceID, chapterKey: prev) { window.insert(block, at: 0) }
        if let next = chapter.next, let block = try? reading.chapterContent(resourceId: resourceID, chapterKey: next) { window.append(block) }
        blocks = window
        relayout(.replace)
    }

    private func jump(to segment: String, flash: Bool) {
        guard let scroll = container?.scrollView, scroll.contentView.bounds.height > 0, let layout = surface.layout else {
            pendingJump = (segment, flash)
            return
        }
        pendingJump = nil
        let target = layout.segment(holding: segment) ?? segment
        // Enough text on both sides of the target to put it at the top and scroll on.
        let height = scroll.contentView.bounds.height
        while blocks.count < Self.maxBlocks, let next = blocks.last?.next, let top = top(of: target),
              documentHeight - top < height * 2, let block = try? reading.chapterContent(resourceId: resourceID, chapterKey: next) {
            blocks.append(block)
            relayout(.append)
        }
        while blocks.count < Self.maxBlocks, let prev = blocks.first?.prev, let top = top(of: target), top < height,
              let block = try? reading.chapterContent(resourceId: resourceID, chapterKey: prev) {
            blocks.insert(block, at: 0)
            relayout(.prepend)
        }
        if let top = top(of: target) {
            setOrigin(top - Self.topMargin)
        }
        recordTop(target)
        reportedTop = target
        updateHeader()
        if flash { self.flash() } else { paintHighlight() }
    }

    // MARK: Layout

    private enum Change { case none, replace, append, prepend, dropFirst(NSRange), dropLast(NSRange) }

    /// Builds the window's layout and puts it in the text storage by the smallest edit.
    private func relayout(_ change: Change) {
        // The storage edit resizes the text view, which moves the clip view: not the reader.
        programmatic += 1
        defer { programmatic -= 1 }
        serial += 1
        let built = ReaderLayout(blocks: blocks, id: "\(resourceID)|window-\(tabID)-\(serial)", editionId: resourceID,
                                 isRightToLeft: edition.isRightToLeft, typography: typography, attribution: attribution)
        let edit: ReaderLayoutEdit
        let old = surface.layout?.display.length ?? 0
        switch change {
        case .none: return
        case .replace: edit = .replace
        case .append: edit = .insert(at: old, length: built.display.length - old)
        case .prepend: edit = .insert(at: 0, length: built.display.length - old)
        case .dropFirst(let range), .dropLast(let range): edit = .delete(range)
        }
        surface.replace(layout: built, edit: edit, typographyKey: typography.key, overlay: overlayMarks(in: built))
        edits.append(edit)
        ensureLayout()
    }

    /// Lays the whole window out, so heights above the reader are exact, never estimates
    /// (TextKit 2 estimates what it has not laid out, and an estimate that changes moves the text).
    private func ensureLayout() {
        guard let textView = surface.textView, let manager = textView.textLayoutManager else { return }
        manager.ensureLayout(for: manager.documentRange)
        manager.textViewportLayoutController.layoutViewport()
    }

    var documentHeight: CGFloat { surface.textView?.frame.height ?? 0 }

    /// The top of a segment's first line, in the text view's coordinates.
    func top(of segment: String) -> CGFloat? {
        guard let range = surface.layout?.verseRanges[segment] else { return nil }
        return surface.rect(for: NSRange(location: range.location, length: min(1, range.length)))?.minY
    }

    /// The top of a block's heading.
    func top(ofBlock key: String) -> CGFloat? {
        guard let block = surface.layout?.blocks.first(where: { $0.chapterKey == key }) else { return nil }
        return surface.rect(for: NSRange(location: block.range.location, length: 1))?.minY
    }

    private func setOrigin(_ y: CGFloat) {
        guard let scroll = container?.scrollView else { return }
        let maxY = max(0, documentHeight - scroll.contentView.bounds.height)
        programmatic += 1
        scroll.contentView.setBoundsOrigin(NSPoint(x: 0, y: max(0, min(maxY, y.rounded()))))
        scroll.reflectScrolledClipView(scroll.contentView)
        programmatic -= 1
    }

    /// Changes the window with the verse at the top kept where it is on screen. `stored`: the
    /// text has already reflowed (a width change), so the anchor is the one recorded before.
    private func anchored(stored: Bool = false, _ change: () -> Void) {
        guard let scroll = container?.scrollView else { return change() }
        let anchor = stored ? topSegment : probeTopSegment()
        let before = stored ? topOffset : anchor.flatMap(top(of:)).map { $0 - scroll.contentView.bounds.minY }
        change()
        if let anchor, let before, let after = top(of: anchor) {
            setOrigin(after - before)
            recordTop(anchor)
        }
    }

    /// Notes the segment at the top and its offset below the top of the visible text.
    private func recordTop(_ segment: String) {
        topSegment = segment
        if let scroll = container?.scrollView, let top = top(of: segment) { topOffset = top - scroll.contentView.bounds.minY }
    }

    func setTypography(_ next: ReaderTypography, padding: CGFloat) {
        let restyle = next.key != typography.key
        guard restyle || padding != self.padding else { return }
        typography = next
        self.padding = padding
        guard !blocks.isEmpty, pendingJump == nil else { return surface.setPadding(padding) }
        // The text may already have reflowed (the column was resized): keep the recorded top.
        anchored(stored: true) {
            surface.setPadding(padding)
            if restyle { relayout(.replace) } else { ensureLayout() }
        }
        updateHeader()
    }

    func containerDidLayout() {
        guard let scroll = container?.scrollView else { return }
        let width = scroll.contentView.bounds.width
        if width != layoutWidth {
            let first = layoutWidth == 0
            layoutWidth = width
            if !first, !blocks.isEmpty, pendingJump == nil { anchored(stored: true) { ensureLayout() } }
            onScrolled()
        }
        if let pending = pendingJump, scroll.contentView.bounds.height > 0 {
            ensureLayout()
            jump(to: pending.segment, flash: pending.flash)
        }
        updateHeader()
    }

    // MARK: Scrolling

    @objc private func boundsChanged(_ note: Notification) {
        defer { if !maintaining { onScrolled() } }
        guard !maintaining, let scroll = container?.scrollView else { return }
        // A resize reflows the text before `containerDidLayout` re-anchors it: not a scroll.
        guard pendingJump == nil, scroll.contentView.bounds.width == layoutWidth else { return }
        let previous = reportedTop
        maintain()
        let top = probeTopSegment()
        updateHeader()
        if let top {
            let changed = top != previous
            recordTop(top)
            if changed, programmatic == 0 {
                reportedTop = top
                onTopSegment(top)
            }
        }
        if programmatic == 0 {
            settleTimer?.invalidate()
            settleTimer = Timer.scheduledTimer(withTimeInterval: 0.15, repeats: false) { [weak self] _ in
                MainActor.assumeIsolated { self?.onSettle() }
            }
        }
    }

    /// Loads a block when the reader nears an end of the window and releases one far out of
    /// view; one change per call, so a fast scroll spreads the work over frames.
    @discardableResult
    func maintain() -> Bool {
        guard let scroll = container?.scrollView, !blocks.isEmpty, let layout = surface.layout else { return false }
        maintaining = true
        defer { maintaining = false }
        let visible = scroll.contentView.bounds
        let height = max(visible.height, 200)
        if visible.minY < height * 1.5, let prev = blocks.first?.prev,
           let block = try? reading.chapterContent(resourceId: resourceID, chapterKey: prev) {
            anchored {
                blocks.insert(block, at: 0)
                relayout(.prepend)
            }
            trim(keepingNear: visible)
            return true
        }
        if documentHeight - visible.maxY < height * 1.5, let next = blocks.last?.next,
           let block = try? reading.chapterContent(resourceId: resourceID, chapterKey: next) {
            blocks.append(block)
            anchored { relayout(.append) }
            trim(keepingNear: visible)
            return true
        }
        _ = layout
        return trim(keepingNear: visible)
    }

    /// Releases blocks beyond the cap that are more than a screen out of view.
    @discardableResult
    private func trim(keepingNear visible: NSRect) -> Bool {
        guard blocks.count > Self.maxBlocks, let scroll = container?.scrollView, let layout = surface.layout else { return false }
        let bounds = scroll.contentView.bounds
        // Over the hard cap the farther block goes even if it is near.
        let height = blocks.count > Self.hardMaxBlocks ? 0 : max(bounds.height, 200)
        if let first = layout.blocks.first, let second = layout.blocks.dropFirst().first,
           let end = surface.rect(for: NSRange(location: second.range.location, length: 1))?.minY, end < bounds.minY - height {
            anchored {
                blocks.removeFirst()
                relayout(.dropFirst(NSRange(location: 0, length: second.range.location)))
            }
            _ = first
            return true
        }
        if let last = layout.blocks.last, let top = top(ofBlock: last.chapterKey), top > bounds.maxY + height {
            let from = last.range.location - (layout.content.joiner as NSString).length - (ReaderLayout.blockBreak as NSString).length
            anchored {
                blocks.removeLast()
                relayout(.dropLast(NSRange(location: from, length: layout.display.length - from)))
            }
            return true
        }
        return false
    }

    /// The segment whose line is at the top of the visible text: the first line whose bottom
    /// is below the probe (a gap between paragraphs counts as the line after it), read from
    /// TextKit 2's line fragments.
    func probeTopSegment() -> String? {
        guard let scroll = container?.scrollView else { return nil }
        return segment(atTextY: scroll.contentView.bounds.minY + Self.topMargin + 4)
    }

    /// The segment whose line is at a height of the text view (or the next line below a gap).
    func segment(atTextY textY: CGFloat) -> String? {
        guard let textView = surface.textView, let layout = surface.layout, layout.display.length > 0,
              let manager = textView.textLayoutManager, let content = manager.textContentManager else { return nil }
        let y = textY - textView.textContainerOrigin.y
        var offset: Int?
        let start = manager.textLayoutFragment(for: CGPoint(x: 0, y: max(0, y)))?.rangeInElement.location ?? manager.documentRange.location
        manager.enumerateTextLayoutFragments(from: start, options: []) { fragment in
            let frame = fragment.layoutFragmentFrame
            guard frame.maxY > y else { return true }
            let base = content.offset(from: content.documentRange.location, to: fragment.rangeInElement.location)
            let line = fragment.textLineFragments.first { $0.typographicBounds.maxY + frame.minY > y } ?? fragment.textLineFragments.last
            offset = base + (line?.characterRange.location ?? 0)
            return false
        }
        return layout.segment(at: offset ?? layout.display.length - 1)
    }

    // MARK: Ink on verses

    /// Called after every scroll and window change (the ink layer follows the text).
    var onScrolled: () -> Void = {}

    /// The canvas (over the header and the text) to text view coordinates and back.
    private var canvasShift: CGFloat {
        (container?.scrollView.contentView.bounds.minY ?? 0) - ReadingHeaderView.height
    }

    /// The verse under a canvas height and the top of its first line there.
    func inkAnchor(atCanvasY y: CGFloat) -> InkSheet.Anchor? {
        guard let segment = segment(atTextY: y + canvasShift), let top = top(of: segment) else { return nil }
        return InkSheet.Anchor(segment: segment, y: top - canvasShift)
    }

    /// How far the verse a stroke was drawn on has moved on the canvas; nil when it is not in
    /// the window.
    func inkOffset(_ anchor: InkSheet.Anchor) -> CGFloat? {
        top(of: anchor.segment).map { $0 - canvasShift - anchor.y }
    }

    /// The chapter block whose text is at the top.
    var topBlock: ReaderBlockSpan? {
        guard let layout = surface.layout else { return nil }
        if let segment = topSegment ?? probeTopSegment(), let range = layout.verseRanges[segment] {
            return layout.block(at: range.location)
        }
        return layout.blocks.first
    }

    private func updateHeader() {
        guard let header = container?.header else { return }
        header.set(chapter: topBlock?.shortLabel ?? "", edition: edition.shortName, theme: theme, padding: padding)
    }

    // MARK: The typed verses' mark

    /// The typed verses from `verse_start` to `verse_end` in document order; a range that runs
    /// past an end of the window is marked up to that end.
    private func overlayMarks(in layout: ReaderLayout) -> [ReaderMark] {
        guard let highlight else { return [] }
        let order = layout.segmentOrder
        let startIndex = order.firstIndex(of: highlight.start), endIndex = order.lastIndex(of: highlight.end)
        guard startIndex != nil || endIndex != nil else { return [] }
        let start = startIndex ?? 0, end = endIndex ?? order.count - 1
        guard start <= end, let range = layout.displayRange(from: order[start], to: order[end]) else { return [] }
        let accent = PlatformColor(hex: theme.accent)
        return [ReaderMark(id: "typed", ranges: [range], color: accent.withAlphaComponent(max(Self.quietMark, flashLevel)))]
    }

    static let quietMark: CGFloat = 0.12
    static let flashPeak: CGFloat = 0.35

    private func paintHighlight() {
        guard let layout = surface.layout else { return }
        surface.setOverlay(overlayMarks(in: layout))
    }

    /// The typed verses at the accent's 35 %, easing to the quiet 12 % over 0.9 s.
    func flash() {
        flashTimer?.invalidate()
        flashLevel = Self.flashPeak
        paintHighlight()
        let start = CACurrentMediaTime()
        flashTimer = Timer.scheduledTimer(withTimeInterval: 0.075, repeats: true) { [weak self] timer in
            MainActor.assumeIsolated {
                guard let self else { return timer.invalidate() }
                let t = min(1, (CACurrentMediaTime() - start) / 0.9)
                let eased = 1 - pow(1 - t, 2)
                self.flashLevel = Self.flashPeak - (Self.flashPeak - Self.quietMark) * CGFloat(eased)
                if t >= 1 {
                    self.flashLevel = 0
                    timer.invalidate()
                    self.flashTimer = nil
                }
                self.paintHighlight()
            }
        }
    }

    var isFlashing: Bool { flashTimer != nil }

    func clearHighlight() {
        flashTimer?.invalidate()
        flashTimer = nil
        flashLevel = 0
        highlight = nil
        surface.setOverlay([])
    }

    /// A range the reader asked for (a selection made in code, Find's next match): left where
    /// it is when it is in view, else brought to the top. The scroll is reported like theirs.
    func reveal(_ range: NSRange) {
        guard let scroll = container?.scrollView, let box = surface.rect(for: range) else { return }
        let visible = scroll.contentView.bounds
        guard box.minY < visible.minY || box.maxY > visible.maxY else { return }
        scrollAsReader(by: box.minY - Self.topMargin - visible.minY)
    }

    /// Scrolls as the reader would (a wheel or a trackpad: each step reported like theirs), for
    /// the launch hooks, the measurement and the tests.
    func scrollAsReader(by delta: CGFloat) {
        guard let scroll = container?.scrollView else { return }
        let clip = scroll.contentView
        let maxY = max(0, documentHeight - clip.bounds.height)
        clip.setBoundsOrigin(NSPoint(x: 0, y: max(0, min(maxY, clip.bounds.minY + delta))))
        scroll.reflectScrolledClipView(clip)
    }

    /// The verses in the window and the text storage's length (the DOM-cap row of the evidence).
    var renderedVerses: Int { surface.layout?.segmentOrder.count ?? 0 }
}
#endif
