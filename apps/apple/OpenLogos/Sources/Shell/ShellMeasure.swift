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
        model.run("column.focus", args: ShellCommandArgs(column: 1))
        stats.append(await measure("open a chapter in a tab, warm (nav.navigate, group follows)", budget: 100) { i in
            self.model.run("nav.navigate", args: ShellCommandArgs(reference: i % 2 == 0 ? "John.3.17" : "John.3.16"))
        })
        let size = Self.windowSize
        let tabs = model.session.workspaces.reduce(0) { $0 + $1.columns.reduce(0) { $0 + $1.tabs.count } }
        return Report(host: "mac", window: "\(Int(size.width))x\(Int(size.height))", workspaces: model.session.workspaces.count, tabs: tabs, stats: stats)
    }

    private func measure(_ name: String, budget: Double, count: Int? = nil, action: @escaping (Int) -> Void) async -> Stat {
        let n = count ?? samplesPerInteraction
        var work: [Double] = []
        var toFrame: [Double] = []
        // Warm up: the first few frames build caches the steady state keeps.
        for i in 0..<5 { action(i); Self.flush() }
        await withCheckedContinuation { (done: CheckedContinuation<Void, Never>) in
            var index = 0
            var started: CFTimeInterval?
            let ticker = FrameTicker { _ in
                let now = CACurrentMediaTime()
                if let s = started { toFrame.append((now - s) * 1000) }
                guard index < n else { return true }
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
