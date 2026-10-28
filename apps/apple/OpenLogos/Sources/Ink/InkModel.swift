// B039: the ink sheet's state on Apple platforms. The record is the UniFFI `InkDocument` of
// crates/ink (schemas/ink-document.schema.json); the outline comes from the core's `inkOutline`
// (the perfect-freehand 1.2.3 port); undo/redo and the stroke eraser work on the record with
// tombstones. Brushes and the touch policy mirror crates/ink/src/brush.rs and touch.rs (same
// names and cases); the Rust tests are the authority, the Swift tests check the mirror.
import Foundation
import SwiftUI

// MARK: - Brushes (mirror of crates/ink/src/brush.rs)

enum InkBrush: String, CaseIterable {
    case pen = "pf-1.2.3/pen-1"
    case highlighter = "pf-1.2.3/highlighter-1"

    static let engineVersion = "pf-1.2.3"

    var tool: InkTool { self == .pen ? .pen : .highlighter }
    var usesPressure: Bool { self == .pen }
    var opacity: Double { self == .pen ? 1 : 0.35 }

    /// The outline options for a stroke of `width`; `hasPressure` says whether every point
    /// carries a reported pressure (then a pen uses it instead of simulating one).
    func options(width: Double, hasPressure: Bool, last: Bool) -> InkOutlineOptions {
        switch self {
        case .pen:
            return InkOutlineOptions(size: width, thinning: 0.5, smoothing: 0.5, streamline: 0.5,
                                     simulatePressure: !hasPressure, last: last, capStart: true, capEnd: true,
                                     taperStart: .none, taperEnd: .none)
        case .highlighter:
            return InkOutlineOptions(size: width, thinning: 0, smoothing: 0.5, streamline: 0.6,
                                     simulatePressure: false, last: last, capStart: false, capEnd: false,
                                     taperStart: .none, taperEnd: .none)
        }
    }

    static func named(_ version: String) -> InkBrush? { InkBrush(rawValue: version) }
}

enum InkToolChoice: String, CaseIterable, Identifiable {
    case pen, highlighter, eraser
    var id: String { rawValue }
    var title: String {
        switch self {
        case .pen: return "Pen"
        case .highlighter: return "Highlighter"
        case .eraser: return "Stroke eraser"
        }
    }
}

// MARK: - Touch policy (mirror of crates/ink/src/touch.rs)

enum InkContact: Equatable { case stylus, finger, pointer }
enum InkInputMode: String, CaseIterable, Identifiable {
    case stylus, finger
    var id: String { rawValue }
}
enum InkPhase { case began, moved, ended, cancelled }

struct InkTouchEvent {
    let id: Int
    let contact: InkContact
    let phase: InkPhase
    /// 0 is the ink canvas; other panes are reading panes.
    let pane: Int
    /// The contact's major radius in points, where the platform reports one.
    let radius: Double?
}

enum InkDecision: Equatable {
    case draw
    case scroll(Int)
    case ignore(InkIgnoreReason)
    case cancelStrokeThenDraw
}

enum InkIgnoreReason: String { case palmDuringStroke, largeContact, secondFinger, fingerInStylusMode, unknown }

/// `TouchArbiter` in crates/ink: decisions are made at `began` and remembered for the touch.
struct InkTouchPolicy {
    static let palmRadiusPoints = 20.0
    /// The threshold in use; `-OPENLOGOS_INK_PALM_RADIUS <pt>` overrides it (the simulator's
    /// XCUITest touches report a radius above the default, see docs/evidence/B039.md).
    var palmRadius = InkTouchPolicy.palmRadiusPoints
    var mode: InkInputMode
    private var active: (id: Int, contact: InkContact)?
    private var live: [Int: InkDecision] = [:]

    init(mode: InkInputMode) {
        self.mode = mode
        let override = UserDefaults.standard.double(forKey: "OPENLOGOS_INK_PALM_RADIUS")
        if override > 0 { palmRadius = override }
    }

    var strokeInProgress: Bool { active != nil }

    mutating func decide(_ event: InkTouchEvent) -> InkDecision {
        switch event.phase {
        case .began:
            let decision = decideBegan(event)
            switch decision {
            case .draw: active = (event.id, event.contact)
            case .cancelStrokeThenDraw:
                if let previous = active { live[previous.id] = nil }
                active = (event.id, event.contact)
            case .scroll, .ignore: break
            }
            live[event.id] = decision == .cancelStrokeThenDraw ? .draw : decision
            return decision
        case .moved:
            return live[event.id] ?? .ignore(.unknown)
        case .ended, .cancelled:
            let decision = live[event.id] ?? .ignore(.unknown)
            live[event.id] = nil
            if active?.id == event.id { active = nil }
            return decision
        }
    }

