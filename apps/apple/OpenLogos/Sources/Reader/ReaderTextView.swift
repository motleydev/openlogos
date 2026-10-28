// The TextKit 2 reading surface (B005, docs/adr/0001-rust-core-native-ui.md): a UITextView on
// iPadOS/iOS and an NSTextView on macOS, created with NSTextLayoutManager so paragraphs are laid
// out per viewport, read-only and selectable with the platform's own handles and edit menu.
// The surface shows a ReaderLayout, reports every selection change through AnchorRoundTrip
// (display UTF-16 → core anchors → display UTF-16) and paints the ranges the core hands back as
// background and underline attributes of the text storage. The project actions (specs/21
// §3: Highlight, Collect, Study, More) are added to the native edit menu as a labelled group;
// nothing of the system menu is replaced. A VoiceOver rotor "Verses" walks the verse markers.
// Programmatic commands (select a core or display range, a phrase, scroll, measure) drive the
// same path the handles do, so the evidence is reproducible.
import SwiftUI
import os

#if canImport(UIKit)
import UIKit
typealias PlatformTextView = UITextView
#else
import AppKit
typealias PlatformTextView = NSTextView
#endif

/// The project actions the edit menu and the action strip offer (specs/21 §3).
enum SelectionAction: String, CaseIterable {
    case highlight = "Highlight", collect = "Collect", study = "Study"
    case copyWithReference = "Copy with reference", openInPane = "Open in pane", searchForThis = "Search for this"
    case shortObservation = "Short observation…", compareRenderings = "Compare renderings"

    var symbol: String {
        switch self {
        case .highlight: return "highlighter"
        case .collect: return "tray.and.arrow.down"
        case .study: return "text.magnifyingglass"
        case .copyWithReference: return "doc.on.doc"
        case .openInPane: return "rectangle.split.2x1"
        case .searchForThis: return "magnifyingglass"
        case .shortObservation: return "text.bubble"
        case .compareRenderings: return "rectangle.split.3x1"
        }
    }

    static let primary: [SelectionAction] = [.highlight, .collect, .study]
    static let more: [SelectionAction] = [.copyWithReference, .openInPane, .searchForThis, .shortObservation, .compareRenderings]
}

/// Something the host asks the surface to do; a new id is a new command.
struct ReaderCommand: Equatable {
    enum Kind: Equatable {
        case selectCore(NSRange)
        case selectDisplay(NSRange)
        case selectPhrase(String)
        case runCase(String)
        case clearSelection
        case scrollTo(fraction: CGFloat)
        /// The B005 measurement: scroll frame times and selection latency, then a summary.
        case measure
    }

    let id: Int
    let kind: Kind

    private static var counter = 0
    static func make(_ kind: Kind) -> ReaderCommand {
        counter += 1
        return ReaderCommand(id: counter, kind: kind)
    }
}

/// The SwiftUI face of the surface.
@MainActor
struct ReaderTextView {
    let paneID: String
    let content: ReaderContent
    let horizontalPadding: CGFloat
    var command: ReaderCommand?
    var onSelection: (ReaderSelection?) -> Void = { _ in }
    var onAction: (SelectionAction, ReaderSelection?) -> Void = { _, _ in }
    var onMeasurement: (String) -> Void = { _ in }

    func makeCoordinator() -> ReaderSurface {
        ReaderSurface(paneID: paneID, onSelection: onSelection, onAction: onAction, onMeasurement: onMeasurement)
    }

    func configure(_ surface: ReaderSurface) {
        surface.onSelection = onSelection
        surface.onAction = onAction
        surface.onMeasurement = onMeasurement
        surface.apply(content: content, padding: horizontalPadding)
        if let command { surface.perform(command) }
    }
}

#if canImport(UIKit)
extension ReaderTextView: UIViewRepresentable {
    func makeUIView(context: Context) -> UITextView {
        let view = context.coordinator.makeTextView()
        configure(context.coordinator)
        return view
    }

    func updateUIView(_ uiView: UITextView, context: Context) {
        configure(context.coordinator)
    }
}
#else
extension ReaderTextView: NSViewRepresentable {
    func makeNSView(context: Context) -> NSScrollView {
        let view = context.coordinator.makeScrollView()
        configure(context.coordinator)
        return view
    }

