// B072: the specs/22 §7 interactions measured in the running Mac app. One interaction per
// display-link frame: the timer starts before the command (or the keystroke), the model
// changes, then the window lays out and draws synchronously (`layoutSubtreeIfNeeded` and
// `displayIfNeeded` on the hosting view), and the timer stops: the host's whole cost of the
// frame, core call included. `to_next_frame` is the time from the start to the next display
// link callback, so a sample over one refresh interval shows as a missed frame. The session
// is ten workspaces × three columns × four tabs (ShellSeed.measurementJSON).
import QuartzCore
import SwiftUI
#if os(macOS)
import AppKit
#endif

@MainActor
final class ShellMeasure {
    struct Stat: Codable {
        let interaction: String
        let budgetMs: Double
        let samples: Int
        let p50Ms: Double
        let p95Ms: Double
        let maxMs: Double
        let nextFrameP50Ms: Double
        let nextFrameP95Ms: Double
    }

    struct Report: Codable {
        let host: String
        let window: String
        let workspaces: Int
        let tabs: Int
        let stats: [Stat]
        /// B078: the scripted continuous scrolls.
        var scrolls: [ScrollRun] = []
    }

    /// B078: one continuous scroll through chapters at a steady pace, one step per frame.
    struct ScrollRun: Codable {
        let pointsPerFrame: Double
        let chapters: Int
        let frames: Int
        let frameP50Ms: Double
        let frameP95Ms: Double
        let frameMaxMs: Double
        let framesOver17Ms: Int
        let framesOver34Ms: Int
        /// The handler and the drawn frame: steps without a window edit, and steps with one.
        let stepP50Ms: Double
        let stepP95Ms: Double
        let editSteps: Int
        let editStepP50Ms: Double
        let editStepP95Ms: Double
        let editStepMaxMs: Double
        /// Steps that moved less than asked (the window did not keep up).
        let stalls: Int
        let maxBlocks: Int
        let maxVerses: Int
        let maxStorageUnits: Int
        let residentMBStart: Double
        let residentMBEnd: Double
        /// Frames over 34 ms: the frame, the scroll step in it and the deferred work since the
        /// frame before (what made it long).
        var slowFrames: [String] = []
    }

    let model: ShellModel
    private let samplesPerInteraction: Int

    init(model: ShellModel, samples: Int = 240) {
        self.model = model
        samplesPerInteraction = samples
    }