    private func decideBegan(_ event: InkTouchEvent) -> InkDecision {
        let inCanvas = event.pane == 0
        switch event.contact {
        case .stylus:
            if let active, inCanvas {
                return active.contact == .finger ? .cancelStrokeThenDraw : .ignore(.secondFinger)
            }
            return inCanvas ? .draw : .scroll(event.pane)
        case .pointer:
            if inCanvas { return active == nil ? .draw : .ignore(.secondFinger) }
            return .scroll(event.pane)
        case .finger:
            if let radius = event.radius, radius >= palmRadius { return .ignore(.largeContact) }
            if active != nil { return .ignore(inCanvas && mode == .finger ? .secondFinger : .palmDuringStroke) }
            if inCanvas { return mode == .finger ? .draw : .ignore(.fingerInStylusMode) }
            return .scroll(event.pane)
        }
    }
}

// MARK: - Samples and measurement

/// One input sample as the platform delivered it, in canvas points.
struct InkSample {
    var x: Double
    var y: Double
    /// Normalised pressure, nil when the device reports none (finger, mouse).
    var pressure: Double?
    /// Platform event time (CACurrentMediaTime clock) in seconds.
    var time: TimeInterval
    /// Tilt in radians where the device reports it (Apple Pencil); schema 1.1 fields, null
    /// otherwise. Altitude `0...π/2`; azimuth folded into `0..<2π`.
    var altitude: Double?
    var azimuth: Double?
}

/// Input-to-visible timing for one delivered event (several coalesced samples share it).
struct InkLatencySample {
    var input: TimeInterval          // the touch's timestamp
    var handler: TimeInterval        // touchesMoved entered
    var outlineDone: TimeInterval    // inkOutline returned
    var committed: TimeInterval      // the layer path was set
    var frameTarget: TimeInterval?   // the first display-link targetTimestamp after the commit
    var outlinePoints: Int
    var inputPoints: Int
}

struct InkLatencyReport: Codable {
    struct Stat: Codable { var n: Int; var p50: Double?; var p95: Double?; var max: Double? }
    var label: String
    var samples: Int
    var inputToHandlerMs: Stat
    var outlineMs: Stat
    var handlerToCommitMs: Stat
    var inputToCommitMs: Stat
    var inputToFrameTargetMs: Stat
    var outlinePoints: Stat
    var inputPoints: Stat
    var note: String

    static func stat(_ values: [Double]) -> Stat {
        let v = values.filter { $0.isFinite }.sorted()
        guard !v.isEmpty else { return Stat(n: 0, p50: nil, p95: nil, max: nil) }
        let p = { (q: Double) in v[min(v.count - 1, Int(Double(v.count) * q))] }
        let r = { (x: Double) in (x * 100).rounded() / 100 }
        return Stat(n: v.count, p50: r(p(0.5)), p95: r(p(0.95)), max: r(v.last!))
    }

    init(label: String, samples: [InkLatencySample]) {
        self.label = label
        self.samples = samples.count
        let ms = { (f: (InkLatencySample) -> Double?) in samples.compactMap(f).map { $0 * 1000 } }
        inputToHandlerMs = Self.stat(ms { $0.handler - $0.input })
        outlineMs = Self.stat(ms { $0.outlineDone - $0.handler })
        handlerToCommitMs = Self.stat(ms { $0.committed - $0.handler })
        inputToCommitMs = Self.stat(ms { $0.committed - $0.input })
        inputToFrameTargetMs = Self.stat(ms { s in s.frameTarget.map { $0 - s.input } })
        outlinePoints = Self.stat(samples.map { Double($0.outlinePoints) })
        inputPoints = Self.stat(samples.map { Double($0.inputPoints) })
        note = "inputToFrameTarget is the CADisplayLink targetTimestamp of the first frame after the layer path was set, minus the touch timestamp: when that frame is due on the display. The simulator renders on the Mac's GPU through a software-presented screen; a device number needs a device."
    }

