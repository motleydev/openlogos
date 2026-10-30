// B072: the Mac shell's cases, hosted in the app (OpenLogosMacTests). The model over the real
// UniFFI ShellHandle (seed session, commands, undo, host commands, the palette), the keymap
// read from the core registry, the tokens, the text source, and the rendered shell in a
// window: the reading surface stays on TextKit 2, the Highlight and Collect drags reach the
// model, ink follows the tool.
import AppKit
import SwiftUI
import XCTest
@testable import OpenLogos

@MainActor
final class ShellTests: XCTestCase {
    // MARK: Tokens

    func testTokensMatchTheBrief() {
        XCTAssertEqual(ShellTokens.readingSizes, [15, 17, 19, 21, 24, 28, 32])
        XCTAssertEqual(ShellTokens.readingSizes[ShellTokens.defaultReadingSizeIndex], 19)
        XCTAssertEqual(ShellTokens.lineSpacings, [1.35, 1.5, 1.7])
        XCTAssertEqual(ShellTokens.lineSpacings[ShellTokens.defaultLineSpacingIndex], 1.5)
        XCTAssertEqual(ShellTokens.light.page, 0xFBFAF7)
        XCTAssertEqual(ShellTokens.light.accent, 0x2F5BD3)
        XCTAssertEqual(ShellTokens.sepia.text, 0x2B2620)
        XCTAssertEqual(ShellTokens.sepia.accent, 0x8A4B16)
        XCTAssertEqual(ShellTokens.dark.page, 0x161616)
        XCTAssertEqual(ShellTokens.dark.rules, 0x2C2B29)
        XCTAssertEqual(ShellTokens.swatches.map(\.rgb), [0xF5D90A, 0x4CC38A, 0x5B9BFF, 0xF25C9A])
        XCTAssertEqual(ShellTokens.swatches.map(\.alpha), [0.35, 0.30, 0.30, 0.28])
        XCTAssertEqual([ShellTokens.Spacing.xs, ShellTokens.Spacing.s, ShellTokens.Spacing.m, ShellTokens.Spacing.l, ShellTokens.Spacing.xl], [4, 8, 12, 16, 24])
        XCTAssertEqual(ShellTokens.Radius.control, 6)
        XCTAssertEqual(ShellTokens.Radius.toolWell, 10)
        XCTAssertEqual([ShellTokens.paletteHeight, ShellTokens.tabStripHeight, ShellTokens.identityRowHeight, ShellTokens.toolWellWidth], [44, 28, 32, 40])
    }

    // MARK: Keymap

    func testEveryMacShortcutInTheRegistryParsesAndNoneIsBoundTwice() {
        var seen: [KeyChord: String] = [:]
        for command in shellRegistry() {
            for text in command.shortcuts.mac {
                guard let chord = KeyChord(registry: text) else { return XCTFail("\(command.id): \(text) does not parse") }
                if let other = seen[chord] { XCTFail("\(text) bound to \(other) and \(command.id)") }
                seen[chord] = command.id
            }
        }
        XCTAssertGreaterThan(seen.count, 30)
    }

