// B039: the ink canvas. UIKit touches (coalesced samples, Pencil force, altitude and azimuth,
// majorRadius for the palm rule) on iPad; NSEvent mouse and tablet events on the Mac. Every
// stroke is one CAShapeLayer whose path is the core's perfect-freehand outline (`inkOutline`)
// filled once with the non-zero rule at the stroke's opacity: the compositing rule of
// crates/ink/src/brush.rs, so a highlighter never darkens over itself. The live stroke is one
// more such layer whose path is replaced on every delivered event.
//
// Latency: each delivered event records the touch timestamp, the handler entry, the outline
// return, the layer commit, and the target timestamp of the first display-link frame after the
// commit; `os_signpost` intervals `ink-sample` (org.openlogos.app / ink) carry the same for
// Instruments. The same plain-value entry points (`began`, `moved`, `ended`) serve UIKit, AppKit
// and the in-process synthesized run, so one code path is measured.
import OSLog
import QuartzCore
import SwiftUI

#if canImport(UIKit)
import UIKit
typealias InkPlatformView = UIView
#else
import AppKit
typealias InkPlatformView = NSView
#endif

private let inkLog = OSLog(subsystem: "org.openlogos.app", category: "ink")

/// The platform-independent canvas: layers, the live stroke, the policy and the measurement.
@MainActor
final class InkCanvasEngine {
    let container = CALayer()
    private let liveLayer = CAShapeLayer()
    private var strokeLayers: [String: CAShapeLayer] = [:]
    private var pathCache: [String: CGPath] = [:]
    private var live: (id: Int, samples: [InkSample], brush: InkBrush, width: Double, color: CGColor, opacity: Double)?
    private var pending: [InkLatencySample] = []
    private(set) var latency: [InkLatencySample] = []
    var recording = false
    var label = "touches"
    weak var sheet: InkSheet?
    var size: CGSize = .zero
    var scale: CGFloat = 2

    init() {
        liveLayer.fillRule = .nonZero
        liveLayer.strokeColor = nil
        container.addSublayer(liveLayer)
    }

    // MARK: Layers

    func syncLayers() {
        guard let sheet else { return }
        let wanted = sheet.liveStrokes
        let ids = Set(wanted.map(\.id))
        for (id, layer) in strokeLayers where !ids.contains(id) {
            layer.removeFromSuperlayer()
            strokeLayers[id] = nil
        }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        for stroke in wanted {
            let layer = strokeLayers[stroke.id] ?? {
                let l = CAShapeLayer()
                l.fillRule = .nonZero
                l.strokeColor = nil
                l.contentsScale = scale
                strokeLayers[stroke.id] = l
                return l
            }()
            if layer.path == nil || pathCache[stroke.id] == nil {
                let path = Self.path(for: stroke)
                pathCache[stroke.id] = path
                layer.path = path
            }
            layer.fillColor = Self.cgColor(stroke.color)
            layer.opacity = Float(stroke.opacity)
            // z-order follows the document: re-insert below the live layer in order.
            layer.removeFromSuperlayer()
            container.insertSublayer(layer, below: liveLayer)
        }
        CATransaction.commit()
    }

    static func path(for stroke: InkStroke) -> CGPath {
        let brush = InkBrush.named(stroke.brushVersion) ?? (stroke.tool == .pen ? .pen : .highlighter)
        let hasPressure = stroke.points.allSatisfy { $0.pressure != nil }
        let usePressure = brush.usesPressure && hasPressure
        var flat: [Double] = []
        flat.reserveCapacity(stroke.points.count * 3)
        for p in stroke.points { flat.append(p.x); flat.append(p.y); flat.append(usePressure ? (p.pressure ?? -1) : -1) }
        return cgPath(inkOutline(points: flat, options: brush.options(width: stroke.width, hasPressure: usePressure, last: true)))
    }

    static func cgPath(_ flat: [Double]) -> CGPath {
        let path = CGMutablePath()
        guard flat.count >= 4 else { return path }
        path.move(to: CGPoint(x: flat[0], y: flat[1]))
        var i = 2
        while i + 1 < flat.count { path.addLine(to: CGPoint(x: flat[i], y: flat[i + 1])); i += 2 }
        path.closeSubpath()
        return path
    }