    var summary: String {
        func f(_ s: Stat) -> String { s.n == 0 ? "–" : "p50 \(s.p50!) · p95 \(s.p95!) · max \(s.max!) ms (n=\(s.n))" }
        return "\(label): \(samples) events\ninput → handler: \(f(inputToHandlerMs))\ninkOutline: \(f(outlineMs)) for \(Int(outlinePoints.p50 ?? 0))–\(Int(outlinePoints.max ?? 0)) outline points\ninput → commit: \(f(inputToCommitMs))\ninput → frame target (earliest visible): \(f(inputToFrameTargetMs))"
    }
}

// MARK: - The sheet

@MainActor
final class InkSheet: ObservableObject {
    @Published private(set) var document: InkDocument
    @Published var tool: InkToolChoice = .pen
    @Published var mode: InkInputMode = .stylus { didSet { policy.mode = mode } }
    @Published var color: Color = Color(red: 0.11, green: 0.31, blue: 0.85)
    @Published var width: Double = 4
    @Published var readout: String = "Draw with the Pencil, a finger (Finger mode) or the pointer."
    @Published var latency: InkLatencyReport?
    @Published private(set) var canUndo = false
    @Published private(set) var canRedo = false
    /// The strokes to show (document order minus tombstones); the canvas syncs its layers to it.
    @Published private(set) var revision = 0

    var policy = InkTouchPolicy(mode: .stylus)
    private var undoStack: [HistoryOp] = []
    private var redoStack: [HistoryOp] = []
    private var counter = 0

    enum HistoryOp { case add(String), erase(String), redoAdd(InkStroke) }

    static let highlighterWidthFactor = 3.0

    init(id: String = "ipad-ink-1") {
        document = InkDocument(schemaVersion: "1.1", fixtureOnly: true, id: id, snapshotId: "fixture:snapshot:1",
                               coordinateSpaceId: "fixture:coordinates:1", strokes: [], deletedStrokeIds: [],
                               viewportTransform: InkViewportTransform(scale: 1, translateX: 0, translateY: 0))
    }

    var liveStrokes: [InkStroke] {
        let deleted = Set(document.deletedStrokeIds)
        return document.strokes.filter { !deleted.contains($0.id) }
    }

    var brush: InkBrush { tool == .highlighter ? .highlighter : .pen }
    var strokeWidth: Double { tool == .highlighter ? width * Self.highlighterWidthFactor : width }

    func hex(_ color: Color) -> String {
        #if canImport(UIKit)
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        UIColor(color).getRed(&r, green: &g, blue: &b, alpha: &a)
        #else
        let c = NSColor(color).usingColorSpace(.sRGB) ?? .black
        let r = c.redComponent, g = c.greenComponent, b = c.blueComponent
        #endif
        return String(format: "#%02X%02X%02X", Int((r * 255).rounded()), Int((g * 255).rounded()), Int((b * 255).rounded()))
    }

    /// A new stroke record for the current tool, from samples relative to the stroke start.
    func makeStroke(samples: [InkSample]) -> InkStroke? {
        guard samples.count >= 2, let first = samples.first else { return nil }
        counter += 1
        var last = 0.0
        let points = samples.map { s -> InkPoint in
            let t = max(last, max(0, (s.time - first.time) * 1000))
            last = t
            return InkPoint(x: s.x, y: s.y, pressure: s.pressure, elapsedMs: t,
                            altitude: s.altitude.map { min(.pi / 2, max(0, $0)) },
                            azimuth: s.azimuth.map { var a = $0.truncatingRemainder(dividingBy: 2 * .pi); if a < 0 { a += 2 * .pi }; return a })
        }
        return InkStroke(id: "stroke-\(counter)", tool: brush.tool, brushVersion: brush.rawValue, width: strokeWidth,
                         color: hex(color), opacity: brush.opacity, points: points)
    }

    func commit(_ stroke: InkStroke) {
        document.strokes.append(stroke)
        undoStack.append(.add(stroke.id))
        redoStack.removeAll()
        bump()
        readout = "stroke \(stroke.id): \(stroke.tool == .pen ? "pen" : "highlighter") \(stroke.points.count) points, width \(stroke.width), pressure \(stroke.points.first?.pressure == nil ? "null" : "recorded"), \(stroke.brushVersion)\(stroke.points.contains { $0.altitude != nil } ? ", tilt recorded" : ", tilt null")"
    }

