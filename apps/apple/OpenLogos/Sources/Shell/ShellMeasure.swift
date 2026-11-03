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
    }

    private let model: ShellModel
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
            let range = layout.runMap.displayRange(fromCore: NSRange(location: 0, length: 12))
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
        #endif
        let size = Self.windowSize
        let tabs = model.session.workspaces.reduce(0) { $0 + $1.columns.reduce(0) { $0 + $1.tabs.count } }
        return Report(host: "mac", window: "\(Int(size.width))x\(Int(size.height))", workspaces: model.session.workspaces.count, tabs: tabs, stats: stats)
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

    private static func stat(_ name: String, budget: Double, _ values: [Double]) -> Stat {
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
    private func measure(_ name: String, budget: Double, count: Int? = nil, prepare: ((Int) -> Void)? = nil, action: @escaping (Int) -> Void) async -> Stat {
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

/// A display link that calls back once per frame until the callback returns true.
@MainActor
private final class FrameTicker: NSObject {
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