    /// Runs every interaction; about thirty seconds at 60 Hz.
    func run() async -> Report {
        var stats: [Stat] = []
        let words = ["Jn 3:16", "new tab", "@workspace 3", "move tab to column 3", "gen 1", ">focus column", "#loved"]
        var keystrokes: [String] = []
        for word in words { for i in 1...word.count { keystrokes.append(String(word.prefix(i))) } }
        model.focusPalette()
        stats.append(await measure("palette keystroke (passage, commands, tabs, fixture text)", budget: 16, count: max(samplesPerInteraction, keystrokes.count)) { i in
            self.model.paletteText = keystrokes[i % keystrokes.count]
            self.model.paletteTextChanged()
        })
        model.closePalette()
        stats.append(await measure("tab switch within a column (tab.next)", budget: 16) { _ in
            self.model.run("tab.next")
        })
        stats.append(await measure("column focus (column.focus N)", budget: 16) { i in
            self.model.run("column.focus", args: ShellCommandArgs(column: UInt32(i % 3 + 1)))
        })
        stats.append(await measure("workspace switch (workspace.next, 10 × 3 × 4)", budget: 100) { _ in
            self.model.run("workspace.next")
        })
        let tools = ["tool.highlight", "tool.pen", "tool.highlighter_pen", "tool.eraser", "tool.collect", "tool.select"]
        stats.append(await measure("tool change (tool.*)", budget: 16) { i in
            self.model.run(tools[i % tools.count])
        })
        // B076: the well follows focus. Column 1 shows SBLGNT (original), column 2 BSB
        // (translation) in the measurement session.
        stats.append(await measure("column focus, original ↔ translation (the well changes)", budget: 16) { i in
            self.model.run("column.focus", args: ShellCommandArgs(column: UInt32(i % 2 + 1)))
        })
        model.run("column.focus", args: ShellCommandArgs(column: 2))
        model.run("view.notes_dock")
        Self.flush()
        stats.append(await measure("translation → notes dock (the well changes)", budget: 16, prepare: { _ in self.model.setFocus(.tab) }) { _ in
            self.model.setFocus(.notesDock)
        })
        model.setFocus(.tab)
        model.run("view.notes_dock")
        model.run("column.focus", args: ShellCommandArgs(column: 1))
        Self.flush()
        if let surface = model.surface(for: model.activeColumn.shownTabId), let layout = surface.layout {
            // Visible text: the start of the verse at the top (B078: a window starts chapters
            // above the reader).
            var core = NSRange(location: 0, length: 12)
            #if os(macOS)
            if let top = model.stream(for: model.activeColumn.shownTabId)?.topSegment, let offset = layout.segmentOffsets[top] { core.location = offset }
            #endif
            let range = layout.runMap.displayRange(fromCore: core)
            stats.append(await measure("selection appears → selection group drawn", budget: 16, prepare: { _ in surface.clearSelection() }) { _ in
                surface.select(display: range)
            })
            surface.clearSelection()
        }
        model.run("column.focus", args: ShellCommandArgs(column: 1))
        stats.append(await measure("open a chapter in a tab, warm (nav.navigate, group follows)", budget: 100) { i in
            self.model.run("nav.navigate", args: ShellCommandArgs(reference: i % 2 == 0 ? "John.3.17" : "John.3.16"))
        })
        #if os(macOS)
        stats += await measureContextMenu()
        let (reading, scrolls) = await measureReading()
        stats += reading
        #else
        let scrolls: [ScrollRun] = []
        #endif
        let size = Self.windowSize
        let tabs = model.session.workspaces.reduce(0) { $0 + $1.columns.reduce(0) { $0 + $1.tabs.count } }
        return Report(host: "mac", window: "\(Int(size.width))x\(Int(size.height))", workspaces: model.session.workspaces.count, tabs: tabs, stats: stats,
                      scrolls: scrolls)
    }

