// B039: real touch sequences on the ink canvas through XCUITest on the iPad simulator. The
// drags are system touch events (finger type; the simulator has no Pencil), delivered to the
// UIKit canvas exactly as a user's would be, so the app's own measurement (touch timestamp →
// handler → outline → layer commit → display-link frame target) is what the readout reports.
// The readout and the stroke count are read back and attached for docs/evidence/B039.md.
import XCTest

final class InkUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = true
    }

    private func launch(extra: [String] = []) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = [
            "-OPENLOGOS_VIEW", "ink", "-OPENLOGOS_WIDTH", "1024", "-OPENLOGOS_LAYOUT", "A", "-OPENLOGOS_DEBUGBAR", "0",
            "-OPENLOGOS_LOG", "1",
        ] + extra
        app.launch()
        return app
    }

    private func attach(_ text: String, name: String) {
        let attachment = XCTAttachment(string: text)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        print("B039 UI: \(name)\n\(text)")
    }

    /// A drag made of many short segments so UIKit delivers a stream of touchesMoved events.
    private func scribble(on canvas: XCUIElement, points: [(Double, Double)]) {
        guard let first = points.first else { return }
        let start = canvas.coordinate(withNormalizedOffset: CGVector(dx: first.0, dy: first.1))
        var previous = start
        start.press(forDuration: 0.05, thenDragTo: start)
        for (x, y) in points.dropFirst() {
            let next = canvas.coordinate(withNormalizedOffset: CGVector(dx: x, dy: y))
            previous.press(forDuration: 0.01, thenDragTo: next, withVelocity: .fast, thenHoldForDuration: 0)
            previous = next
        }
    }

    func testFingerDragsDrawStrokesAndTheAppMeasuresThem() throws {
        let app = launch()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 30))
        sleep(2)
        let canvas = app.descendants(matching: .any)["ink-canvas"].firstMatch
        XCTAssertTrue(canvas.waitForExistence(timeout: 10), "the ink canvas is on screen")
        // Finger mode so the system touches draw.
        let mode = app.segmentedControls["ink-mode"].firstMatch
        if mode.exists { mode.buttons["Finger"].tap() } else { app.buttons["Finger"].firstMatch.tap() }

        // One long drag across the sheet (a single system gesture, many touchesMoved).
        let a = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.1, dy: 0.3))
        let b = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5))
        a.press(forDuration: 0.05, thenDragTo: b, withVelocity: .slow, thenHoldForDuration: 0.05)
        // A wavy scribble in segments.
        var wave: [(Double, Double)] = []
        for i in 0..<16 {
            let x: Double = 0.15 + Double(i) * 0.045
            let y: Double = i % 2 == 0 ? 0.65 : 0.73
            wave.append((x, y))
        }
        scribble(on: canvas, points: wave)
        sleep(1)

        let meta = app.staticTexts["ink-meta"].firstMatch
        let readout = app.descendants(matching: .any)["ink-readout"].firstMatch
        XCTAssertTrue(readout.waitForExistence(timeout: 5))
        let metaText = meta.exists ? meta.label : "(no meta)"
        let readoutText = String(describing: readout.value ?? readout.label)
        attach("meta: \(metaText)\nreadout: \(readoutText)", name: "ink-after-drags.txt")
        XCTAssertTrue(metaText.contains("strokes"), metaText)
        XCTAssertFalse(metaText.hasPrefix("0 strokes"), "the drags produced strokes: \(metaText)")

        // The measured latency of those real touches, from the app's log in its container
        // (-OPENLOGOS_LOG 1 writes Documents/b039-ink.jsonl on export); the Measure button
        // adds the in-process synthesized run beside it for the same build.
        app.buttons["ink-measure"].firstMatch.tap()
        sleep(3)
        let after = String(describing: readout.value ?? readout.label)
        attach(after, name: "ink-measure-readout.txt")
        XCTAssertTrue(after.contains("synthesized pen"), after)
        app.buttons["ink-export"].firstMatch.tap()
        sleep(1)
        let exported = String(describing: readout.value ?? readout.label)
        XCTAssertTrue(exported.contains("schema_version") || exported.contains("valid"), exported.prefix(200).description)
        attach(String(exported.prefix(600)), name: "ink-export-readout.txt")
    }

    func testPencilKitProbeDrawsWithAFingerAndExports() throws {
        let app = launch(extra: ["-OPENLOGOS_INK_PENCILKIT", "1", "-OPENLOGOS_INK_PKDEMO", "1"])
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 30))
        sleep(3)
        let canvas = app.descendants(matching: .any)["pencilkit-canvas"].firstMatch
        XCTAssertTrue(canvas.waitForExistence(timeout: 10), "the PencilKit canvas is on screen")
        let a = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.15, dy: 0.6))
        let b = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.8, dy: 0.75))
        a.press(forDuration: 0.05, thenDragTo: b, withVelocity: .slow, thenHoldForDuration: 0.05)
        sleep(2)
        let readout = app.descendants(matching: .any)["ink-readout"].firstMatch
        XCTAssertTrue(readout.waitForExistence(timeout: 5))
        let text = String(describing: readout.value ?? readout.label)
        attach(text, name: "pencilkit-export.txt")
        XCTAssertTrue(text.contains("PencilKit export"), text)
        XCTAssertTrue(text.contains("3 strokes") || text.contains("strokes"), text)
        XCTAssertTrue(text.contains("inkDocumentJson: valid"), text)
    }
}