    static func cgColor(_ hex: String) -> CGColor {
        var s = hex.hasPrefix("#") ? String(hex.dropFirst()) : hex
        if s.count == 3 { s = s.map { "\($0)\($0)" }.joined() }
        let v = UInt32(s, radix: 16) ?? 0
        return CGColor(srgbRed: CGFloat((v >> 16) & 0xFF) / 255, green: CGFloat((v >> 8) & 0xFF) / 255, blue: CGFloat(v & 0xFF) / 255, alpha: 1)
    }

    // MARK: Input (plain values; UIKit, AppKit and the synthesized run all come here)

    /// Returns the policy's decision so the platform view can act on `scroll` or ignore.
    @discardableResult
    func began(id: Int, contact: InkContact, radius: Double?, sample: InkSample) -> InkDecision {
        guard let sheet else { return .ignore(.unknown) }
        let decision = sheet.policy.decide(InkTouchEvent(id: id, contact: contact, phase: .began, pane: 0, radius: radius))
        switch decision {
        case .cancelStrokeThenDraw:
            cancelLive()
            fallthrough
        case .draw:
            if sheet.tool == .eraser {
                sheet.erase(at: CGPoint(x: sample.x, y: sample.y))
                syncLayers()
            } else {
                let brush = sheet.brush
                live = (id, [sample], brush, sheet.strokeWidth, Self.cgColor(sheet.hex(sheet.color)), brush.opacity)
                liveLayer.fillColor = live?.color
                liveLayer.opacity = Float(brush.opacity)
                liveLayer.contentsScale = scale
            }
        case .ignore(let reason):
            if reason != .fingerInStylusMode {
                let radiusNote = radius.map { String(format: " (majorRadius %.1f pt, threshold %.0f)", $0, sheet.policy.palmRadius) } ?? ""
                sheet.readout = "\(contact) ignored: \(reason.rawValue)\(radiusNote)"
            }
        case .scroll:
            break
        }
        return decision
    }

    func moved(id: Int, samples: [InkSample], handlerEntered: TimeInterval = CACurrentMediaTime()) {
        guard let sheet else { return }
        let decision = sheet.policy.decide(InkTouchEvent(id: id, contact: .finger, phase: .moved, pane: 0, radius: nil))
        guard decision == .draw, live?.id == id, var current = live, let first = samples.first else { return }
        if sheet.tool == .eraser {
            for s in samples { sheet.erase(at: CGPoint(x: s.x, y: s.y)) }
            syncLayers()
            return
        }
        let signpost = OSSignpostID(log: inkLog)
        os_signpost(.begin, log: inkLog, name: "ink-sample", signpostID: signpost, "input %f", first.time)
        current.samples.append(contentsOf: samples)
        live = current
        let outline = liveOutline(current)
        let outlineDone = CACurrentMediaTime()
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        liveLayer.path = Self.cgPath(outline)
        CATransaction.commit()
        let committed = CACurrentMediaTime()
        os_signpost(.end, log: inkLog, name: "ink-sample", signpostID: signpost, "outline %d committed", outline.count / 2)
        if recording {
            pending.append(InkLatencySample(input: first.time, handler: handlerEntered, outlineDone: outlineDone, committed: committed,
                                            frameTarget: nil, outlinePoints: outline.count / 2, inputPoints: current.samples.count))
        }
    }

    func ended(id: Int, sample: InkSample?, cancelled: Bool) {
        guard let sheet else { return }
        let decision = sheet.policy.decide(InkTouchEvent(id: id, contact: .finger, phase: cancelled ? .cancelled : .ended, pane: 0, radius: nil))
        guard decision == .draw, live?.id == id, var current = live else { return }
        if let sample { current.samples.append(sample) }
        live = nil
        liveLayer.path = nil
        guard !cancelled, sheet.tool != .eraser else { return }
        var samples = current.samples
        // A pen whose pressure never varied is no sensor: null, not a fabricated constant.
        if samples.count > 3, let p0 = samples[0].pressure, samples.allSatisfy({ $0.pressure == p0 }) {
            for i in samples.indices { samples[i].pressure = nil }
        }
        if let stroke = sheet.makeStroke(samples: samples) {
            sheet.commit(stroke)
            syncLayers()
        }
    }

    func cancelLive() {
        live = nil
        liveLayer.path = nil
    }