    func updateNSView(_ nsView: NSScrollView, context: Context) {
        configure(context.coordinator)
    }
}
#endif

// MARK: - The surface

/// Owns the text view, the layout and the round trip; the SwiftUI coordinator.
@MainActor
final class ReaderSurface: NSObject {
    let paneID: String
    var onSelection: (ReaderSelection?) -> Void
    var onAction: (SelectionAction, ReaderSelection?) -> Void
    var onMeasurement: (String) -> Void

    private(set) var layout: ReaderLayout?
    private(set) var textView: PlatformTextView?
    #if !canImport(UIKit)
    private(set) var scrollView: NSScrollView?
    #endif
    /// The last selection reported, nil when there is none.
    private(set) var current: ReaderSelection?
    /// The last error the core raised, for the readout.
    private(set) var lastError: String?
    private var lastCommandID = -1
    private var padding: CGFloat = 20
    private var measuring: ReaderMeasurement?
    private var contentObserver: NSObjectProtocol?

    init(paneID: String,
         onSelection: @escaping (ReaderSelection?) -> Void,
         onAction: @escaping (SelectionAction, ReaderSelection?) -> Void,
         onMeasurement: @escaping (String) -> Void) {
        self.paneID = paneID
        self.onSelection = onSelection
        self.onAction = onAction
        self.onMeasurement = onMeasurement
        super.init()
    }

    /// Whether the text view runs on TextKit 2 (an NSTextLayoutManager), the premise of B005.
    var usesTextKit2: Bool { textView?.textLayoutManager != nil }

    // MARK: Content

    func apply(content: ReaderContent, padding: CGFloat) {
        let samePadding = padding == self.padding
        self.padding = padding
        if let layout, layout.content == content, samePadding { return }
        let built = ReaderLayout(content: content)
        layout = built
        current = nil
        lastError = nil
        guard let textView else { return }
        #if canImport(UIKit)
        textView.textStorage.setAttributedString(built.display)
        #else
        textView.textStorage?.setAttributedString(built.display)
        #endif
        applyPadding()
        #if canImport(UIKit)
        textView.accessibilityIdentifier = "reader-\(paneID)"
        textView.accessibilityLabel = "\(content.editionId) text"
        installRotor()
        #else
        textView.setAccessibilityIdentifier("reader-\(paneID)")
        textView.setAccessibilityLabel("\(content.editionId) text")
        #endif
        clearHighlights()
    }

    /// Rebuilds the layout with the current Dynamic Type fonts (iOS content size changes).
    private func rebuildForTypography() {
        guard let content = layout?.content else { return }
        layout = nil
        apply(content: content, padding: padding)
    }

    private func applyPadding() {
        #if canImport(UIKit)
        textView?.textContainerInset = UIEdgeInsets(top: 20, left: padding, bottom: 24, right: padding)
        #else
        textView?.textContainerInset = NSSize(width: padding, height: 20)
        #endif
    }

    // MARK: Commands

    func perform(_ command: ReaderCommand) {
        guard command.id != lastCommandID else { return }
        lastCommandID = command.id
        guard let layout else { return }
        switch command.kind {
        case .selectCore(let range):
            select(display: layout.runMap.displayRange(fromCore: range))
        case .selectDisplay(let range):
            select(display: range)
        case .selectPhrase(let phrase):
            if let range = layout.coreRange(ofPhrase: phrase) { select(display: layout.runMap.displayRange(fromCore: range)) }
        case .runCase(let name):
            if let anchorCase = AnchorCase.named(name), anchorCase.paneID == paneID, let range = anchorCase.displayRange(in: layout) {
                select(display: range)
            }
        case .clearSelection:
            setSelectedRange(NSRange(location: 0, length: 0))
            selectionChanged()
        case .scrollTo(let fraction):
            scroll(toFraction: fraction)
        case .measure:
            measure()
        }
    }

    /// Puts the platform selection on a display range and runs the round trip, as a drag of
    /// the handles would. The text view may itself adjust the range; what it kept is used.
    func select(display range: NSRange) {
        guard let textView else { return }
        // Make sure the range is laid out before selecting it so the handles land on glyphs.
        scrollRangeToVisible(range)
        setSelectedRange(range)
        _ = textView
        selectionChanged()
    }

