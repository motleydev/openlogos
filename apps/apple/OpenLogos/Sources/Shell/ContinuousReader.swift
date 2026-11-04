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

    /// The tab's stream, kept by the model while the tab is hidden, so showing it again
    /// shows its window as it was (a tab or workspace switch lays nothing out again).
    func makeCoordinator() -> ReadingStream {
        model.pooledStream(for: tab)
    }

    func makeNSView(context: Context) -> ReadingContainerView {
        let stream = context.coordinator
        let view = stream.container ?? stream.makeView()
        configure(stream)
        model.register(stream)
        if stream.started {
            stream.update(tab: tab, revision: model.session.revision)
        } else {
            stream.start(tab: tab, revision: model.session.revision)
        }
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
        stream.onSettleReport = { segment, scalar in model.recordScroll(tabID: tabID, segmentID: segment, scalar: scalar) }
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

/// `Jn 3 · BSB`: the block in view, over a 1 pt rule. Drawn by the view itself: a label whose
/// text changes would ask the window for a layout pass (SwiftUI's included) at every chapter.
final class ReadingHeaderView: NSView {
    static let height: CGFloat = 28
    private var title = NSAttributedString()
    private var page = PlatformColor.clear
    private var rules = PlatformColor.clear
    private var padding: CGFloat = 24

    override init(frame: NSRect) {
        super.init(frame: frame)
        setAccessibilityElement(true)
        setAccessibilityRole(.staticText)
        setAccessibilityIdentifier("reading-header")
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    override var isFlipped: Bool { true }

    func set(chapter: String, edition: String, theme: ShellTokens.Theme, padding: CGFloat) {
        let text = NSMutableAttributedString(string: chapter, attributes: [
            .font: NSFont.systemFont(ofSize: ShellTokens.chromeSize, weight: .semibold),
            .foregroundColor: PlatformColor(hex: theme.text),
        ])
        text.append(NSAttributedString(string: " · \(edition)", attributes: [
            .font: NSFont.systemFont(ofSize: ShellTokens.chromeSize),
            .foregroundColor: PlatformColor(hex: theme.chrome),
        ]))
        let page = PlatformColor(hex: theme.page), rules = PlatformColor(hex: theme.rules)
        guard text != title || page != self.page || rules != self.rules || padding != self.padding else { return }
        title = text
        self.page = page
        self.rules = rules
        self.padding = padding
        setAccessibilityLabel("\(chapter), \(edition)")
        needsDisplay = true
    }

    var text: String { title.string }

    override func draw(_ dirtyRect: NSRect) {
        page.setFill()
        bounds.fill()
        rules.setFill()
        NSRect(x: 0, y: bounds.height - ShellTokens.rule, width: bounds.width, height: ShellTokens.rule).fill()
        let size = title.size()
        title.draw(with: NSRect(x: padding, y: (bounds.height - size.height) / 2, width: max(0, bounds.width - 2 * padding), height: size.height),
                   options: [.usesLineFragmentOrigin, .truncatesLastVisibleLine])
    }
}

/// Work that can wait a frame, one item per display-link frame: the streams' loads, releases
/// and layout of blocks added below that are not urgent. An item that did something (returns
/// true) goes again in a later frame.
@MainActor
final class ReadingScheduler: NSObject {
    static let shared = ReadingScheduler()
    private var work: [(key: String, run: () -> Bool)] = []
    private var firsts: [(key: String, run: () -> Bool)] = []
    private var link: CADisplayLink?
    /// How long each item took, and what it was (the measurement reads them).
    var durations: [Double] = []
    var kinds: [String] = []

    /// `first`: small work that must not wait behind loads (a scroll report); every such item
    /// runs on the next frame, before the one regular item.
    func enqueue(_ key: String, first: Bool = false, _ run: @escaping () -> Bool) {
        guard !work.contains(where: { $0.key == key }), !firsts.contains(where: { $0.key == key }) else { return }
        if first { firsts.append((key, run)) } else { work.append((key, run)) }
        guard link == nil else { return }
        if let view = NSApp.windows.first(where: { $0.isVisible })?.contentView {
            link = view.displayLink(target: self, selector: #selector(tick(_:)))
            link?.add(to: .main, forMode: .common)
        } else {
            DispatchQueue.main.async { [weak self] in self?.drain() }
        }
    }

    var isIdle: Bool { work.isEmpty && firsts.isEmpty }

    @objc private func tick(_ link: CADisplayLink) {
        let urgent = firsts
        firsts = []
        urgent.forEach { _ = $0.run() }
        guard !work.isEmpty else {
            link.invalidate()
            self.link = nil
            return
        }
        let next = work.removeFirst()
        let t0 = CACurrentMediaTime()
        let again = next.run()
        if durations.count < 8192 {
            durations.append((CACurrentMediaTime() - t0) * 1000)
            kinds.append(next.key.hasPrefix("maintain") ? "window" : next.key)
        }
        if again { enqueue(next.key, next.run) }
    }

    /// Runs everything now (no window to time frames by).
    func drain() {
        let urgent = firsts
        firsts = []
        urgent.forEach { _ = $0.run() }
        var rounds = 0
        while !work.isEmpty, rounds < 64 {
            let next = work.removeFirst()
            if next.run() { work.append(next) }
            rounds += 1
        }
    }
}

/// One reading tab's window of chapters over its surface.
@MainActor
final class ReadingStream: NSObject {
    let tabID: String
    let workspaceID: String
    private(set) var resourceID: String
    private(set) var started = false
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
    var onSettleReport: (String, UInt32) -> Void = { _, _ in }
    private var lastReport: CFTimeInterval = 0

    /// Reports the verse on the reading line, at most once per frame (a second change in the
    /// same frame is reported on the next one, read again then).
    private func report() {
        let now = CACurrentMediaTime()
        guard now - lastReport >= 1.0 / 90 else {
            ReadingScheduler.shared.enqueue("report-\(ObjectIdentifier(self).hashValue)", first: true) { [weak self] in
                guard let self, self.programmatic == 0, let top = self.topSegment, top != self.reportedTop else { return false }
                self.lastReport = CACurrentMediaTime()
                self.reportedTop = top
                self.onTopSegment(top)
                return false
            }
            return
        }
        guard let top = topSegment else { return }
        lastReport = now
        reportedTop = top
        onTopSegment(top)
    }
    var onSettle: () -> Void = {}

    /// This many blocks when the reader stops: the one in view and two each side. While the
    /// text moves, releases wait (an edit above the reader costs TextKit 2 a relayout of all
    /// the text down to the view, a dropped frame or two), up to `hardMaxBlocks`.
    static let maxBlocks = 5
    static let hardMaxBlocks = 9
    /// The reading line, this far below the sticky header: a typed verse's first line is put
    /// there, a follower's verse too, and the verse there is the tab's position (a sliver of
    /// the verse before shows above it).
    static let topMargin: CGFloat = 24

    private var typography: ReaderTypography = .current()
    private var padding: CGFloat = 24
    private var layoutWidth: CGFloat = 0
    /// The window waits for the column's first width.
    private var needsReplace = false
    /// A block added below is laid out in a frame of its own before the reader reaches it (else
    /// the frame that first shows it lays it all out).
    private var tailNeedsLayout = false
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

    init(tabID: String, workspaceID: String = "", resourceID: String, reading: ReadingHandle) {
        self.tabID = tabID
        self.workspaceID = workspaceID
        self.resourceID = resourceID
        self.reading = reading
        surface = ReaderSurface(paneID: tabID, onSelection: { _ in }, onAction: { _, _ in }, onMeasurement: { _ in })
        super.init()
        surface.revealOverride = { [weak self] range in self?.reveal(range) }
        surface.afterPaint = { [weak self] in self?.ensureExact() }
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
        started = true
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

    /// A fresh window: the chapter and the next. The chapter before comes in the next frame
    /// (ReadingScheduler), anchored, so opening lays out only from the chapter's start.
    private func load(around chapter: ReadingChapter?) {
        guard let chapter else { return }
        var window = [chapter]
        if let next = chapter.next, let block = try? reading.chapterContent(resourceId: resourceID, chapterKey: next) { window.append(block) }
        blocks = window
        relayout(.replace)
    }

    private func jump(to segment: String, flash: Bool) {
        guard let scroll = container?.scrollView, scroll.contentView.bounds.height > 0, !needsReplace, let layout = surface.layout else {
            pendingJump = (segment, flash)
            return
        }
        pendingJump = nil
        let target = layout.segment(holding: segment) ?? segment
        // Enough text on both sides of the target to put it at the top and scroll on.
        let height = scroll.contentView.bounds.height
        while blocks.count < Self.maxBlocks, let next = blocks.last?.next, let top = top(of: target),
              documentHeight - top < height * 1.2, let block = try? reading.chapterContent(resourceId: resourceID, chapterKey: next) {
            blocks.append(block)
            relayout(.append)
        }
        _ = layout
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
        if case .none = change { return }
        // Not laid out before the column has a width (at width 0 every glyph is a line): the
        // first layout of the container does it.
        guard (container?.scrollView.contentView.bounds.width ?? 0) > 0 else {
            needsReplace = true
            return
        }
        needsReplace = false
        // The storage edit resizes the text view, which moves the clip view: not the reader.
        programmatic += 1
        defer { programmatic -= 1 }
        serial += 1
        let t0 = CACurrentMediaTime()
        let built = ReaderLayout(pieces: blocks.map(piece), id: "\(resourceID)|window-\(tabID)-\(serial)", editionId: resourceID,
                                 isRightToLeft: edition.isRightToLeft)
        let t1 = CACurrentMediaTime()
        var t2 = t1
        var kind = "replace"
        defer { Self.timings.append((t1 - t0, t2 - t1, CACurrentMediaTime() - t2, kind)) }
        let edit: ReaderLayoutEdit
        let old = surface.layout?.display.length ?? 0
        switch surface.layout == nil || surface.storage?.length != old ? .replace : change {
        case .none: return
        case .replace: edit = .replace
        case .append: edit = .insert(at: old, length: built.display.length - old)
        case .prepend: edit = .insert(at: 0, length: built.display.length - old)
        case .dropFirst(let range), .dropLast(let range): edit = .delete(range)
        }
        switch edit {
        case .replace: kind = "replace"
        case .insert(let at, _): kind = at == 0 ? "add above" : "add below"
        case .delete(let range): kind = range.location == 0 ? "release above" : "release below"
        }
        surface.replace(layout: built, edit: edit, typographyKey: typography.key, overlay: overlayMarks(in: built))
        t2 = CACurrentMediaTime()
        edits.append(edit)
        // A block added above is laid out now (the verse at the top is placed after it); one
        // added below is laid out as it scrolls into view.
        if case .insert(0, let length) = edit { ensureExact(through: length) } else { ensureExact() }
        if case .insert(let at, _) = edit, at > 0 { tailNeedsLayout = true }
        applyEndRoom()
        // The text view takes its height from what is laid out (and estimated) now.
        surface.textView?.textLayoutManager?.textViewportLayoutController.layoutViewport()
        if Self.timings.count > 4096 { Self.timings.removeFirst(2048) }
        onWindowChanged()
        ShellPersistence.firstText(self)
    }

    /// Blocks laid out once for every stream (a tab switch or a workspace switch shows chapters
    /// already laid out), by edition, typography and chapter; the oldest go past the cap.
    private static var pieces: [String: ReaderBlockPiece] = [:]
    private static var pieceOrder: [String] = []
    static let pieceCap = 96

    private func pieceKey(_ chapterKey: String, revision: String) -> String {
        "\(resourceID)|\(typography.key)|\(edition.isRightToLeft)|\(revision)|\(chapterKey)"
    }

    private func hasPiece(_ chapterKey: String) -> Bool {
        Self.pieces[pieceKey(chapterKey, revision: blocks.first?.contentRevision ?? "")] != nil
    }

    private func piece(_ chapter: ReadingChapter) -> ReaderBlockPiece {
        let key = pieceKey(chapter.chapterKey, revision: chapter.contentRevision)
        if let piece = Self.pieces[key] { return piece }
        let piece = ReaderBlockPiece(chapter: chapter, editionId: resourceID, isRightToLeft: edition.isRightToLeft, typography: typography,
                                     attribution: attribution(chapter))
        Self.pieces[key] = piece
        Self.pieceOrder.append(key)
        if Self.pieceOrder.count > Self.pieceCap {
            for old in Self.pieceOrder.prefix(Self.pieceOrder.count - Self.pieceCap) { Self.pieces[old] = nil }
            Self.pieceOrder.removeFirst(Self.pieceOrder.count - Self.pieceCap)
        }
        return piece
    }

    /// Lays the window out from its start to the end of the view (or `offset`, if further), so
    /// heights above and in view are exact, never estimates: TextKit 2 estimates what it has not
    /// laid out, and an estimate that changes moves the text below it. Text below the view may
    /// stay estimated: nothing the reader sees sits under it.
    func ensureExact(through offset: Int? = nil) {
        guard let textView = surface.textView, let manager = textView.textLayoutManager, let content = manager.textContentManager else { return }
        let start = manager.documentRange.location
        var end = manager.textViewportLayoutController.viewportRange?.endLocation ?? manager.documentRange.endLocation
        if let scroll = container?.scrollView,
           let fragment = manager.textLayoutFragment(for: CGPoint(x: 0, y: scroll.contentView.bounds.maxY - textView.textContainerOrigin.y)),
           fragment.rangeInElement.endLocation.compare(end) == .orderedDescending {
            end = fragment.rangeInElement.endLocation
        }
        if let offset, let location = content.location(start, offsetBy: min(offset, surface.storage?.length ?? 0)),
           location.compare(end) == .orderedDescending {
            end = location
        }
        if let range = NSTextRange(location: start, end: end) { manager.ensureLayout(for: range) }
    }

    private func ensureLayout() {
        ensureExact()
        surface.textView?.textLayoutManager?.textViewportLayoutController.layoutViewport()
    }

    var documentHeight: CGFloat { surface.textView?.frame.height ?? 0 }

    /// Room below an edition's last chapter (80 % of the view, as on the web, or more when the
    /// last verse needs it), so the last verse can reach the reading line; none while more
    /// chapters follow.
    var endRoom: CGFloat {
        guard let last = blocks.last, last.next == nil, let scroll = container?.scrollView else { return 0 }
        let height = scroll.contentView.bounds.height
        var room = height * Self.endRoomShare
        if let id = surface.layout?.segmentOrder.last, let range = surface.layout?.verseRanges[id],
           let box = surface.rect(for: NSRange(location: range.location, length: 1)) {
            room = max(room, height - Self.topMargin - (textHeight - box.minY))
        }
        return room.rounded(.up)
    }
    static let endRoomShare: CGFloat = 0.8

    /// The room as the text view's minimum height past its text.
    private var appliedRoom: CGFloat = 0

    /// The height of the text alone (TextKit 2's usage, with the container's insets).
    private var textHeight: CGFloat {
        guard let textView = surface.textView, let manager = textView.textLayoutManager else { return documentHeight }
        return manager.usageBoundsForTextContainer.maxY + textView.textContainerInset.height * 2
    }

    private func applyEndRoom() {
        guard let textView = surface.textView else { return }
        let text = textHeight
        let room = endRoom
        guard room != appliedRoom || (room > 0 && abs(textView.minSize.height - (text + room)) > 0.5) else { return }
        appliedRoom = room
        textView.minSize = NSSize(width: 0, height: room > 0 ? text + room : 0)
        textView.setFrameSize(NSSize(width: textView.frame.width, height: text + room))
    }

    /// The top of a segment's first line, in the text view's coordinates.
    func top(of segment: String) -> CGFloat? {
        guard let range = surface.layout?.verseRanges[segment] else { return nil }
        ensureExact(through: NSMaxRange(range))
        return surface.rect(for: NSRange(location: range.location, length: min(1, range.length)))?.minY
    }

    /// The top of a block's heading.
    func top(ofBlock key: String) -> CGFloat? {
        guard let block = surface.layout?.blocks.first(where: { $0.chapterKey == key }) else { return nil }
        ensureExact(through: block.range.location + 1)
        return surface.rect(for: NSRange(location: block.range.location, length: 1))?.minY
    }

    private func setOrigin(_ y: CGFloat) {
        guard let scroll = container?.scrollView else { return }
        if y + scroll.contentView.bounds.height > documentHeight {
            surface.textView?.textLayoutManager?.textViewportLayoutController.layoutViewport()
        }
        applyEndRoom()
        let maxY = max(0, documentHeight - scroll.contentView.bounds.height)
        programmatic += 1
        scroll.contentView.setBoundsOrigin(NSPoint(x: 0, y: max(0, min(maxY, y))))
        scroll.reflectScrolledClipView(scroll.contentView)
        programmatic -= 1
    }

    /// Changes the window with the verse at the top kept where it is on screen. `stored`: the
    /// text has already reflowed (a width change), so the anchor is the one recorded before.
    private func anchored(stored: Bool = false, _ change: () -> Void) {
        guard let scroll = container?.scrollView else { return change() }
        // The verse the stream last put or found at the top: a jump's verse stays the top verse
        // (even when its first line starts with the verse before) until the reader scrolls.
        let anchor = topSegment ?? probeTopSegment()
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
        if needsReplace, width > 0 { relayout(.replace) }
        if let pending = pendingJump, scroll.contentView.bounds.height > 0 {
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
        // A block is loaded now only when the reader is close to an end of the window; other
        // loads and releases go one per frame (ReadingScheduler), so a scroll that crosses a
        // chapter in three linked columns does not do three columns' work in one frame.
        if programmatic == 0 { maintain(urgent: true) }
        ReadingScheduler.shared.enqueue("maintain-\(ObjectIdentifier(self).hashValue)") { [weak self] in self?.maintain() ?? false }
        // Only the reader's scrolls move the top verse; the stream's own keep it.
        let top = programmatic == 0 ? probeTopSegment() : nil
        if let top { recordTop(top) }
        updateHeader()
        if let top {
            let changed = top != previous
            if changed, programmatic == 0 { report() }
        }
        if programmatic == 0 {
            settleTimer?.invalidate()
            settleTimer = Timer.scheduledTimer(withTimeInterval: 0.15, repeats: false) { [weak self] _ in
                MainActor.assumeIsolated {
                    guard let self else { return }
                    self.releaseFar()
                    // The settle report: the verse on the reading line and how far into it.
                    if let position = self.probe() {
                        self.reportedTop = position.segment
                        self.onSettleReport(position.segment, position.scalar)
                    }
                    self.onSettle()
                }
            }
        }
    }

    /// Loads a block when the reader nears an end of the window; one change per call, so a fast
    /// scroll spreads the work over frames. Blocks far out of view are released when the
    /// reader stops (`releaseFar`), or, past the hard cap, several at once: an edit above the
    /// reader costs TextKit 2 a relayout down to the view, so it is done as rarely as possible
    /// while the text moves.
    @discardableResult
    func maintain(urgent: Bool = false) -> Bool {
        guard let scroll = container?.scrollView, !blocks.isEmpty, surface.layout != nil, !needsReplace, pendingJump == nil else { return false }
        maintaining = true
        defer { maintaining = false }
        let visible = scroll.contentView.bounds
        let height = max(visible.height, 200)
        let reach = urgent ? 0.75 : 1.5
        let needsBlock = (visible.minY < height * reach && blocks.first?.prev != nil)
            || (documentHeight - visible.maxY < height * reach && blocks.last?.next != nil)
        // At the hard cap, release first (several blocks in one edit), then load.
        if needsBlock, blocks.count >= Self.hardMaxBlocks {
            return trim(to: Self.maxBlocks, margin: height) || trim(to: Self.hardMaxBlocks - 1, margin: 0)
        }
        // A block not laid out yet is laid out in this frame and added in the next.
        if !urgent, tailNeedsLayout {
            tailNeedsLayout = false
            ensureExact(through: surface.storage?.length ?? 0)
            return true
        }
        if !urgent {
            let wanted = visible.minY < height * reach ? blocks.first?.prev : documentHeight - visible.maxY < height * reach ? blocks.last?.next : nil
            if let wanted, !hasPiece(wanted), let block = try? reading.chapterContent(resourceId: resourceID, chapterKey: wanted) {
                _ = piece(block)
                return true
            }
        }
        if visible.minY < height * reach, let prev = blocks.first?.prev,
           let block = try? reading.chapterContent(resourceId: resourceID, chapterKey: prev) {
            anchored {
                blocks.insert(block, at: 0)
                relayout(.prepend)
            }
            return true
        }
        if documentHeight - visible.maxY < height * reach, let next = blocks.last?.next,
           let block = try? reading.chapterContent(resourceId: resourceID, chapterKey: next) {
            blocks.append(block)
            anchored { relayout(.append) }
            return true
        }
        guard blocks.count > Self.hardMaxBlocks else { return false }
        return trim(to: Self.maxBlocks, margin: height)
    }

    /// The reader stopped: blocks more than two screens out of view go, down to the cap.
    func releaseFar() {
        guard let scroll = container?.scrollView, !needsReplace, pendingJump == nil else { return }
        maintaining = true
        defer { maintaining = false }
        trim(to: Self.maxBlocks, margin: max(scroll.contentView.bounds.height, 200) * 2)
    }

    /// Releases blocks at one end, as many as can go in one edit: down to `count` blocks, each
    /// more than `margin` out of view (what stays then still reaches past the load threshold).
    @discardableResult
    private func trim(to count: Int, margin: CGFloat) -> Bool {
        guard blocks.count > count, let scroll = container?.scrollView, let layout = surface.layout, layout.blocks.count == blocks.count else { return false }
        let bounds = scroll.contentView.bounds
        // Where a block starts; below the view an estimate does (no layout for a release).
        func start(_ index: Int) -> CGFloat? { surface.rect(for: NSRange(location: layout.blocks[index].range.location, length: 1))?.minY }
        var front = 0
        while blocks.count - front > count, front + 1 < layout.blocks.count, let next = start(front + 1), next < bounds.minY - margin { front += 1 }
        if front > 0 {
            let keep = layout.blocks[front].range.location
            anchored {
                blocks.removeFirst(front)
                relayout(.dropFirst(NSRange(location: 0, length: keep)))
            }
            return true
        }
        var back = 0
        while blocks.count - back > count, back + 1 < layout.blocks.count, let top = start(layout.blocks.count - 1 - back), top > bounds.maxY + margin { back += 1 }
        if back > 0 {
            let first = layout.blocks[layout.blocks.count - back]
            let from = first.range.location - (layout.content.joiner as NSString).length - (ReaderLayout.blockBreak as NSString).length
            anchored {
                blocks.removeLast(back)
                relayout(.dropLast(NSRange(location: from, length: layout.display.length - from)))
            }
            return true
        }
        return false
    }

    /// The segment whose line is at the top of the visible text: the first line whose bottom
    /// is below the probe (a gap between paragraphs counts as the line after it), read from
    /// TextKit 2's line fragments.
    func probeTopSegment() -> String? { probe()?.segment }

    /// The position on the reading line.
    func probe() -> (segment: String, scalar: UInt32)? {
        guard let scroll = container?.scrollView else { return nil }
        return position(atTextY: scroll.contentView.bounds.minY + Self.topMargin + 4)
    }

    /// The segment whose line is at a height of the text view (or the next line below a gap).
    func segment(atTextY textY: CGFloat) -> String? { position(atTextY: textY)?.segment }

    /// The position on the row at a height (the next row below a gap): the verse that begins on
    /// that row, else the verse running through it with the Unicode scalar offset of the row's
    /// first character in its text. Short verses share rows, so "the verse at the row's start"
    /// would give the tail of the verse before; with this rule a follower put at a verse reads
    /// that verse back.
    func position(atTextY textY: CGFloat) -> (segment: String, scalar: UInt32)? {
        guard let textView = surface.textView, let layout = surface.layout, layout.display.length > 0,
              let manager = textView.textLayoutManager, let content = manager.textContentManager else { return nil }
        let y = textY - textView.textContainerOrigin.y
        var row: NSRange?
        let start = manager.textLayoutFragment(for: CGPoint(x: 0, y: max(0, y)))?.rangeInElement.location ?? manager.documentRange.location
        manager.enumerateTextLayoutFragments(from: start, options: []) { fragment in
            let frame = fragment.layoutFragmentFrame
            guard frame.maxY > y else { return true }
            let base = content.offset(from: content.documentRange.location, to: fragment.rangeInElement.location)
            if let line = fragment.textLineFragments.first(where: { $0.typographicBounds.maxY + frame.minY > y }) ?? fragment.textLineFragments.last {
                row = NSRange(location: base + line.characterRange.location, length: line.characterRange.length)
            }
            return false
        }
        guard let row else { return layout.segment(at: layout.display.length - 1).map { ($0, 0) } }
        // A heading row: the block's first verse.
        if layout.display.attribute(.openLogosHeading, at: min(row.location, layout.display.length - 1), effectiveRange: nil) != nil {
            return layout.segment(at: row.location).map { ($0, 0) }
        }
        if let begins = layout.segmentOrder.first(where: { id in
            guard let range = layout.verseRanges[id] else { return false }
            return range.location >= row.location && range.location < NSMaxRange(row)
        }) {
            return (begins, 0)
        }
        guard let through = layout.segment(at: row.location), let offset = layout.segmentOffsets[through], let text = layout.segmentTexts[through] else { return nil }
        let into = max(0, layout.runMap.coreOffset(fromDisplay: row.location) - offset)
        let prefix = (text as NSString).substring(to: min(into, (text as NSString).length))
        return (through, UInt32(prefix.unicodeScalars.count))
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

    /// A verse's first character (its number) on the ink canvas, when it is in the window.
    func canvasBox(of segment: String) -> CGRect? {
        guard let range = surface.layout?.verseRanges[segment] else { return nil }
        ensureExact(through: NSMaxRange(range))
        return surface.rect(for: NSRange(location: range.location, length: min(2, range.length))).map { $0.offsetBy(dx: 0, dy: -canvasShift) }
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

    /// B074: the core's highlights on the window's chapters (the model answers), painted
    /// under the typed verses' mark.
    var storedMarks: (String, ReaderLayout) -> [ReaderMark] = { _, _ in [] }
    /// After every window change (the model shows the core's ink on the new chapters).
    var onWindowChanged: () -> Void = {}

    /// The highlights painted again (one was added).
    func repaintMarks() { paintHighlight() }

    /// The text's left edge on the ink canvas: stored strokes measure `x` from it.
    var inkLeading: CGFloat {
        guard let textView = surface.textView else { return padding }
        return textView.textContainerOrigin.x + (textView.textContainer?.lineFragmentPadding ?? 0)
    }

    /// The stored highlights, then the typed verses from `verse_start` to `verse_end` in
    /// document order; a range that runs past an end of the window is marked up to that end.
    private func overlayMarks(in layout: ReaderLayout) -> [ReaderMark] {
        storedMarks(resourceID, layout) + typedMarks(in: layout)
    }

    private func typedMarks(in layout: ReaderLayout) -> [ReaderMark] {
        guard let highlight else { return [] }
        let order = layout.segmentOrder
        let startIndex = order.firstIndex(of: highlight.start), endIndex = order.lastIndex(of: highlight.end)
        guard startIndex != nil || endIndex != nil else { return [] }
        let start = startIndex ?? 0, end = endIndex ?? order.count - 1
        guard start <= end, let range = layout.displayRange(from: order[start], to: order[end]) else { return [] }
        let accent = PlatformColor(hex: theme.accent)
        // The mark: B072's underline on each verse's text; the flash: the accent behind them all.
        let verses = order[start...end].compactMap { id in layout.segmentOffsets[id].map { NSRange(location: $0, length: ((layout.segmentTexts[id] ?? "") as NSString).length) } }
            .map(layout.runMap.displayRange(fromCore:))
        var marks = [ReaderMark(id: "typed", ranges: verses, color: accent, underline: accent.withAlphaComponent(Self.markAlpha))]
        if flashLevel > 0 { marks.insert(ReaderMark(id: "flash", ranges: [range], color: accent.withAlphaComponent(flashLevel)), at: 0) }
        return marks
    }

    /// Each window edit's parts (layout from blocks, storage edit and paint, TextKit 2 layout)
    /// and its kind, for the measurement.
    static var timings: [(Double, Double, Double, String)] = []
    static let markAlpha: CGFloat = 0.6
    static let flashPeak: CGFloat = 0.3
    static let flashSeconds = 1.2

    private func paintHighlight() {
        guard let layout = surface.layout else { return }
        surface.setOverlay(overlayMarks(in: layout))
    }

    /// The typed verses behind the accent at 30 %, fading out over 1.2 s, once.
    func flash() {
        flashTimer?.invalidate()
        flashLevel = Self.flashPeak
        paintHighlight()
        let start = CACurrentMediaTime()
        flashTimer = Timer.scheduledTimer(withTimeInterval: 0.075, repeats: true) { [weak self] timer in
            MainActor.assumeIsolated {
                guard let self else { return timer.invalidate() }
                let t = min(1, (CACurrentMediaTime() - start) / Self.flashSeconds)
                let eased = 1 - pow(1 - t, 2)
                self.flashLevel = Self.flashPeak * CGFloat(1 - eased)
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
        paintHighlight()
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
        applyEndRoom()
        let maxY = max(0, documentHeight - clip.bounds.height)
        clip.setBoundsOrigin(NSPoint(x: 0, y: max(0, min(maxY, clip.bounds.minY + delta))))
        scroll.reflectScrolledClipView(clip)
    }

    /// The verses in the window and the text storage's length (the DOM-cap row of the evidence).
    var renderedVerses: Int { surface.layout?.segmentOrder.count ?? 0 }
}
#endif
