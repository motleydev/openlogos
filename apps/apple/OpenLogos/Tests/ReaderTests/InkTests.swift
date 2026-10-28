// B039: the ink layer in the app on the iPad simulator (OpenLogosTests) and natively on the
// Mac (OpenLogosMacTests): the touch policy mirror runs the same cases as crates/ink/src/touch.rs
// (same names), the core's inkOutline is reached through the bindings, the sheet's history and
// eraser work on the record with tombstones, the engine's plain-value input path measures a
// synthesized stroke, and (iPad) the PencilKit export maps a programmatic PKDrawing into a
// record inkDocumentJson accepts.
import XCTest
@testable import OpenLogos

#if canImport(PencilKit) && canImport(UIKit)
import PencilKit
#endif

@MainActor
final class InkTests: XCTestCase {
    private func ev(_ id: Int, _ contact: InkContact, _ phase: InkPhase, _ pane: Int, radius: Double? = nil) -> InkTouchEvent {
        InkTouchEvent(id: id, contact: contact, phase: phase, pane: pane, radius: radius ?? (contact == .finger ? 7 : nil))
    }

    private func run(_ mode: InkInputMode, _ stream: [InkTouchEvent]) -> [InkDecision] {
        var policy = InkTouchPolicy(mode: mode)
        return stream.map { policy.decide($0) }
    }

    // MARK: Touch policy (mirror of crates/ink/src/touch.rs, same cases)

    func testStylusModeFingerInAnotherPaneDuringAStrokeDoesNotScrollIt() {
        let d = run(.stylus, [
            ev(1, .stylus, .began, 0), ev(1, .stylus, .moved, 0), ev(2, .finger, .began, 2), ev(2, .finger, .moved, 2),
            ev(1, .stylus, .moved, 0), ev(2, .finger, .moved, 2), ev(1, .stylus, .ended, 0), ev(2, .finger, .moved, 2),
            ev(2, .finger, .ended, 2), ev(3, .finger, .began, 2), ev(3, .finger, .moved, 2),
        ])
        XCTAssertEqual(d[0], .draw)
        XCTAssertEqual(d[2], .ignore(.palmDuringStroke))
        XCTAssertEqual(d[3], .ignore(.palmDuringStroke))
        XCTAssertEqual(d[5], .ignore(.palmDuringStroke))
        XCTAssertEqual(d[7], .ignore(.palmDuringStroke), "a palm stays a palm")
        XCTAssertEqual(d[9], .scroll(2))
        XCTAssertEqual(d[10], .scroll(2))
    }

    func testStylusModeFingerInTheCanvasIsIgnoredAndScrollsElsewhere() {
        let d = run(.stylus, [ev(1, .finger, .began, 0), ev(1, .finger, .ended, 0), ev(2, .finger, .began, 1)])
        XCTAssertEqual(d[0], .ignore(.fingerInStylusMode))
        XCTAssertEqual(d[2], .scroll(1))
    }

    func testFingerModeDrawsWithOneFingerAndIgnoresTheSecond() {
        let d = run(.finger, [
            ev(1, .finger, .began, 0), ev(2, .finger, .began, 0), ev(2, .finger, .moved, 0), ev(1, .finger, .moved, 0),
            ev(3, .finger, .began, 1), ev(1, .finger, .ended, 0), ev(2, .finger, .ended, 0), ev(4, .finger, .began, 1),
        ])
        XCTAssertEqual(d[0], .draw)
        XCTAssertEqual(d[1], .ignore(.secondFinger))
        XCTAssertEqual(d[3], .draw)
        XCTAssertEqual(d[4], .ignore(.palmDuringStroke))
        XCTAssertEqual(d[7], .scroll(1))
    }