    #if os(macOS)
    /// B077: the context menu. Built (hit test, core, NSMenu) for each target kind in turn;
    /// on screen (the menu's window open and its tracking running); and two choices through to
    /// their effect drawn: Compare (`tab.open_edition`) and Search in all editions (the palette).
    private func measureContextMenu() async -> [Stat] {
        var stats: [Stat] = []
        let ws = model.activeWorkspace
        // A column showing text with words: the first one with a laid-out surface.
        guard let column = ws.columns.first(where: { model.surface(for: $0.shownTabId)?.layout?.display.length ?? 0 > 40 }),
              let tab = column.tabs.first(where: { $0.id == column.shownTabId }), let surface = model.surface(for: tab.id),
              let layout = surface.layout, let text = surface.textView else { return stats }
        model.focusColumn(column.number)
        Self.flush()
        let string = layout.display.string as NSString
        // Word starts in the first verse, for right-clicks on different words.
        var points: [NSPoint] = []
        string.enumerateSubstrings(in: NSRange(location: 0, length: min(string.length, 400)), options: .byWords) { _, range, _, _ in
            if let box = surface.rect(for: range) { points.append(NSPoint(x: box.midX, y: box.midY)) }
        }
        // Only the points a right-click finds a word at (not a verse number).
        points = points.filter { point in
            defer { surface.clearSelection() }
            if case .word = surface.contextHit(at: point) { return true }
            return false
        }
        guard !points.isEmpty else { return stats }
        let other = ws.columns.first { $0.number != column.number }?.number ?? column.number
        let kinds: [(Int) -> NSMenu?] = [
            { i in self.model.nsMenu(for: surface.contextHit(at: points[i % points.count]), tab: tab, column: column) },
            { i in
                surface.select(display: NSRange(location: 0, length: min(string.length, 30)))
                return self.model.nsMenu(for: surface.contextHit(at: points[1 % points.count]), tab: tab, column: column)
            },
            { _ in self.model.nsMenu(for: .tab(tabId: ws.columns[Int(other) - 1].tabs.last?.id ?? tab.id)) },
            { _ in self.model.nsMenu(for: .column(column: other)) },
            { _ in self.model.nsMenu(for: .background(column: column.number)) },
            { _ in self.model.nsMenu(for: .noteText(text: "grace", tabId: nil)) },
            { _ in self.model.nsMenu(for: .noteLink(reference: "Rom.8.28", tabId: nil)) },
        ]
        // Built: what AppKit waits for between the right-click and showing the menu (no
        // redraw: the word's selection is drawn while the menu is up).
        func built(_ kind: (Int) -> NSMenu?) -> [Double] {
            (0..<max(samplesPerInteraction, 210)).map { i in
                surface.clearSelection()
                let t0 = CACurrentMediaTime()
                _ = kind(i)
                return (CACurrentMediaTime() - t0) * 1000
            }
        }
        for _ in 0..<5 { kinds.forEach { _ = $0(0) } }
        stats.append(Self.stat("right-click → menu built, word (hit test selects it, core, NSMenu)", budget: 16, built(kinds[0])))
        stats.append(Self.stat("right-click → menu built, selection", budget: 16, built(kinds[1])))
        stats.append(Self.stat("right-click → menu built, tab · column · background · note text · note link", budget: 16,
                               built { i in kinds[2 + i % 5](i) }))
        surface.clearSelection()
        Self.flush()

        // On screen: popUp shows the menu window and starts tracking; a timer in the tracking
        // run loop mode stops the clock (the window is up by then) and closes the menu.
        func popUp(_ make: (Int) -> NSMenu?) async -> [Double] {
            var open: [Double] = []
            for i in 0..<max(samplesPerInteraction, 200) {
                surface.clearSelection()
                Self.flush()
                let t0 = CACurrentMediaTime()
                guard let menu = make(i) else { continue }
                let timer = Timer(timeInterval: 0, repeats: false) { _ in
                    MainActor.assumeIsolated {
                        open.append((CACurrentMediaTime() - t0) * 1000)
                        menu.cancelTracking()
                    }
                }
                RunLoop.main.add(timer, forMode: .eventTracking)
                menu.popUp(positioning: nil, at: points[i % points.count], in: text)
                try? await Task.sleep(nanoseconds: 20_000_000)
            }
            return open
        }
        stats.append(Self.stat("right-click → menu window open and tracking (word menu, end to end)", budget: 16,
                               await popUp { i in self.model.nsMenu(for: surface.contextHit(at: points[i % points.count]), tab: tab, column: column) }))
        // AppKit's own cost of showing a menu, for a short one and one the word menu's size.
        for rows in [3, 18] {
            stats.append(Self.stat("AppKit floor: popUp of a plain \(rows)-row NSMenu → tracking", budget: 16, await popUp { _ in
                let plain = NSMenu()
                (1...rows).forEach { plain.addItem(withTitle: "Row \($0)", action: nil, keyEquivalent: "") }
                return plain
            }))
        }
        surface.clearSelection()

        // Choices: the menu's focus, the row, the effect drawn.
        let wordMenu = { () -> ShellNSMenu? in
            surface.clearSelection()
            return self.model.nsMenu(for: surface.contextHit(at: points[0]), tab: tab, column: column) as? ShellNSMenu
        }
        if let menu = wordMenu(), let row = menu.items.first(where: { $0.identifier?.rawValue == "tab.open_edition" && $0.action != nil }),
           let choice = row.representedObject as? MenuChoice, let core = menu.shellMenu {
            stats.append(await measure("choose → effect: \(row.title) (tab.open_edition, drawn)", budget: 16,
                                       prepare: { i in if i > 0 { self.model.run("edit.undo") } }) { _ in
                self.model.choose(core, commandId: choice.commandId, args: choice.args)
            })
            model.run("edit.undo")
        }
        if let menu = wordMenu(), let row = menu.items.first(where: { ($0.representedObject as? MenuChoice)?.args.scope == "all" }),
           let choice = row.representedObject as? MenuChoice, let core = menu.shellMenu {
            stats.append(await measure("choose → effect: \(row.title) (palette Text section drawn)", budget: 50,
                                       prepare: { _ in self.model.closePalette() }) { _ in
                self.model.choose(core, commandId: choice.commandId, args: choice.args)
            })
            model.closePalette()
        }
        surface.clearSelection()
        Self.flush()
        return stats
    }
    #endif

