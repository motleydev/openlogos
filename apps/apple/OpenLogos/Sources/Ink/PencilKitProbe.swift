// B039: the PencilKit probe (iPad). A PKCanvasView with the system pen, and an export of every
// PKStroke into the portable InkDocument record through the fields PencilKit exposes:
// PKStrokePath control points (location, timeOffset, size, opacity, force, azimuth, altitude),
// PKInk (inkType, color), the stroke transform, the eraser mask and randomSeed. What it does
// not expose is listed by `PencilKitExport.notes` and in docs/evidence/B039.md. The opaque
// `PKDrawing.dataRepresentation()` is the native fidelity blob of specs/13: kept beside the
// portable record for re-editing with PencilKit's own brushes, never the only copy.
#if canImport(UIKit) && canImport(PencilKit)
import PencilKit
import SwiftUI

struct PencilKitProbe: UIViewRepresentable {
    @ObservedObject var sheet: InkSheet
    @Binding var export: String?

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeUIView(context: Context) -> PKCanvasView {
        let view = PKCanvasView()
        view.drawingPolicy = .anyInput // the simulator has no Pencil; a finger (or XCUITest) must draw
        view.tool = PKInkingTool(.pen, color: .systemBlue, width: 4)
        view.backgroundColor = .clear
        view.isOpaque = false
        view.delegate = context.coordinator
        view.accessibilityIdentifier = "pencilkit-canvas"
        view.accessibilityLabel = "PencilKit canvas"
        if UserDefaults.standard.bool(forKey: "OPENLOGOS_INK_PKDEMO") {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                view.drawing = PencilKitExport.demoDrawing(in: view.bounds.size == .zero ? CGSize(width: 500, height: 600) : view.bounds.size)
            }
        }
        return view
    }

    func updateUIView(_ view: PKCanvasView, context: Context) {
        switch sheet.tool {
        case .pen: view.tool = PKInkingTool(.pen, color: UIColor(sheet.color), width: sheet.width)
        case .highlighter: view.tool = PKInkingTool(.marker, color: UIColor(sheet.color), width: sheet.strokeWidth)
        case .eraser: view.tool = PKEraserTool(.vector)
        }
    }

    final class Coordinator: NSObject, PKCanvasViewDelegate {
        let parent: PencilKitProbe
        init(_ parent: PencilKitProbe) { self.parent = parent }

        func canvasViewDrawingDidChange(_ canvasView: PKCanvasView) {
            let (document, notes) = PencilKitExport.document(from: canvasView.drawing, id: "pencilkit-probe-1")
            parent.sheet.replace(with: document)
            var lines = notes
            switch Result(catching: { try inkDocumentJson(document: document) }) {
            case .success(let json): lines.append("inkDocumentJson: valid, \(json.count) bytes")
                if UserDefaults.standard.bool(forKey: "OPENLOGOS_LOG") { PencilKitExport.log(json: json, notes: notes) }
            case .failure(let error): lines.append("inkDocumentJson rejected: \(error)")
            }
            parent.export = lines.joined(separator: "\n")
            parent.sheet.readout = "PencilKit: \(canvasView.drawing.strokes.count) strokes exported to the portable record"
        }
    }
}

enum PencilKitExport {
    /// `UITouch.maximumPossibleForce` on Apple Pencil is 4.1667 (13 / 3). PencilKit documents
    /// `PKStrokePoint.force` as "the force of the touch" without a unit; dividing by this is an
    /// assumption to be checked on a device, and the export says so.
    static let assumedMaximumForce = 4.1667