    func testAStylusCancelsAFingerStrokeAndTakesOver() {
        var policy = InkTouchPolicy(mode: .finger)
        XCTAssertEqual(policy.decide(ev(1, .finger, .began, 0)), .draw)
        XCTAssertEqual(policy.decide(ev(2, .stylus, .began, 0)), .cancelStrokeThenDraw)
        XCTAssertEqual(policy.decide(ev(2, .stylus, .moved, 0)), .draw)
        XCTAssertEqual(policy.decide(ev(1, .finger, .moved, 0)), .ignore(.unknown))
        XCTAssertTrue(policy.strokeInProgress)
    }

    func testAWideContactIsAPalmInEitherModeEvenWithNoStroke() {
        for mode in InkInputMode.allCases {
            var policy = InkTouchPolicy(mode: mode)
            XCTAssertEqual(policy.decide(ev(9, .finger, .began, 0, radius: InkTouchPolicy.palmRadiusPoints)), .ignore(.largeContact))
            XCTAssertEqual(policy.decide(ev(9, .finger, .moved, 0, radius: 6)), .ignore(.largeContact))
            XCTAssertFalse(policy.strokeInProgress)
        }
    }

    func testAPointerDrawsInTheCanvasAndScrollsElsewhere() {
        XCTAssertEqual(run(.stylus, [ev(1, .pointer, .began, 0), ev(1, .pointer, .ended, 0), ev(2, .pointer, .began, 3)]), [.draw, .draw, .scroll(3)])
    }

    // MARK: The core through the bindings

    func testInkOutlineReturnsAClosedPolygonOfPairs() {
        let flat: [Double] = [0, 0, -1, 40, 10, -1, 80, 0, 0.7, 120, 20, 0.9]
        let outline = inkOutline(points: flat, options: InkBrush.pen.options(width: 8, hasPressure: false, last: true))
        XCTAssertGreaterThanOrEqual(outline.count, 8)
        XCTAssertEqual(outline.count % 2, 0)
        XCTAssertTrue(inkOutline(points: [], options: InkBrush.pen.options(width: 8, hasPressure: false, last: true)).isEmpty)
        let highlighter = inkOutline(points: flat, options: InkBrush.highlighter.options(width: 12, hasPressure: false, last: true))
        XCTAssertNotEqual(outline, highlighter)
    }

    func testBrushVersionsNameTheEngineRelease() {
        for brush in InkBrush.allCases {
            XCTAssertTrue(brush.rawValue.hasPrefix(InkBrush.engineVersion))
            XCTAssertEqual(InkBrush.named(brush.rawValue), brush)
        }
        XCTAssertNil(InkBrush.named("pencilkit/com.apple.ink.pen"))
    }

    // MARK: The sheet

    private func stroke(_ sheet: InkSheet, x0: Double, y: Double) -> InkStroke {
        let samples = (0..<12).map { i in InkSample(x: x0 + Double(i) * 8, y: y, pressure: nil, time: Double(i) * 0.008, altitude: nil, azimuth: nil) }
        return sheet.makeStroke(samples: samples)!
    }

    func testHistoryAndStrokeEraserKeepTombstonesAndRoundTripThroughTheCore() throws {
        let sheet = InkSheet(id: "test-ink")
        let a = stroke(sheet, x0: 10, y: 50)
        sheet.commit(a)
        sheet.tool = .highlighter
        let b = stroke(sheet, x0: 10, y: 150)
        sheet.commit(b)
        XCTAssertEqual(sheet.liveStrokes.map(\.id), ["stroke-1", "stroke-2"])
        XCTAssertEqual(b.tool, .highlighter)
        XCTAssertEqual(b.width, 4 * InkSheet.highlighterWidthFactor)
        XCTAssertEqual(b.opacity, 0.35)
        XCTAssertNil(a.points[0].pressure, "a finger reports no pressure: null, not 0.5")
        XCTAssertEqual(a.points.last?.elapsedMs ?? 0, 88, accuracy: 0.01)

        sheet.erase(at: CGPoint(x: 50, y: 52))
        XCTAssertEqual(sheet.document.deletedStrokeIds, ["stroke-1"])
        XCTAssertEqual(sheet.liveStrokes.map(\.id), ["stroke-2"])
        sheet.erase(at: CGPoint(x: 50, y: 400), "nothing there: no change")
        XCTAssertEqual(sheet.document.deletedStrokeIds, ["stroke-1"])

        sheet.undo()
        XCTAssertEqual(sheet.liveStrokes.count, 2)
        sheet.undo()
        XCTAssertEqual(sheet.liveStrokes.map(\.id), ["stroke-1"])
        sheet.redo()
        XCTAssertEqual(sheet.liveStrokes.map(\.id), ["stroke-1", "stroke-2"])
        sheet.redo()
        XCTAssertEqual(sheet.liveStrokes.map(\.id), ["stroke-2"])
        XCTAssertFalse(sheet.canRedo)

        let json = try sheet.exportJSON().get()
        XCTAssertTrue(json.contains("\"deleted_stroke_ids\": [\n    \"stroke-1\"\n  ]"), json)
        XCTAssertTrue(json.contains("\"pressure\": null"))
        XCTAssertTrue(json.contains("\"tool\": \"highlighter\""))
    }