    #if os(macOS)
    /// B078: continuous reading. Column 2 shows BSB in link group 1 with columns 1 (SBLGNT)
    /// and 3 (BSB) in the measurement session, so every move also moves two followers.
    private func measureReading() async -> ([Stat], [ScrollRun]) {
        var stats: [Stat] = []
        model.run("column.focus", args: ShellCommandArgs(column: 2))
        Self.flush()
        guard let stream = model.stream(for: model.activeColumn.shownTabId) else { return ([], []) }
        // A typed passage whose chapter is not in the window: locate, the chapter and its
        // neighbours from the core, the window laid out, the verse at the top, the followers
        // the same, drawn.
        let places = ["John.3.16", "Gen.1.1", "Ps.119.105", "Rev.22.21", "Isa.53.5", "Rom.8.28", "Mal.4.5", "Matt.5.3", "Job.38.4", "Acts.2.1",
                      "Ps.23", "Jude.1.3", "Luke.24.27", "Heb.11.1", "Exod.20.1"]
        stats.append(await measure("open a chapter: typed jump to a passage outside the window (nav.navigate, window loaded, two followers, drawn)", budget: 100) { i in
            self.model.run("nav.navigate", args: ShellCommandArgs(reference: places[i % places.count]))
        })
        // Into the next verse: record_scroll, the followers scrolled, drawn.
        model.run("nav.navigate", args: ShellCommandArgs(reference: "Matt.5.1"))
        Self.flush()
        model.followSamples = []
        stats.append(await measure("scroll into the next verse (record_scroll, two followers scrolled, drawn)", budget: 16) { _ in
            let before = stream.topSegment
            var steps = 0
            repeat { stream.scrollAsReader(by: 6); steps += 1 } while stream.topSegment == before && steps < 30
        })
        stats.append(Self.stat("linked follow, host side (core record_scroll and both followers scrolled, not drawn)", budget: 16, model.followSamples))
        stats.append(await measure("tab switch within a column after reading (tab.next)", budget: 16) { _ in
            self.model.run("tab.next")
        })
        model.run("column.focus", args: ShellCommandArgs(column: 2))
        while model.activeTab?.contentId != "bsb" { model.run("tab.next") }
        Self.flush()
        stats.append(await measure("labels after a scroll (refresh: tab labels and identity rows re-rendered, drawn)", budget: 16,
                                   prepare: { i in
            _ = try? self.model.handle.apply(commandId: "tab.record_scroll",
                                             args: ShellCommandArgs(workspaceId: self.model.session.activeWorkspaceId, tabId: self.model.activeColumn.shownTabId,
                                                                    segmentId: i % 2 == 0 ? "Matt.5.3" : "Matt.6.1", scalar: 0), hasSelection: false)
        }) { _ in self.model.refresh() })
        var runs: [ScrollRun] = []
        ReadingScheduler.shared.durations = []
        ReadingStream.timings = []
        ReadingScheduler.shared.kinds = []
        for pace in [40.0, 120.0] {
            guard let driver = model.stream(for: model.activeColumn.shownTabId) else { break }
            model.run("nav.navigate", args: ShellCommandArgs(reference: "Matt.1.1"))
            Self.flush()
            runs.append(await scrollRun(driver, pace: CGFloat(pace)))
        }
        let parts = ReadingStream.timings
        stats.append(Self.stat("window edit, part: layout of the window from its blocks (ReaderLayout(pieces:))", budget: 16, parts.map { $0.0 * 1000 }))
        stats.append(Self.stat("window edit, part: text storage edit and paint", budget: 16, parts.map { $0.1 * 1000 }))
        stats.append(Self.stat("window edit, part: TextKit 2 layout to the view and the viewport", budget: 16, parts.map { $0.2 * 1000 }))
        for kind in ["add above", "add below", "release above", "release below", "replace"] {
            let all = parts.filter { $0.3 == kind }.map { ($0.0 + $0.1 + $0.2) * 1000 }
            if !all.isEmpty { stats.append(Self.stat("window edit, whole: \(kind)", budget: 16, all)) }
        }
        let scheduler = ReadingScheduler.shared
        let window = zip(scheduler.durations, scheduler.kinds).filter { $0.1 == "window" && $0.0 > 0.05 }.map(\.0)
        stats.append(Self.stat("deferred, one per frame: a block loaded or released (one column)", budget: 16, window))
        return (stats, runs)
    }