    static func document(from drawing: PKDrawing, id: String) -> (InkDocument, [String]) {
        var strokes: [InkStroke] = []
        var notes: [String] = []
        var masks = 0, markers = 0, withForce = 0, withTilt = 0
        for (index, stroke) in drawing.strokes.enumerated() {
            let control = Array(stroke.path)
            guard control.count >= 2 else { continue }
            let forces = control.map(\.force)
            let hasForce = forces.contains { $0 > 0 }
            if hasForce { withForce += 1 }
            if control.contains(where: { $0.altitude != 0 || $0.azimuth != 0 }) { withTilt += 1 }
            if stroke.mask != nil { masks += 1 }
            if stroke.ink.inkType == .marker { markers += 1 }
            let tool: InkTool = stroke.ink.inkType == .marker ? .highlighter : .pen
            let width = control.map { Double($0.size.width) }.reduce(0, +) / Double(control.count)
            var red: CGFloat = 0, green: CGFloat = 0, blue: CGFloat = 0, alpha: CGFloat = 1
            stroke.ink.color.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)).getRed(&red, green: &green, blue: &blue, alpha: &alpha)
            let meanOpacity = control.map { Double($0.opacity) }.reduce(0, +) / Double(control.count)
            let points = control.map { p -> InkPoint in
                let location = p.location.applying(stroke.transform)
                return InkPoint(x: location.x, y: location.y,
                                pressure: hasForce ? min(1, max(0, Double(p.force) / assumedMaximumForce)) : nil,
                                elapsedMs: max(0, p.timeOffset * 1000))
            }
            strokes.append(InkStroke(id: "pk-\(index + 1)", tool: tool,
                                     brushVersion: "pencilkit/\(stroke.ink.inkType.rawValue)",
                                     width: max(0.1, width), color: String(format: "#%02X%02X%02X", Int(red * 255), Int(green * 255), Int(blue * 255)),
                                     opacity: min(1, max(0, Double(alpha) * (tool == .highlighter ? 1 : meanOpacity))),
                                     points: points))
        }
        var document = InkDocument(schemaVersion: "1.0", fixtureOnly: true, id: id, snapshotId: "fixture:snapshot:1",
                                   coordinateSpaceId: "fixture:coordinates:1", strokes: strokes, deletedStrokeIds: [],
                                   viewportTransform: InkViewportTransform(scale: 1, translateX: 0, translateY: 0))
        // Monotonic time: PencilKit control points are resampled and can repeat an offset.
        for i in document.strokes.indices {
            var last = 0.0
            for j in document.strokes[i].points.indices {
                let t = max(last, document.strokes[i].points[j].elapsedMs)
                document.strokes[i].points[j].elapsedMs = t
                last = t
            }
        }
        let blob = drawing.dataRepresentation()
        notes.append("PencilKit export: \(strokes.count) strokes, \(strokes.reduce(0) { $0 + $1.points.count }) control points (PKStrokePath, not raw touches)")
        notes.append("force > 0 on \(withForce) strokes (finger and simulator report 0 → pressure null); tilt on \(withTilt) strokes (no schema 1.0 field; dropped)")
        notes.append("marker strokes → highlighter: \(markers); eraser masks (UIBezierPath, bitmap eraser) on \(masks) strokes: not representable, stroke kept whole")
        notes.append("brush_version is PencilKit's inkType (\(Set(drawing.strokes.map { $0.ink.inkType.rawValue }).sorted().joined(separator: ", "))): its geometry is opaque, so these strokes re-render with our fallback brush and are labelled")
        notes.append("native blob PKDrawing.dataRepresentation(): \(blob.count) bytes, opaque format, kept only as the optional fidelity cache")
        notes.append("force scale assumed /\(assumedMaximumForce) (UITouch.maximumPossibleForce on Pencil); unverified without a Pencil")
        return (document, notes)
    }

    /// A programmatic drawing for the simulator: a pen stroke with force and tilt, a marker band.
    static func demoDrawing(in size: CGSize) -> PKDrawing {
        let w = size.width, h = size.height
        func points(_ n: Int, _ f: (Double) -> (Double, Double), force: ((Double) -> Double)?, width: Double) -> [PKStrokePoint] {
            (0..<n).map { i in
                let t = Double(i) / Double(n - 1)
                let (x, y) = f(t)
                return PKStrokePoint(location: CGPoint(x: x * w, y: y * h), timeOffset: t * 0.9, size: CGSize(width: width, height: width),
                                     opacity: 1, force: force?(t) ?? 0, azimuth: force == nil ? 0 : .pi / 4, altitude: force == nil ? 0 : .pi / 3)
            }
        }
        let pen = PKStroke(ink: PKInk(.pen, color: .systemBlue),
                           path: PKStrokePath(controlPoints: points(60, { t in (0.1 + t * 0.6, 0.3 + sin(t * .pi * 4) * 0.05) }, force: { t in 0.5 + 2.5 * sin(t * .pi) }, width: 4),
                                              creationDate: Date()))
        let marker = PKStroke(ink: PKInk(.marker, color: .systemYellow),
                              path: PKStrokePath(controlPoints: points(20, { t in (0.1 + t * 0.6, 0.15) }, force: nil, width: 14), creationDate: Date()))
        return PKDrawing(strokes: [marker, pen])
    }

    static func log(json: String, notes: [String]) {
        let directory = UserDefaults.standard.string(forKey: "OPENLOGOS_LOG_DIR").map { URL(fileURLWithPath: $0) }
            ?? FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        try? json.write(to: directory.appendingPathComponent("b039-pencilkit-export.json"), atomically: true, encoding: .utf8)
        try? notes.joined(separator: "\n").write(to: directory.appendingPathComponent("b039-pencilkit-notes.txt"), atomically: true, encoding: .utf8)
    }
}
#endif