    func testChordsResolveToRegisteredCommands() {
        let keymap = ShellKeymap(registry: shellRegistry(), platform: .mac)
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "K", command: true))?.commandID, "palette.focus")
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "/"))?.commandID, "palette.focus")
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "/"))?.bare, true)
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "3", option: true)), KeyBinding(commandID: "column.focus", column: 3, bare: false))
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "2", command: true)), KeyBinding(commandID: "column.focus", column: 2, bare: false), "the native alias")
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "2", option: true, shift: true))?.commandID, "tab.move_to_column")
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "T", option: true))?.commandID, "tab.new")
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "T", command: true))?.commandID, "tab.new")
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "Tab", control: true))?.commandID, "tab.next")
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "Tab", shift: true, control: true))?.commandID, "tab.previous")
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "]", option: true, shift: true))?.commandID, "workspace.next")
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "N", command: true, shift: true))?.commandID, "workspace.new")
        XCTAssertEqual(keymap.binding(for: KeyChord(key: "Z", command: true))?.commandID, "edit.undo")
        for (key, id) in [("V", "tool.select"), ("H", "tool.highlight"), ("P", "tool.pen"), ("M", "tool.highlighter_pen"), ("E", "tool.eraser"), ("C", "tool.collect"), ("X", "tool.swap_swatches")] {
            XCTAssertEqual(keymap.binding(for: KeyChord(key: key)), KeyBinding(commandID: id, column: nil, bare: true), key)
        }
        XCTAssertNil(keymap.binding(for: KeyChord(key: "Q", command: true)), "nothing the registry does not list")
        XCTAssertNil(keymap.binding(for: KeyChord(key: "0", option: true)), "columns are 1…9")
        XCTAssertEqual(KeyChord.keyName(code: 40), "K")
        XCTAssertEqual(KeyChord.keyName(code: 30), "]")
    }

    func testBareKeysOnlyOutsideTextFields() {
        let model = ShellModel()
        model.run("tool.pen")
        XCTAssertFalse(model.handle(chord: KeyChord(key: "V"), inTextField: true), "V types into a field")
        XCTAssertEqual(model.tool, "pen")
        XCTAssertTrue(model.handle(chord: KeyChord(key: "V"), inTextField: false))
        XCTAssertEqual(model.tool, "select")
        XCTAssertTrue(model.handle(chord: KeyChord(key: "2", option: true), inTextField: true), "⌥ chords work in the palette field too")
        XCTAssertEqual(model.activeWorkspace.activeColumnNumber, 2)
        XCTAssertFalse(model.handle(chord: KeyChord(key: "Z", command: true), inTextField: true), "⌘Z in a field edits the field")
    }

    // MARK: Session and commands

    func testTheSeedSessionIsTheBriefs() {
        let model = ShellModel()
        XCTAssertEqual(model.session.workspaces.map(\.title), ["Study", "Hebrew"])
        let study = model.session.workspaces[0]
        XCTAssertEqual(study.columns.map(\.number), [1, 2, 3])
        let labels = study.columns.map { $0.tabs.map(TabTitle.label) }
        XCTAssertEqual(labels, [["BSB Jn 3:16", "WLC Gen 1:1"], ["SBLGNT Jn 3:16"], ["Note"]], "the core's resource titles and short references")
        XCTAssertEqual(model.session.ink.swatch, "#F5D90A", "the core's default swatches are the tokens'")
        XCTAssertEqual(model.session.ink.alternateSwatch, "#4CC38A")
        XCTAssertEqual(study.columns.map { TabTitle.linkState($0.tabs[0]) }, ["Group 1", "Group 1", "Pinned"])
        XCTAssertEqual(model.session.workspaces[1].columns.count, 1)
        XCTAssertEqual(model.activeTab?.contentId, "bsb")
        XCTAssertEqual(model.tool, "select")
    }

    func testCommandsGoThroughTheCoreAndUndo() {
        let model = ShellModel()
        model.run("column.focus", args: ShellCommandArgs(column: 3))
        XCTAssertEqual(model.activeWorkspace.activeColumnNumber, 3)
        model.focusColumn(1)
        model.run("tab.next")
        XCTAssertEqual(model.activeTab?.contentId, "wlc")
        model.run("column.new")
        XCTAssertEqual(model.activeWorkspace.columns.count, 4)
        XCTAssertEqual(model.session.undoTitle, "New column")
        XCTAssertTrue(model.handle(chord: KeyChord(key: "Z", command: true), inTextField: false))
        XCTAssertEqual(model.activeWorkspace.columns.count, 3, "⌘Z takes the column back through the shell's undo stack")
        model.run("workspace.next")
        XCTAssertEqual(model.activeWorkspace.title, "Hebrew")
        XCTAssertFalse(model.run("column.close"), "the only column")
        XCTAssertNotNil(model.toast, "the disabled reason is shown")
    }

    func testReadingAndInkSettingsLiveInTheSession() {
        let model = ShellModel()
        XCTAssertEqual(model.session.reading.theme, "light")
        model.run("view.text_size_up")
        XCTAssertEqual(model.session.reading.textSizeStep, 1)
        XCTAssertEqual(model.readingSize, 21)
        model.run("view.text_size_down"); model.run("view.text_size_down")
        XCTAssertEqual(model.readingSize, 17)
        model.run("view.text_size_reset")
        XCTAssertEqual(model.readingSize, 19)
        for _ in 0..<4 { model.run("view.text_size_up") }
        XCTAssertEqual(model.readingSize, 32, "step 4 is the largest token size")
        XCTAssertNotNil(model.disabledReason("view.text_size_up"), "the core stops at the last size")
        model.run("view.text_size_reset")
        model.run("view.line_spacing", args: ShellCommandArgs(number: 1.7))
        XCTAssertEqual(model.lineSpacing, 1.7)
        model.run("view.theme", args: ShellCommandArgs(text: "sepia"))
        XCTAssertEqual(model.theme, ShellTokens.sepia)
        model.run("view.theme", args: ShellCommandArgs(text: "dark"))
        XCTAssertEqual(model.theme, ShellTokens.dark)
        XCTAssertEqual(model.swatch.id, "yellow")
        XCTAssertTrue(model.handle(chord: KeyChord(key: "X"), inTextField: false))
        XCTAssertEqual(model.session.ink.swatch, "#4CC38A", "X swaps in the core")
        XCTAssertEqual(model.swatch.id, "green")
        XCTAssertEqual(model.alternateSwatch.id, "yellow")
        model.run("tool.swatch", args: ShellCommandArgs(text: ShellTokens.hex(0x5B9BFF)))
        XCTAssertEqual(model.swatch.id, "blue")
        XCTAssertEqual(model.inkWidth, 4)
        model.cycleInkWidth()
        XCTAssertEqual(model.inkWidth, 6)
        XCTAssertEqual(model.toolWellClearance, 60, "12 + 40 + 8")
        model.run("view.tool_palette")
        XCTAssertTrue(model.toolWellCollapsed, "the well's visibility is the host's (handled: false)")
        XCTAssertEqual(model.toolWellClearance, 42, "collapsed, column 1 still clears the grip: 12 + 22 + 8")
        XCTAssertEqual(model.typography.key, "dark-2-1.7")
        XCTAssertEqual(model.typography.body.pointSize, 19)
        XCTAssertEqual(model.measureCap, 640, "from the core's layoutConstants()")
    }

    func testColumnWidthsAndScrollAnchorsLiveInTheSession() {
        let model = ShellModel()
        model.commitResize(column: 1, share: 0.5)
        let widths = model.activeWorkspace.columns.map(\.widthFraction)
        XCTAssertEqual(widths[0], 0.5, accuracy: 0.0001)
        XCTAssertEqual(widths.reduce(0, +), 1, accuracy: 0.0001)
        XCTAssertTrue(model.widthOverrides.isEmpty)
        model.run("column.equalize")
        XCTAssertEqual(model.activeWorkspace.columns[0].widthFraction, 1.0 / 3, accuracy: 0.0001)
        let revision = model.session.revision
        _ = try? model.handle.apply(commandId: "tab.record_scroll", args: ShellCommandArgs(workspaceId: "w1", tabId: "t1", segmentId: "John.3.17", scalar: 0), hasSelection: false)
        model.refresh()
        XCTAssertEqual(model.session.revision, revision, "recording a scroll anchor is quiet")
        XCTAssertEqual(model.activeTab?.scrollSegmentId, "John.3.17")
    }

    /// The Swift tokens against design/tokens/shell.json, once the web branch's file is in this
    /// tree (it lands with the web shell).
    func testTokensMatchTheSharedTokenFile() throws {
        let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .appendingPathComponent("../../../../../design/tokens/shell.json").standardized
        guard let data = try? Data(contentsOf: url) else { throw XCTSkip("design/tokens/shell.json is not in this tree yet") }
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let reading = try XCTUnwrap(json["reading"] as? [String: Any])
        XCTAssertEqual((reading["sizes"] as? [Double])?.map { CGFloat($0) }, ShellTokens.readingSizes)
        XCTAssertEqual(reading["default_size"] as? Double, Double(ShellTokens.readingSizes[ShellTokens.defaultReadingSizeIndex]))
        XCTAssertEqual((reading["line_spacings"] as? [Double])?.map { CGFloat($0) }, ShellTokens.lineSpacings)
        let caps = try XCTUnwrap(reading["measure_cap"] as? [String: Any])
        let constants = layoutConstants()
        XCTAssertEqual(caps["standard"] as? Double, constants.measureCapStandardPt)
        XCTAssertEqual(caps["accessibility"] as? Double, constants.measureCapAccessibilityPt)
        let themes = try XCTUnwrap(json["themes"] as? [String: [String: String]])
        for theme in ShellTokens.themes {
            let entry = try XCTUnwrap(themes[theme.id], theme.id)
            XCTAssertEqual(entry["page"], ShellTokens.hex(theme.page), theme.id)
            XCTAssertEqual(entry["text"], ShellTokens.hex(theme.text), theme.id)
            XCTAssertEqual(entry["chrome"], ShellTokens.hex(theme.chrome), theme.id)
            XCTAssertEqual(entry["rule"], ShellTokens.hex(theme.rules), theme.id)
            XCTAssertEqual(entry["accent"], ShellTokens.hex(theme.accent), theme.id)
        }
        let highlights = try XCTUnwrap(json["highlights"] as? [String: [String: Any]])
        for swatch in ShellTokens.swatches {
            XCTAssertEqual(highlights[swatch.id]?["color"] as? String, ShellTokens.hex(swatch.rgb), swatch.id)
            XCTAssertEqual(highlights[swatch.id]?["opacity"] as? Double, swatch.alpha, swatch.id)
        }
        XCTAssertEqual((json["spacing"] as? [Double])?.map { CGFloat($0) }, [ShellTokens.Spacing.xs, ShellTokens.Spacing.s, ShellTokens.Spacing.m, ShellTokens.Spacing.l, ShellTokens.Spacing.xl])
        let radius = try XCTUnwrap(json["radius"] as? [String: Double])
        XCTAssertEqual(radius["control"], Double(ShellTokens.Radius.control))
        XCTAssertEqual(radius["tool_well"], Double(ShellTokens.Radius.toolWell))
        // `$…` keys are notes; the metrics are the numbers.
        let m = try XCTUnwrap(json["metrics"] as? [String: Any]).compactMapValues { $0 as? Double }
        XCTAssertEqual(m["palette_field_height"], Double(ShellTokens.paletteHeight))
        XCTAssertEqual(m["palette_results_max_fraction"], Double(ShellTokens.paletteResultsMaxFraction))
        XCTAssertEqual(m["palette_dropdown_max_width"], Double(ShellTokens.paletteDropdownMaxWidth))
        XCTAssertEqual(m["tab_strip_height"], Double(ShellTokens.tabStripHeight))
        XCTAssertEqual(m["identity_row_height"], Double(ShellTokens.identityRowHeight))
        XCTAssertEqual(m["column_badge"], Double(ShellTokens.badgeSide))
        XCTAssertEqual(m["column_rule"], Double(ShellTokens.rule))
        XCTAssertEqual(m["shown_tab_underline"], Double(ShellTokens.accentUnderline))
        XCTAssertEqual(m["tool_well_width"], Double(ShellTokens.toolWellWidth))
        XCTAssertEqual(m["tool_well_collapsed"], Double(ShellTokens.toolGripSide))
        XCTAssertEqual(m["tool_well_inset"], Double(ShellTokens.toolWellInset))
        XCTAssertEqual(m["tool_button"], Double(ShellTokens.toolButtonSide))
        let tools = try XCTUnwrap(json["tools"] as? [[String: String]])
        XCTAssertEqual(tools.map { $0["command"] ?? "" }, ToolWell.tools.map(\.command))
        XCTAssertEqual(tools.map { $0["mac_symbol"] ?? "" }, ToolWell.tools.map(\.symbol))
        XCTAssertEqual(tools.map { $0["title"] ?? "" }, ToolWell.tools.map(\.name))
    }

    // MARK: Palette

    func testThePaletteAnswersEveryKeystrokeInSections() {
        let model = ShellModel()
        model.focusPalette()
        XCTAssertTrue(model.paletteOpen)
        for prefix in ["J", "Jn", "Jn ", "Jn 3", "Jn 3:", "Jn 3:16", "Jn 3:16 ", "Jn 3:16 @", "Jn 3:16 @2"] {
            model.paletteText = prefix
            model.paletteTextChanged()
            XCTAssertNotNil(model.paletteResult, prefix)
        }
        XCTAssertEqual(model.paletteResult?.targetColumn, 2)
        model.paletteText = "move tab to"
        model.paletteTextChanged()
        let moves = model.paletteRows.filter { $0.commandId == "tab.move_to_column" && $0.disabledReason == nil }
        XCTAssertFalse(moves.isEmpty, "a command typed without its column expands into one row per column")
        XCTAssertFalse(moves.contains { $0.args.column == 1 }, "not the current column")
        model.paletteText = "Jn 3:16 @2"
        model.paletteTextChanged()
        let first = try! XCTUnwrap(model.paletteRows.first)
        XCTAssertEqual(first.section, .passage)
        XCTAssertEqual(first.commandId, "nav.navigate")
        model.runSelectedRow()
        XCTAssertFalse(model.paletteOpen, "Enter runs and closes")
        XCTAssertEqual(model.activeWorkspace.activeColumnNumber, 2)

        model.focusPalette()
        model.paletteText = ">new"
        model.paletteTextChanged()
        XCTAssertEqual(Set(model.paletteResult?.groups.map(\.section) ?? []), [.commands])
        XCTAssertTrue(model.paletteRows.contains { $0.commandId == "tab.new" && $0.shortcut == "Alt+T" })
        model.movePaletteSelection(1)
        XCTAssertEqual(model.paletteSelection, 1)
        model.movePaletteSelection(-2)
        XCTAssertEqual(model.paletteSelection, model.paletteRows.count - 1, "↑ wraps")

        model.paletteText = ">undo"
        model.paletteTextChanged()
        model.closePalette()
        model.focusPalette()
        model.paletteText = ">highlight selection"
        model.paletteTextChanged()
        let disabled = model.paletteRows.first { $0.commandId == "selection.highlight" }
        XCTAssertNotNil(disabled?.disabledReason, "a selection command is listed, disabled with its reason")
        model.runRow(try! XCTUnwrap(disabled))
        XCTAssertTrue(model.paletteOpen, "a disabled row does not run")

        model.paletteText = ""
        model.paletteTextChanged()
        model.paletteText = "o"
        model.paletteTextChanged()
        let sections = model.paletteResult?.groups.map(\.section) ?? []
        if let section = sections.first(where: { s in (model.paletteResult?.groups.first { $0.section == s }).map { $0.total > UInt32($0.items.count) } ?? false }) {
            let before = model.paletteRows.filter { $0.section == section }.count
            model.paletteSelection = model.paletteRows.firstIndex { $0.section == section } ?? 0
            model.toggleExpandSelectedSection()
            XCTAssertGreaterThan(model.paletteRows.filter { $0.section == section }.count, before, "⌥Enter shows the whole section")
        }
        model.closePalette()
        XCTAssertNil(model.paletteResult)
    }

    func testTabsAcrossWorkspacesComeFromThePalette() {
        let model = ShellModel()
        model.focusPalette()
        model.paletteText = "@hebrew"
        model.paletteTextChanged()
        let row = try! XCTUnwrap(model.paletteRows.first)
        XCTAssertEqual(row.commandId, "workspace.switch")
        model.runRow(row)
        XCTAssertEqual(model.activeWorkspace.title, "Hebrew")
        model.focusPalette()
        model.paletteText = "@jn"
        model.paletteTextChanged()
        XCTAssertEqual(model.paletteRows.filter { $0.section == .tabs }.map(\.title).prefix(2), ["BSB · Jn 3:16", "SBLGNT · Jn 3:16"],
                       "tab rows name the resource as the tab strip does")
    }

    // MARK: Text

    func testTheTextSourceAnswersFixturesAndNothingElse() {
        let source = FixtureTextSource()
        XCTAssertEqual(source.text(resourceID: "bsb", reference: "John.3.16")?.content.verses.map(\.id), ["John.3.16", "John.3.17"])
        XCTAssertEqual(source.text(resourceID: "sblgnt", reference: "John.3.16")?.content.editionId, "sblgnt")
        XCTAssertEqual(source.text(resourceID: "wlc", reference: "Gen.1.1")?.content.isRightToLeft, true)
        XCTAssertNotNil(source.text(resourceID: "fixture:note:1", reference: nil))
        XCTAssertNil(source.text(resourceID: "bsb", reference: "Gen.1.1"), "no BSB Genesis in the fixtures: the placeholder")
        XCTAssertNil(source.text(resourceID: "kjv", reference: "John.3.16"))
        XCTAssertEqual(ShellReference.human("John.3.16"), "John 3:16")
        XCTAssertEqual(ShellReference.short("John.3.16"), "Jn 3:16")
    }

    // MARK: Rendered

    private func host(_ model: ShellModel, size: CGSize = CGSize(width: 1440, height: 900)) -> NSWindow {
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: ShellView(model: model))
        window.makeKeyAndOrderFront(nil)
        window.contentView?.layoutSubtreeIfNeeded()
        window.displayIfNeeded()
        return window
    }

    private func spin(_ seconds: TimeInterval = 0.4) {
        RunLoop.main.run(until: Date().addingTimeInterval(seconds))
    }

    func testTheShellRendersEveryShownTabOnTextKit2() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        for column in model.activeWorkspace.columns {
            let surface = try XCTUnwrap(model.surface(for: column.shownTabId), "column \(column.number)")
            XCTAssertTrue(surface.usesTextKit2)
            if let layout = surface.layout, let marker = layout.markers.first {
                let size = (layout.display.attribute(.font, at: marker.range.location, effectiveRange: nil) as? NSFont)?.pointSize ?? 0
                XCTAssertLessThan(size, 19, "the verse number is subordinate")
            }
        }
        let first = try XCTUnwrap(model.surface(for: "t1"))
        XCTAssertEqual(first.topSegmentID, "John.3.16")
        XCTAssertTrue(first.usesTextKit2, "reading the scroll anchor keeps TextKit 2")

        // A tab switch makes a new surface for the revealed tab and releases the hidden one.
        model.run("tab.next")
        spin(0.2)
        XCTAssertNotNil(model.surface(for: "t2"))
        model.run("view.text_size_up")
        spin(0.2)
        XCTAssertEqual(model.surface(for: "t2")?.layout?.display.attribute(.font, at: (model.surface(for: "t2")?.layout?.markers.first?.range.upperBound ?? 0) + 1, effectiveRange: nil).map { ($0 as! NSFont).pointSize }, 21)
    }

    func testHighlightAndCollectDragsReachTheModel() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        let surface = try XCTUnwrap(model.surface(for: "t1"))
        let layout = try XCTUnwrap(surface.layout)

        model.run("tool.highlight")
        spin(0.1)
        XCTAssertEqual(surface.dragMode, .highlight)
        surface.select(display: layout.runMap.displayRange(fromCore: try XCTUnwrap(layout.coreRange(ofPhrase: "loved the world"))))
        surface.dragEnded()
        XCTAssertEqual(model.marks["t1"]?.count, 1)
        XCTAssertEqual(surface.selectedRange.length, 0, "the selection collapses into the highlight")
        let painted = model.marks["t1"]![0].ranges[0]
        XCTAssertEqual((layout.display.string as NSString).substring(with: painted), "loved the world")

        model.run("tool.collect")
        spin(0.1)
        surface.select(display: layout.runMap.displayRange(fromCore: try XCTUnwrap(layout.coreRange(ofPhrase: "eternal life"))))
        surface.dragEnded()
        XCTAssertEqual(model.toast, "Collected: Jn 3:16")

        model.run("tool.select")
        spin(0.1)
        surface.select(display: layout.runMap.displayRange(fromCore: try XCTUnwrap(layout.coreRange(ofPhrase: "the world"))))
        surface.dragEnded()
        XCTAssertGreaterThan(surface.selectedRange.length, 0, "Select keeps the native selection")
        XCTAssertNotNil(model.selection)
        XCTAssertNil(model.disabledReason("selection.highlight"), "with a selection the selection commands are enabled")
    }

    func testInkFollowsTheTool() {
        let model = ShellModel()
        model.run("tool.pen")
        XCTAssertTrue(model.inkActive)
        let sheet = model.inkSheet(for: "t1")
        XCTAssertEqual(sheet.tool, .pen)
        model.run("tool.highlighter_pen")
        XCTAssertEqual(sheet.tool, .highlighter)
        XCTAssertEqual(sheet.hex(sheet.color), "#F5D90A", "the highlighter pen uses the current swatch")
        model.cycleInkWidth()
        XCTAssertEqual(sheet.width, 6, "tool.width_up in the core, applied to the sheet")
        model.run("tool.eraser")
        XCTAssertEqual(sheet.tool, .eraser)
        model.run("tool.select")
        XCTAssertFalse(model.inkActive)
    }

    func testEscapeReturnsFocusToTheActiveTab() throws {
        let model = ShellModel()
        let window = host(model)
        defer { window.close() }
        spin()
        model.focusPalette()
        spin(0.2)
        model.closePalette()
        spin(0.2)
        let surface = try XCTUnwrap(model.surface(for: "t1"))
        XCTAssertTrue(window.firstResponder === surface.textView, "Esc returns focus to the tab that had it")
    }
}