    func testTheEngineMeasuresASynthesizedStrokeThroughTheSameInputPath() async {
        let sheet = InkSheet(id: "test-engine")
        let engine = InkCanvasEngine()
        engine.sheet = sheet
        engine.size = CGSize(width: 600, height: 500)
        let report = await engine.synthesize(points: 40, interval: 0.002, label: "unit test")
        XCTAssertEqual(sheet.liveStrokes.count, 1)
        XCTAssertEqual(sheet.liveStrokes[0].points.count, 40)
        XCTAssertNotNil(sheet.liveStrokes[0].points[1].pressure)
        XCTAssertEqual(report.samples, 39)
        XCTAssertGreaterThan(report.outlineMs.n, 0)
        // No display link runs in a unit test, so frame targets are absent; commits are timed.
        XCTAssertEqual(report.inputToCommitMs.n, 39)
        print("B039 unit: \(report.summary)")
    }

    // MARK: PencilKit (iPad)

    #if canImport(PencilKit) && canImport(UIKit)
    func testPencilKitExportMapsControlPointsIntoTheRecord() throws {
        let drawing = PencilKitExport.demoDrawing(in: CGSize(width: 500, height: 600))
        let (document, notes) = PencilKitExport.document(from: drawing, id: "pk-test")
        XCTAssertEqual(document.strokes.count, 2)
        let marker = document.strokes[0], pen = document.strokes[1]
        XCTAssertEqual(marker.tool, .highlighter)
        XCTAssertEqual(marker.brushVersion, "pencilkit/\(PKInkingTool.InkType.marker.rawValue)")
        XCTAssertNil(marker.points[0].pressure, "a marker built with force 0 exports null pressure")
        XCTAssertEqual(pen.tool, .pen)
        XCTAssertNotNil(pen.points[5].pressure)
        XCTAssertLessThanOrEqual(pen.points.map { $0.pressure ?? 0 }.max() ?? 0, 1)
        XCTAssertEqual(pen.width, 4, accuracy: 0.01)
        XCTAssertEqual(pen.points.count, 60, "control points come back one for one from PKStrokePath")
        XCTAssertEqual(pen.points.last?.elapsedMs ?? 0, 900, accuracy: 0.5)
        let json = try inkDocumentJson(document: document)
        XCTAssertTrue(json.contains("\"schema_version\": \"1.0\""))
        XCTAssertTrue(notes.contains { $0.contains("tilt on") && $0.contains("no schema 1.0 field") }, notes.joined(separator: "\n"))
        XCTAssertTrue(notes.contains { $0.contains("native blob") })
        let blob = drawing.dataRepresentation()
        XCTAssertGreaterThan(blob.count, 0)
        let back = try PKDrawing(data: blob)
        XCTAssertEqual(back.strokes.count, 2, "the opaque blob round-trips through PencilKit itself")
        print("B039 PencilKit:\n" + notes.joined(separator: "\n"))
    }
    #endif
}

private extension InkSheet {
    func erase(at p: CGPoint, _ note: String) { erase(at: p) }
}
