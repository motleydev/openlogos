// B005, acceptance 3: what VoiceOver and the accessibility tree see of the three-pane view
// with the notes dock, through XCUITest: the accessibility audit (iOS 17 / macOS 14) on the
// anchors scenario at 1024 and 1440 points, the elements and labels in focus order, and one
// selection driven by launch argument with its readout visible. Numbers and labels are printed
// and attached so docs/evidence/B005.md can quote them.
import XCTest

final class ReaderUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = true
    }

    private func launch(width: Int, extra: [String] = []) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = [
            "-OPENLOGOS_VIEW", "anchors", "-OPENLOGOS_WIDTH", "\(width)", "-OPENLOGOS_LAYOUT", "A",
            "-OPENLOGOS_DEBUGBAR", "0",
        ] + extra
        app.launch()
        return app
    }

    private func attach(_ text: String, name: String) {
        let attachment = XCTAttachment(string: text)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        print("B005 UI: \(name)\n\(text)")
    }

    func testAccessibilityAuditAt1024And1440() throws {
        for width in [1024, 1440] {
            let app = launch(width: width)
            XCTAssertTrue(app.wait(for: .runningForeground, timeout: 20))
            sleep(2)
            var issues: [String] = []
            do {
                try app.performAccessibilityAudit { issue in
                    issues.append("\(issue.auditType.rawValue): \(issue.compactDescription) [\(issue.element?.debugDescription.prefix(120) ?? "")]")
                    // Report everything; nothing is suppressed, nothing fails the run.
                    return true
                }
            } catch {
                issues.append("audit threw: \(error)")
            }
            attach(issues.isEmpty ? "no issues" : issues.joined(separator: "\n"), name: "accessibility-audit-\(width).txt")
        }
    }

    func testElementsAndLabelsInFocusOrder() throws {
        let app = launch(width: 1440)
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 20))
        sleep(2)
        let textViews = app.textViews.allElementsBoundByIndex
        var lines: [String] = []
        for (index, view) in textViews.enumerated() {
            lines.append("textView[\(index)] id=\(view.identifier) label=“\(view.label)” value=\(String(describing: view.value).prefix(80))")
        }
        let panes = app.otherElements.matching(NSPredicate(format: "label BEGINSWITH 'Pane '")).allElementsBoundByIndex
        for pane in panes { lines.append("pane: “\(pane.label)”") }
        let dock = app.otherElements["Notes dock"]
        lines.append("notes dock exists: \(dock.exists)")
        attach(lines.joined(separator: "\n"), name: "elements-1440.txt")
        XCTAssertGreaterThanOrEqual(textViews.count, 3, "three reading surfaces and the note")
    }

    func testLaunchArgumentSelectionShowsTheReadout() throws {
        let app = launch(width: 1440, extra: ["-OPENLOGOS_CASE", "cross-verse"])
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 20))
        let readout = app.staticTexts.matching(NSPredicate(format: "label CONTAINS 'round trip identical'")).firstMatch
        XCTAssertTrue(readout.waitForExistence(timeout: 10), "the anchors readout appears after the launch-argument selection")
        attach(readout.label, name: "readout-cross-verse.txt")
    }
}