    /// Scrolls one step per display-link frame until 32 chapters have passed the top.
    private func scrollRun(_ stream: ReadingStream, pace: CGFloat) async -> ScrollRun {
        var frames: [Double] = [], steps: [Double] = [], editSteps: [Double] = []
        var chapters = Set<String>()
        var stalls = 0, maxBlocks = 0, maxVerses = 0, maxUnits = 0
        let start = Self.residentMB()
        var slow: [String] = []
        var stepNotes: [String] = []
        var lastStep = 0.0
        var deferredMark = ReadingScheduler.shared.durations.count
        await withCheckedContinuation { (done: CheckedContinuation<Void, Never>) in
            var last: CFTimeInterval?
            var count = 0
            let ticker = FrameTicker { timestamp in
                if let last {
                    let frame = (timestamp - last) * 1000
                    frames.append(frame)
                    let scheduler = ReadingScheduler.shared
                    if frame > 34, slow.count < 40 {
                        let work = zip(scheduler.durations.dropFirst(deferredMark), scheduler.kinds.dropFirst(deferredMark))
                            .map { String(format: "%@ %.1f", $0.1, $0.0) }.joined(separator: ", ")
                        slow.append(String(format: "frame %.1f · step %.1f · deferred [%@]", frame, lastStep, work))
                    }
                    deferredMark = scheduler.durations.count
                }
                last = timestamp
                count += 1
                guard chapters.count < 32, count < 8000 else { return true }
                let edits = stream.edits.count
                let others = self.model.activeWorkspace.columns.compactMap { self.model.stream(for: $0.shownTabId) }
                let otherEdits = others.map(\.edits.count)
                let reports = self.model.recordedScrolls
                let before = stream.container?.scrollView.contentView.bounds.minY ?? 0
                let t0 = CACurrentMediaTime()
                stream.scrollAsReader(by: pace)
                let scrolled = (CACurrentMediaTime() - t0) * 1000
                Self.flush()
                let dt = (CACurrentMediaTime() - t0) * 1000
                lastStep = dt
                if dt > 15 {
                    let changed = zip(others, otherEdits).flatMap { s, n in s.edits.dropFirst(n).map { "\(s.tabID):\($0)" } }
                    stepNotes.append(String(format: "step %.1f (scroll %.1f) at %@ reports %d edits %@", dt, scrolled, stream.topSegment ?? "-",
                                            self.model.recordedScrolls - reports, changed.joined(separator: " ")))
                }
                let after = stream.container?.scrollView.contentView.bounds.minY ?? 0
                if stream.edits.count != edits { editSteps.append(dt) } else {
                    steps.append(dt)
                    if after - before < pace - 1 { stalls += 1 }
                }
                if let key = stream.topBlock?.chapterKey { chapters.insert(key) }
                maxBlocks = max(maxBlocks, stream.blocks.count)
                maxVerses = max(maxVerses, stream.renderedVerses)
                maxUnits = max(maxUnits, stream.surface.storage?.length ?? 0)
                return false
            } finished: {
                done.resume()
            }
            ticker.start()
        }
        func pct(_ values: [Double], _ p: Double) -> Double {
            guard !values.isEmpty else { return 0 }
            let sorted = values.sorted()
            return sorted[min(sorted.count - 1, Int((Double(sorted.count - 1) * p).rounded()))]
        }
        func r(_ v: Double) -> Double { (v * 100).rounded() / 100 }
        frames.removeFirst(min(3, frames.count))
        return ScrollRun(pointsPerFrame: Double(pace), chapters: chapters.count, frames: frames.count,
                         frameP50Ms: r(pct(frames, 0.5)), frameP95Ms: r(pct(frames, 0.95)), frameMaxMs: r(frames.max() ?? 0),
                         framesOver17Ms: frames.filter { $0 > 17.5 }.count, framesOver34Ms: frames.filter { $0 > 34 }.count,
                         stepP50Ms: r(pct(steps, 0.5)), stepP95Ms: r(pct(steps, 0.95)),
                         editSteps: editSteps.count, editStepP50Ms: r(pct(editSteps, 0.5)), editStepP95Ms: r(pct(editSteps, 0.95)),
                         editStepMaxMs: r(editSteps.max() ?? 0), stalls: stalls, maxBlocks: maxBlocks, maxVerses: maxVerses,
                         maxStorageUnits: maxUnits, residentMBStart: r(start), residentMBEnd: r(Self.residentMB()), slowFrames: slow + stepNotes.prefix(30))
    }