    private func liveOutline(_ current: (id: Int, samples: [InkSample], brush: InkBrush, width: Double, color: CGColor, opacity: Double)) -> [Double] {
        let hasPressure = current.brush.usesPressure && current.samples.allSatisfy { $0.pressure != nil }
        var flat: [Double] = []
        flat.reserveCapacity(current.samples.count * 3)
        for s in current.samples { flat.append(s.x); flat.append(s.y); flat.append(hasPressure ? (s.pressure ?? -1) : -1) }
        return inkOutline(points: flat, options: current.brush.options(width: current.width, hasPressure: hasPressure, last: false))
    }

    // MARK: Frames

    /// Called by the platform's display link each frame with that frame's target time.
    func frame(target: TimeInterval) {
        guard !pending.isEmpty else { return }
        for var p in pending { p.frameTarget = target; latency.append(p) }
        pending.removeAll()
    }

    func resetLatency() { latency.removeAll(); pending.removeAll() }

    /// The synthesized run: a looping pen stroke with pressure delivered through `began`,
    /// `moved`, `ended` at `interval` seconds per event, as the UIKit path would deliver them.
    func synthesize(points: Int = 120, interval: TimeInterval = 0.004, contact: InkContact = .stylus, label: String) async -> InkLatencyReport {
        resetLatency()
        recording = true
        self.label = label
        let previousMode = sheet?.mode
        if contact == .finger { sheet?.mode = .finger }
        let w = size.width, h = size.height
        let cx = w * 0.5, cy = h * 0.45, rx = min(w, h) * 0.3, ry = rx * 0.6
        func at(_ i: Int) -> InkSample {
            let a = Double(i) / Double(points) * .pi * 2.4
            return InkSample(x: cx + cos(a) * rx + Double(i) * 0.6, y: cy + sin(a) * ry,
                             pressure: contact == .stylus ? 0.3 + 0.5 * sin(Double(i) / Double(points) * .pi) : nil,
                             time: CACurrentMediaTime(), altitude: nil, azimuth: nil)
        }
        began(id: 900, contact: contact, radius: contact == .finger ? 7 : nil, sample: at(0))
        for i in 1..<points {
            try? await Task.sleep(for: .seconds(interval))
            moved(id: 900, samples: [at(i)])
        }
        ended(id: 900, sample: nil, cancelled: false)
        try? await Task.sleep(for: .milliseconds(80))
        recording = false
        if let previousMode { sheet?.mode = previousMode }
        return report(label: label)
    }

    /// Samples still waiting for a display-link frame (none runs in a unit test) are reported
    /// without a frame target rather than dropped.
    func report(label: String) -> InkLatencyReport { InkLatencyReport(label: label, samples: latency + pending) }
}

// MARK: - UIKit

#if canImport(UIKit)
final class InkCanvasUIView: UIView {
    let engine = InkCanvasEngine()
    private var displayLink: CADisplayLink?
    private var touchIds: [ObjectIdentifier: Int] = [:]
    private var nextId = 1
    var recordRealTouches = true

    override init(frame: CGRect) {
        super.init(frame: frame)
        isMultipleTouchEnabled = true
        backgroundColor = .clear
        layer.addSublayer(engine.container)
        isAccessibilityElement = true
        accessibilityLabel = "Ink canvas"
        accessibilityIdentifier = "ink-canvas"
        let link = CADisplayLink(target: self, selector: #selector(tick(_:)))
        link.add(to: .main, forMode: .common)
        displayLink = link
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    override func layoutSubviews() {
        super.layoutSubviews()
        engine.container.frame = bounds
        engine.size = bounds.size
        engine.scale = window?.screen.scale ?? 2
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window == nil { displayLink?.invalidate(); displayLink = nil }
    }

    @objc private func tick(_ link: CADisplayLink) { engine.frame(target: link.targetTimestamp) }

    private func id(for touch: UITouch) -> Int {
        let key = ObjectIdentifier(touch)
        if let id = touchIds[key] { return id }
        touchIds[key] = nextId
        nextId += 1
        return nextId - 1
    }

    private func contact(of touch: UITouch) -> InkContact {
        switch touch.type {
        case .pencil: return .stylus
        case .indirectPointer: return .pointer
        default: return .finger
        }
    }

    private func sample(_ touch: UITouch) -> InkSample {
        let p = touch.location(in: self)
        let pencil = touch.type == .pencil
        return InkSample(x: p.x, y: p.y,
                         pressure: pencil && touch.maximumPossibleForce > 0 ? min(1, max(0, touch.force / touch.maximumPossibleForce)) : nil,
                         time: touch.timestamp,
                         altitude: pencil ? touch.altitudeAngle : nil,
                         azimuth: pencil ? touch.azimuthAngle(in: self) : nil)
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        engine.recording = recordRealTouches
        for touch in touches {
            engine.began(id: id(for: touch), contact: contact(of: touch), radius: touch.type == .direct ? touch.majorRadius : nil, sample: sample(touch))
        }
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        let entered = CACurrentMediaTime()
        for touch in touches {
            let coalesced = event?.coalescedTouches(for: touch) ?? [touch]
            engine.moved(id: id(for: touch), samples: coalesced.map(sample), handlerEntered: entered)
        }
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            engine.ended(id: id(for: touch), sample: sample(touch), cancelled: false)
            touchIds[ObjectIdentifier(touch)] = nil
        }
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            engine.ended(id: id(for: touch), sample: nil, cancelled: true)
            touchIds[ObjectIdentifier(touch)] = nil
        }
    }
}