    private func setSelectedRange(_ range: NSRange) {
        #if canImport(UIKit)
        textView?.selectedRange = range
        #else
        textView?.setSelectedRange(range)
        #endif
    }

    var selectedRange: NSRange {
        #if canImport(UIKit)
        return textView?.selectedRange ?? NSRange(location: 0, length: 0)
        #else
        return textView?.selectedRange() ?? NSRange(location: 0, length: 0)
        #endif
    }

    // MARK: The round trip

    /// Called for every selection change, from the handles or from `select(display:)`.
    func selectionChanged() {
        guard let layout else { return }
        let range = selectedRange
        do {
            let selection = try AnchorRoundTrip.run(layout: layout, paneID: paneID, displayRange: range)
            lastError = nil
            if selection != current {
                current = selection
                paint(selection)
                onSelection(selection)
            }
        } catch {
            lastError = "\(error)"
            current = nil
            clearHighlights()
            onSelection(nil)
        }
    }

    // MARK: Highlights

    /// Paints the ranges the core handed back as background and underline attributes of the
    /// text storage (a real attribute, so UITextView and NSTextView both draw it; the
    /// rendering-attribute route of NSTextLayoutManager was tried first and is not drawn by
    /// either view for backgroundColor, see docs/evidence/B005.md). The ranges never cover a
    /// verse number, so the markers keep their own attributes.
    func paint(_ selection: ReaderSelection?) {
        clearHighlights()
        guard let selection, let storage = storage else { return }
        storage.beginEditing()
        for range in selection.highlights where range.location + range.length <= storage.length {
            storage.addAttributes([
                .backgroundColor: PlatformColor.readerHighlight,
                .underlineStyle: NSUnderlineStyle.single.rawValue,
                .underlineColor: PlatformColor.systemOrange,
            ], range: range)
        }
        storage.endEditing()
    }

    private var storage: NSTextStorage? {
        #if canImport(UIKit)
        return textView?.textStorage
        #else
        return textView?.textStorage
        #endif
    }

    private func clearHighlights() {
        guard let storage, storage.length > 0 else { return }
        let all = NSRange(location: 0, length: storage.length)
        storage.beginEditing()
        storage.removeAttribute(.backgroundColor, range: all)
        storage.removeAttribute(.underlineStyle, range: all)
        storage.removeAttribute(.underlineColor, range: all)
        storage.endEditing()
    }

    // MARK: Scrolling

    var contentOffsetY: CGFloat {
        get {
            #if canImport(UIKit)
            return textView?.contentOffset.y ?? 0
            #else
            return scrollView?.contentView.bounds.origin.y ?? 0
            #endif
        }
        set {
            #if canImport(UIKit)
            textView?.contentOffset = CGPoint(x: 0, y: newValue)
            #else
            guard let scrollView else { return }
            scrollView.contentView.scroll(to: CGPoint(x: 0, y: newValue))
            scrollView.reflectScrolledClipView(scrollView.contentView)
            #endif
        }
    }

    var maximumContentOffsetY: CGFloat {
        #if canImport(UIKit)
        guard let textView else { return 0 }
        return max(0, textView.contentSize.height - textView.bounds.height)
        #else
        guard let scrollView, let document = scrollView.documentView else { return 0 }
        return max(0, document.frame.height - scrollView.contentView.bounds.height)
        #endif
    }

    func scroll(toFraction fraction: CGFloat) {
        contentOffsetY = maximumContentOffsetY * max(0, min(1, fraction))
    }

    private func scrollRangeToVisible(_ range: NSRange) {
        #if canImport(UIKit)
        textView?.scrollRangeToVisible(range)
        #else
        textView?.scrollRangeToVisible(range)
        #endif
    }

    // MARK: Measurement

    private func measure() {
        guard measuring == nil, let textView else { return }
        let run = ReaderMeasurement(surface: self, view: textView) { [weak self] summary in
            self?.measuring = nil
            self?.onMeasurement(summary)
        }
        measuring = run
        run.start()
    }

    // MARK: Edit menu actions

    func trigger(_ action: SelectionAction) {
        onAction(action, current)
    }
}