    /// The process's physical footprint, in MB.
    static func residentMB() -> Double {
        var info = task_vm_info_data_t()
        var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<natural_t>.size)
        let result = withUnsafeMutablePointer(to: &info) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count) }
        }
        return result == KERN_SUCCESS ? Double(info.phys_footprint) / 1_048_576 : 0
    }
    #endif

    static func stat(_ name: String, budget: Double, _ values: [Double]) -> Stat {
        func pct(_ p: Double) -> Double {
            guard !values.isEmpty else { return 0 }
            let sorted = values.sorted()
            return sorted[min(sorted.count - 1, Int((Double(sorted.count - 1) * p).rounded()))]
        }
        func r(_ v: Double) -> Double { (v * 100).rounded() / 100 }
        return Stat(interaction: name, budgetMs: budget, samples: values.count, p50Ms: r(pct(0.5)), p95Ms: r(pct(0.95)),
                    maxMs: r(values.max() ?? 0), nextFrameP50Ms: r(pct(0.5)), nextFrameP95Ms: r(pct(0.95)))
    }

    /// `prepare` runs and is drawn before the timer starts (the state an interaction starts from).
    func measure(_ name: String, budget: Double, count: Int? = nil, prepare: ((Int) -> Void)? = nil, action: @escaping (Int) -> Void) async -> Stat {
        let n = count ?? samplesPerInteraction
        var work: [Double] = []
        var toFrame: [Double] = []
        // Warm up: the first few frames build caches the steady state keeps.
        for i in 0..<5 { prepare?(i); Self.flush(); action(i); Self.flush() }
        await withCheckedContinuation { (done: CheckedContinuation<Void, Never>) in
            var index = 0
            var started: CFTimeInterval?
            let ticker = FrameTicker { _ in
                let now = CACurrentMediaTime()
                if let s = started { toFrame.append((now - s) * 1000) }
                guard index < n else { return true }
                if let prepare {
                    prepare(index)
                    Self.flush()
                }
                let t0 = CACurrentMediaTime()
                action(index)
                Self.flush()
                work.append((CACurrentMediaTime() - t0) * 1000)
                started = t0
                index += 1
                return false
            } finished: {
                done.resume()
            }
            ticker.start()
        }
        func pct(_ values: [Double], _ p: Double) -> Double {
            guard !values.isEmpty else { return 0 }
            let sorted = values.sorted()
            return sorted[min(sorted.count - 1, Int((Double(sorted.count - 1) * p).rounded()))]
        }
        func r(_ v: Double) -> Double { (v * 100).rounded() / 100 }
        return Stat(interaction: name, budgetMs: budget, samples: work.count, p50Ms: r(pct(work, 0.5)), p95Ms: r(pct(work, 0.95)),
                    maxMs: r(work.max() ?? 0), nextFrameP50Ms: r(pct(toFrame, 0.5)), nextFrameP95Ms: r(pct(toFrame, 0.95)))
    }

    /// Lays out and draws the window now.
    static func flush() {
        #if os(macOS)
        guard let window = NSApp.windows.first(where: { $0.isVisible }) else { return }
        window.contentView?.layoutSubtreeIfNeeded()
        window.displayIfNeeded()
        #endif
    }

    static var windowSize: CGSize {
        #if os(macOS)
        return NSApp.windows.first(where: { $0.isVisible })?.contentView?.bounds.size ?? .zero
        #else
        return .zero
        #endif
    }

    static func write(_ report: Report, to path: String) {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.keyEncodingStrategy = .convertToSnakeCase
        if let data = try? encoder.encode(report) { try? data.write(to: URL(fileURLWithPath: path)) }
    }
}