struct InkCanvas: UIViewRepresentable {
    @ObservedObject var sheet: InkSheet
    let onReady: (InkCanvasEngine) -> Void

    func makeUIView(context: Context) -> InkCanvasUIView {
        let view = InkCanvasUIView(frame: .zero)
        view.engine.sheet = sheet
        DispatchQueue.main.async { onReady(view.engine) }
        return view
    }

    func updateUIView(_ view: InkCanvasUIView, context: Context) {
        view.engine.sheet = sheet
        view.engine.syncLayers()
    }
}

#else

// MARK: - AppKit

final class InkCanvasNSView: NSView {
    let engine = InkCanvasEngine()
    private var displayLink: CADisplayLink?
    private var dragId = 0

    override init(frame: CGRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.addSublayer(engine.container)
        setAccessibilityLabel("Ink canvas")
        setAccessibilityIdentifier("ink-canvas")
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    override var isFlipped: Bool { true }

    override func layout() {
        super.layout()
        engine.container.frame = bounds
        engine.size = bounds.size
        engine.scale = window?.backingScaleFactor ?? 2
        if displayLink == nil {
            let link = displayLink(target: self, selector: #selector(tick(_:)))
            link.add(to: .main, forMode: .common)
            displayLink = link
        }
    }

    @objc private func tick(_ link: CADisplayLink) { engine.frame(target: link.targetTimestamp) }

    private func sample(_ event: NSEvent) -> InkSample {
        let p = convert(event.locationInWindow, from: nil)
        let tablet = event.subtype == .tabletPoint
        return InkSample(x: p.x, y: p.y, pressure: tablet ? Double(event.pressure) : nil, time: event.timestamp,
                         altitude: nil, azimuth: nil)
    }

    override func mouseDown(with event: NSEvent) {
        dragId += 1
        engine.recording = true
        engine.began(id: dragId, contact: .pointer, radius: nil, sample: sample(event))
    }

    override func mouseDragged(with event: NSEvent) {
        let entered = CACurrentMediaTime()
        var samples = [sample(event)]
        if let coalesced = event.coalescedEventsPointingDevice(), !coalesced.isEmpty { samples = coalesced.map(sample) }
        engine.moved(id: dragId, samples: samples, handlerEntered: entered)
    }

    override func mouseUp(with event: NSEvent) {
        engine.ended(id: dragId, sample: sample(event), cancelled: false)
    }
}

private extension NSEvent {
    /// AppKit has no coalesced-event API; one sample per event.
    func coalescedEventsPointingDevice() -> [NSEvent]? { nil }
}

struct InkCanvas: NSViewRepresentable {
    @ObservedObject var sheet: InkSheet
    let onReady: (InkCanvasEngine) -> Void

    func makeNSView(context: Context) -> InkCanvasNSView {
        let view = InkCanvasNSView(frame: .zero)
        view.engine.sheet = sheet
        DispatchQueue.main.async { onReady(view.engine) }
        return view
    }

    func updateNSView(_ view: InkCanvasNSView, context: Context) {
        view.engine.sheet = sheet
        view.engine.syncLayers()
    }
}
#endif