    /// Stroke eraser: the topmost live stroke whose centre line passes within its half width (+4 pt).
    func erase(at p: CGPoint) {
        for stroke in liveStrokes.reversed() {
            let r = stroke.width / 2 + 4
            for i in 1..<stroke.points.count where Self.distance(p, stroke.points[i - 1], stroke.points[i]) <= r {
                document.deletedStrokeIds.append(stroke.id)
                undoStack.append(.erase(stroke.id))
                redoStack.removeAll()
                bump()
                readout = "erased \(stroke.id) (tombstone kept in deleted_stroke_ids)"
                return
            }
        }
    }

    func undo() {
        guard let op = undoStack.popLast() else { return }
        switch op {
        case .add(let id):
            if let i = document.strokes.firstIndex(where: { $0.id == id }) { redoStack.append(.redoAdd(document.strokes.remove(at: i))) }
        case .erase(let id):
            if let i = document.deletedStrokeIds.lastIndex(of: id) { document.deletedStrokeIds.remove(at: i) }
            redoStack.append(.erase(id))
        case .redoAdd: break
        }
        bump()
    }

    func redo() {
        guard let op = redoStack.popLast() else { return }
        switch op {
        case .redoAdd(let stroke):
            document.strokes.append(stroke); undoStack.append(.add(stroke.id))
        case .erase(let id):
            document.deletedStrokeIds.append(id); undoStack.append(.erase(id))
        case .add: break
        }
        bump()
    }

    /// The document as schema JSON through the core (validates first).
    func exportJSON() -> Result<String, Error> {
        Result { try inkDocumentJson(document: document) }
    }

    /// Replaces the document (PencilKit import); history is cleared.
    func replace(with document: InkDocument) {
        self.document = document
        undoStack.removeAll(); redoStack.removeAll()
        counter = document.strokes.count
        bump()
    }

    private func bump() {
        revision += 1
        canUndo = !undoStack.isEmpty
        canRedo = !redoStack.isEmpty
    }

    static func distance(_ p: CGPoint, _ a: InkPoint, _ b: InkPoint) -> Double {
        let dx = b.x - a.x, dy = b.y - a.y
        let l2 = dx * dx + dy * dy
        let t = l2 == 0 ? 0 : max(0, min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / l2))
        return hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy))
    }

    /// Fixture strokes for pictures: a highlighter pair, a pressure word, a loop, a null-pressure line.
    func addDemoStrokes(in size: CGSize) {
        let w = size.width, h = size.height
        func mk(_ id: String, _ brush: InkBrush, _ color: String, _ width: Double, _ pts: [(Double, Double)], _ pressure: ((Double) -> Double)?) -> InkStroke {
            let n = Double(pts.count - 1)
            return InkStroke(id: id, tool: brush.tool, brushVersion: brush.rawValue, width: width, color: color, opacity: brush.opacity,
                             points: pts.enumerated().map { i, p in InkPoint(x: p.0 * w, y: p.1 * h, pressure: pressure?(Double(i) / n), elapsedMs: Double(i) * 8, altitude: nil, azimuth: nil) })
        }
        func curve(_ n: Int, _ f: (Double) -> (Double, Double)) -> [(Double, Double)] { (0..<n).map { f(Double($0) / Double(n - 1)) } }
        document.strokes.append(mk("demo-hl", .highlighter, "#FACC15", 14, curve(24) { t in (0.08 + t * 0.55, 0.12 + sin(t * 3) * 0.004) }, nil))
        document.strokes.append(mk("demo-word", .pen, "#1D4ED8", 3.5, curve(90) { t in (0.1 + t * 0.5, 0.26 + sin(t * .pi * 6) * 0.03 * (1 - t * 0.3) + t * 0.02) }, { t in 0.3 + 0.6 * sin(t * .pi) }))
        document.strokes.append(mk("demo-loop", .pen, "#B91C1C", 3, curve(80) { t in let a = t * .pi * 2.5; return (0.72 + cos(a) * 0.12, 0.38 + sin(a) * 0.1 + t * 0.03) }, { t in 0.25 + 0.5 * t }))
        document.strokes.append(mk("demo-finger", .pen, "#15803D", 4, curve(40) { t in (0.12 + t * 0.4, 0.56 + abs(sin(t * .pi * 2)) * 0.08) }, nil))
        document.strokes.append(mk("demo-hl2", .highlighter, "#38BDF8", 14, curve(20) { t in (0.3 + t * 0.4, 0.13 + t * 0.001) }, nil))
        counter = 100
        for s in document.strokes { undoStack.append(.add(s.id)) }
        bump()
    }
}

#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