#if os(macOS)
extension ShellMeasure {
    /// Bodies of the palette's result list so far, and the text rows the last one drew
    /// (`PaletteResults`): a sample stops only once the list it times has really rendered.
    static var paletteRenders = 0
    static var paletteTextRows = 0
    static func notePaletteRender(textRows: Int) {
        paletteRenders += 1
        paletteTextRows = textRows
    }

    /// B071: `-OPENLOGOS_PACK_MEASURE`, the specs/22 §7 budgets over the real editions.
    func runPack(queries: [String]) async -> Report {
        var stats: [Stat] = []
        let model = model
        var keystrokes: [String] = []
        for query in queries {
            let scalars = Array(query)
            guard scalars.count >= 2 else { continue }
            for i in 2...scalars.count { keystrokes.append(String(scalars[..<i])) }
        }
        model.focusPalette()
        Self.flush()

        /// One keystroke to its text rows on screen: `main` is what the main thread did in the
        /// keystroke itself (the other sections, computed and drawn), `total` adds the answer
        /// from the search handle and the list drawn with it.
        func keystroke(_ text: String) async -> (main: Double, total: Double, rows: Int, rendered: Bool) {
            let t0 = CACurrentMediaTime()
            model.paletteText = text
            model.paletteTextChanged()
            Self.flush()
            let main = (CACurrentMediaTime() - t0) * 1000
            // Read when the rows go into the state, before SwiftUI can draw them.
            var renders = Self.paletteRenders
            if model.textSearch != nil {
                await withCheckedContinuation { (done: CheckedContinuation<Void, Never>) in
                    var resumed = false
                    model.onTextDelivered = {
                        guard !resumed else { return }
                        resumed = true
                        renders = Self.paletteRenders
                        done.resume()
                    }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
                        guard !resumed else { return }
                        resumed = true
                        done.resume()
                    }
                }
                model.onTextDelivered = nil
            }
            Self.flush()
            var turns = 0
            while Self.paletteRenders == renders, turns < 50 {
                await Task.yield()
                Self.flush()
                turns += 1
            }
            return (main, (CACurrentMediaTime() - t0) * 1000, Self.paletteTextRows, Self.paletteRenders != renders)
        }
        for text in keystrokes.prefix(30) { _ = await keystroke(text) }
        var mains: [Double] = [], totals: [Double] = [], withRows = 0, rendered = 0
        for text in keystrokes {
            let sample = await keystroke(text)
            mains.append(sample.main)
            totals.append(sample.total)
            if sample.rows > 0 { withRows += 1 }
            if sample.rendered { rendered += 1 }
            try? await Task.sleep(nanoseconds: 16_000_000)
        }
        stats.append(Self.stat("keystroke to painted text results, three editions (\(withRows) with text rows, \(rendered) of \(totals.count) saw the list render)", budget: 50, totals))
        stats.append(Self.stat("the keystroke on the main thread: passage, commands, tabs computed and drawn", budget: 16, mains))