// MARK: - UIKit

#if canImport(UIKit)
extension ReaderSurface: UITextViewDelegate {
    func makeTextView() -> UITextView {
        let view = ReaderUITextView(usingTextLayoutManager: true)
        view.surface = self
        view.isEditable = false
        view.isSelectable = true
        view.isScrollEnabled = true
        view.alwaysBounceVertical = true
        view.backgroundColor = .clear
        view.textContainer.lineFragmentPadding = 0
        view.adjustsFontForContentSizeCategory = false // the layout is rebuilt instead
        view.delegate = self
        view.linkTextAttributes = [:]
        textView = view
        contentObserver = NotificationCenter.default.addObserver(
            forName: UIContentSizeCategory.didChangeNotification, object: nil, queue: .main
        ) { [weak self] _ in
            Task { @MainActor in self?.rebuildForTypography() }
        }
        return view
    }

    func textViewDidChangeSelection(_ textView: UITextView) {
        selectionChanged()
    }

    /// specs/21 §3: the project actions as a labelled group ahead of the system items.
    func textView(_ textView: UITextView, editMenuForTextIn range: NSRange, suggestedActions: [UIMenuElement]) -> UIMenu? {
        guard range.length > 0 else { return UIMenu(children: suggestedActions) }
        let primary = SelectionAction.primary.map { action in
            UIAction(title: action.rawValue, image: UIImage(systemName: action.symbol)) { [weak self] _ in self?.trigger(action) }
        }
        let more = UIMenu(title: "More", image: UIImage(systemName: "ellipsis"), children: SelectionAction.more.map { action in
            UIAction(title: action.rawValue, image: UIImage(systemName: action.symbol)) { [weak self] _ in self?.trigger(action) }
        })
        let group = UIMenu(title: "OpenLogos", options: .displayInline, children: primary + [more])
        return UIMenu(children: [group] + suggestedActions)
    }

    /// A VoiceOver rotor that moves between verse markers, each announced as "verse n".
    private func installRotor() {
        guard let textView, let layout else { return }
        let markers = layout.markers
        guard !markers.isEmpty else {
            textView.accessibilityCustomRotors = []
            return
        }
        let rotor = UIAccessibilityCustomRotor(name: "Verses") { [weak textView] predicate in
            guard let textView else { return nil }
            let forward = predicate.searchDirection == .next
            var currentOffset = forward ? -1 : Int.max
            if let range = predicate.currentItem.targetRange {
                currentOffset = textView.offset(from: textView.beginningOfDocument, to: range.start)
            }
            let candidates = forward
                ? markers.first { $0.range.location > currentOffset }
                : markers.last { $0.range.location < currentOffset }
            guard let next = candidates,
                  let start = textView.position(from: textView.beginningOfDocument, offset: next.range.location),
                  let end = textView.position(from: start, offset: next.range.length),
                  let textRange = textView.textRange(from: start, to: end)
            else { return nil }
            return UIAccessibilityCustomRotorItemResult(targetElement: textView, targetRange: textRange)
        }
        textView.accessibilityCustomRotors = [rotor]
    }
}

/// The UITextView subclass: keeps a reference to its surface for the edit menu and the tests.
final class ReaderUITextView: UITextView {
    weak var surface: ReaderSurface?
}

#else

// MARK: - AppKit

extension ReaderSurface: NSTextViewDelegate {
    func makeScrollView() -> NSScrollView {
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = false
        scroll.drawsBackground = false
        scroll.autohidesScrollers = true

        let view = ReaderNSTextView(usingTextLayoutManager: true)
        view.surface = self
        view.isEditable = false
        view.isSelectable = true
        view.isRichText = true
        view.drawsBackground = false
        view.allowsUndo = false
        view.isVerticallyResizable = true
        view.isHorizontallyResizable = false
        view.autoresizingMask = [.width]
        view.minSize = NSSize(width: 0, height: 0)
        view.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        view.textContainer?.widthTracksTextView = true
        view.textContainer?.lineFragmentPadding = 0
        view.textContainer?.containerSize = NSSize(width: 0, height: CGFloat.greatestFiniteMagnitude)
        view.delegate = self
        scroll.documentView = view
        textView = view
        scrollView = scroll
        return scroll
    }