        // Typing at a fast pace while the display link counts frames: what a hitch would show.
        let pace: UInt64 = 90_000_000
        var frames: [Double] = []
        var last: CFTimeInterval?
        var typing = true
        let ticker = FrameTicker { timestamp in
            if let last { frames.append((timestamp - last) * 1000) }
            last = timestamp
            return !typing
        } finished: {}
        ticker.start()
        for text in keystrokes.prefix(240) {
            model.paletteText = text
            model.paletteTextChanged()
            try? await Task.sleep(nanoseconds: pace)
        }
        typing = false
        try? await Task.sleep(nanoseconds: 50_000_000)
        stats.append(Self.stat("frame interval while typing 240 keystrokes at 90 ms (\(frames.filter { $0 > 17.5 }.count) over one frame, \(frames.filter { $0 > 34 }.count) over two)", budget: 16.7, frames))
        model.closePalette()
        Self.flush()

        // Chapters, column 2 (BSB in group 1 with columns 1 and 3 in the measurement session).
        model.run("column.focus", args: ShellCommandArgs(column: 2))
        Self.flush()
        // Long ones among them, each already laid out once.
        let warm = ["Ps.119.1", "Num.7.1", "1Chr.6.1", "Luke.1.1", "Matt.26.1", "Isa.66.1", "Ps.117.1", "John.3.16", "Gen.1.1", "Rev.22.1", "Rom.8.1", "Job.38.1"]
        stats.append(await measure("open a chapter, warm: typed jump, already laid out, long chapters included (followers, drawn)", budget: 100, count: 240) { i in
            model.run("nav.navigate", args: ShellCommandArgs(reference: warm[i % warm.count]))
        })
        // Every sample a chapter this run has not shown: from disk, hash checked, laid out.
        var fresh: [String] = []
        var key: String? = "Gen.2"
        while let current = key, fresh.count < 240 {
            fresh.append(current)
            for _ in 0..<4 { key = key.flatMap { try? model.reading.chapterNeighbors(resourceId: "bsb", chapterKey: $0).next } }
        }
        stats.append(await measure("open a chapter never shown yet: read, hash checked, laid out (followers, drawn)", budget: 100, count: fresh.count) { i in
            model.run("nav.navigate", args: ShellCommandArgs(reference: fresh[i % fresh.count]))
        })
        // Workspaces: real text in every tab.
        stats.append(await measure("workspace switch (workspace.next, 10 × 3 × 4, real text)", budget: 100) { _ in
            model.run("workspace.next")
        })
        let size = Self.windowSize
        let tabs = model.session.workspaces.reduce(0) { $0 + $1.columns.reduce(0) { $0 + $1.tabs.count } }
        return Report(host: "mac", window: "\(Int(size.width))x\(Int(size.height))", workspaces: model.session.workspaces.count, tabs: tabs, stats: stats)
    }
}
#endif

/// A display link that calls back once per frame until the callback returns true.
@MainActor
final class FrameTicker: NSObject {
    private var link: CADisplayLink?
    private let tick: (CFTimeInterval) -> Bool
    private let finished: () -> Void
    private var retainSelf: FrameTicker?

    init(tick: @escaping (CFTimeInterval) -> Bool, finished: @escaping () -> Void) {
        self.tick = tick
        self.finished = finished
    }

    func start() {
        retainSelf = self
        #if os(macOS)
        guard let view = NSApp.windows.first(where: { $0.isVisible })?.contentView else { finished(); return }
        link = view.displayLink(target: self, selector: #selector(step(_:)))
        #else
        link = CADisplayLink(target: self, selector: #selector(step(_:)))
        #endif
        link?.add(to: .main, forMode: .common)
    }

    @objc private func step(_ link: CADisplayLink) {
        if tick(link.timestamp) {
            link.invalidate()
            self.link = nil
            finished()
            retainSelf = nil
        }
    }
}