    func textViewDidChangeSelection(_ notification: Notification) {
        selectionChanged()
    }

    /// specs/21 §3 on the Mac: the project actions at the top of the text view's context menu.
    func menu(for view: NSTextView, suggested menu: NSMenu) -> NSMenu {
        guard view.selectedRange().length > 0 else { return menu }
        var index = 0
        for action in SelectionAction.primary {
            let item = NSMenuItem(title: action.rawValue, action: #selector(ReaderNSTextView.projectAction(_:)), keyEquivalent: "")
            item.target = view
            item.representedObject = action.rawValue
            item.image = NSImage(systemSymbolName: action.symbol, accessibilityDescription: action.rawValue)
            menu.insertItem(item, at: index)
            index += 1
        }
        let more = NSMenuItem(title: "More", action: nil, keyEquivalent: "")
        let submenu = NSMenu(title: "More")
        for action in SelectionAction.more {
            let item = NSMenuItem(title: action.rawValue, action: #selector(ReaderNSTextView.projectAction(_:)), keyEquivalent: "")
            item.target = view
            item.representedObject = action.rawValue
            submenu.addItem(item)
        }
        more.submenu = submenu
        menu.insertItem(more, at: index)
        menu.insertItem(NSMenuItem.separator(), at: index + 1)
        return menu
    }
}

/// The NSTextView subclass: the context menu with the project group and the surface reference.
final class ReaderNSTextView: NSTextView {
    weak var surface: ReaderSurface?

    override func menu(for event: NSEvent) -> NSMenu? {
        guard let menu = super.menu(for: event) else { return nil }
        return surface?.menu(for: self, suggested: menu) ?? menu
    }

    @objc func projectAction(_ sender: NSMenuItem) {
        guard let raw = sender.representedObject as? String, let action = SelectionAction(rawValue: raw) else { return }
        surface?.trigger(action)
    }
}
#endif

// MARK: - Measurement

/// The B005 numbers (docs/evidence/B005.md, acceptance 3): frame intervals while the surface is
/// scrolled by a display link at a steady pace through the whole set, and the latency of
/// programmatic selections spread over the document, each through the full round trip and
/// paint. Instruments-free: CADisplayLink timestamps and CFAbsoluteTime around the pipeline;
/// the same intervals are also emitted as os_signpost intervals for Instruments.
@MainActor
final class ReaderMeasurement {
    private weak var surface: ReaderSurface?
    private let completion: (String) -> Void
    private var link: CADisplayLink?
    private var lastTimestamp: CFTimeInterval = 0
    private var frameIntervals: [Double] = []
    private var phase = 0
    private var ticksInPhase = 0
    private var selectionIndex = 0
    private var pipelineMs: [Double] = []
    private var toNextFrameMs: [Double] = []
    private var selectionSet: CFAbsoluteTime = 0
    private let scrollTicks = 480 // 8 s at 60 Hz
    private let stepPoints: CGFloat = 12 // 720 pt/s at 60 Hz
    private let selections = 30
    private let signposter = AnchorRoundTrip.signposter
    private var scrollState: OSSignpostIntervalState?
    private let width: CGFloat

    init(surface: ReaderSurface, view: PlatformTextView, completion: @escaping (String) -> Void) {
        self.surface = surface
        self.completion = completion
        width = view.bounds.width
    }

    func start() {
        guard let view = surface?.textView else { return }
        surface?.contentOffsetY = 0
        #if canImport(UIKit)
        link = CADisplayLink(target: self, selector: #selector(tick(_:)))
        link?.add(to: .main, forMode: .common)
        #else
        link = view.displayLink(target: self, selector: #selector(tick(_:)))
        link?.add(to: .main, forMode: .common)
        #endif
        scrollState = signposter.beginInterval("scroll-measurement")
    }

    @objc private func tick(_ link: CADisplayLink) {
        guard let surface else { return finish() }
        let now = link.timestamp
        if lastTimestamp > 0 { frameIntervals.append((now - lastTimestamp) * 1000) }
        lastTimestamp = now
        ticksInPhase += 1
        switch phase {
        case 0:
            // Scroll: a steady pace, wrapping to the top when the end is reached.
            let maximum = surface.maximumContentOffsetY
            var offset = surface.contentOffsetY + stepPoints
            if offset > maximum { offset = 0 }
            surface.contentOffsetY = offset
            if ticksInPhase >= scrollTicks {
                if let scrollState { signposter.endInterval("scroll-measurement", scrollState) }
                phase = 1
                ticksInPhase = 0
                frameIntervals.removeFirst(min(5, frameIntervals.count)) // the first frames include the link start
                scrollFrames = frameIntervals
                frameIntervals = []
            }
        case 1:
            // Selections: one every 6 frames, spread over the document; the frame after the
            // selection is the paint.
            if selectionSet > 0 {
                toNextFrameMs.append((CFAbsoluteTimeGetCurrent() - selectionSet) * 1000)
                selectionSet = 0
            }
            if ticksInPhase % 6 == 1 {
                guard selectionIndex < selections, let layout = surface.layout else {
                    phase = 2
                    return
                }
                let core = layout.coreText as NSString
                let step = core.length / (selections + 1)
                let start = step * (selectionIndex + 1)
                // About forty units, widened by the surface to clusters and clipped to the text.
                let range = NSRange(location: start, length: min(40, core.length - start))
                let display = layout.runMap.displayRange(fromCore: range)
                let t0 = CFAbsoluteTimeGetCurrent()
                surface.select(display: display)
                let elapsed = (CFAbsoluteTimeGetCurrent() - t0) * 1000
                pipelineMs.append(elapsed)
                selectionSet = t0
                selectionIndex += 1
            }
        default:
            finish()
        }
    }

    private var scrollFrames: [Double] = []

    private func finish() {
        link?.invalidate()
        link = nil
        guard let surface else { return }
        surface.perform(ReaderCommand.make(.clearSelection))
        let frames = scrollFrames
        func percentile(_ values: [Double], _ p: Double) -> Double {
            guard !values.isEmpty else { return 0 }
            let sorted = values.sorted()
            let index = min(sorted.count - 1, Int((Double(sorted.count - 1) * p).rounded()))
            return sorted[index]
        }
        let over17 = frames.filter { $0 > 17.5 }.count
        let over34 = frames.filter { $0 > 34 }.count
        let verses = surface.layout?.content.verses.count ?? 0
        let result: [String: Any] = [
            "pane": surface.paneID,
            "width_pt": Double(width),
            "verses": verses,
            "textkit2": surface.usesTextKit2,
            "scroll_frames": frames.count,
            "scroll_ms_p50": percentile(frames, 0.5),
            "scroll_ms_p95": percentile(frames, 0.95),
            "scroll_ms_max": frames.max() ?? 0,
            "scroll_frames_over_17ms": over17,
            "scroll_frames_over_34ms": over34,
            "selections": pipelineMs.count,
            "select_ms_p50": percentile(pipelineMs, 0.5),
            "select_ms_p95": percentile(pipelineMs, 0.95),
            "select_ms_max": pipelineMs.max() ?? 0,
            "select_to_next_frame_ms_p50": percentile(toNextFrameMs, 0.5),
            "select_to_next_frame_ms_p95": percentile(toNextFrameMs, 0.95),
            "select_to_next_frame_ms_max": toNextFrameMs.max() ?? 0,
        ]
        RoundTripLog.append(result, to: "b005-perf.jsonl")
        let summary = String(
            format: "%d verses at %.0f pt, TextKit 2 %@ · scroll %d frames: p50 %.1f ms, p95 %.1f ms, max %.1f ms, %d over 17.5 ms, %d over 34 ms · select (set → anchors → paint) %d runs: p50 %.2f ms, p95 %.2f ms, max %.2f ms; to next frame p50 %.1f ms, p95 %.1f ms",
            verses, Double(width), surface.usesTextKit2 ? "yes" : "NO",
            frames.count, percentile(frames, 0.5), percentile(frames, 0.95), frames.max() ?? 0, over17, over34,
            pipelineMs.count, percentile(pipelineMs, 0.5), percentile(pipelineMs, 0.95), pipelineMs.max() ?? 0,
            percentile(toNextFrameMs, 0.5), percentile(toNextFrameMs, 0.95)
        )
        completion(summary)
    }
}
