// B072: launch arguments for pictures and measurements (scripts/b072-mac.sh drives them):
//   -OPENLOGOS_WINDOW 1440x900          the window's content size, at the screen's top left
//   -OPENLOGOS_THEME light|sepia|dark   the shell theme
//   -OPENLOGOS_PALETTE "text"           opens the palette with this text typed
//   -OPENLOGOS_PALETTE_EXPAND commands  expands a section (passage, commands, tabs, text)
//   -OPENLOGOS_TOOL pen                 runs tool.<name>
//   -OPENLOGOS_RUN "a,b:arg"            runs registered commands in order; `:arg` is a column
//                                       number, else a reference (nav.navigate) or text
//   -OPENLOGOS_SHELL_DEMO 1             a highlight, ink strokes and a collect toast on screen
//   -OPENLOGOS_SELECT "2:the world"     selects the phrase in column 2's text (the selection group)
//   -OPENLOGOS_WORD "2:ἠγάπησεν"        a Word study click on the word in column 2
//   -OPENLOGOS_NOTE_DEMO 1              types into the shown note: a bold word and a passage link
//   -OPENLOGOS_FOCUS dock               focus in the notes dock's editor
//   -OPENLOGOS_SHELL_MEASURE <path>     the specs/22 §7 run over ten workspaces, JSON to <path>, quit
//   -OPENLOGOS_SNAPSHOT <path>          the window's content drawn by the app itself into a PNG, quit
//                                       (no screen-recording permission needed)
import SwiftUI
#if os(macOS)
import AppKit
#endif

private extension Double {
    func clamped(_ low: Double, _ high: Double) -> Double { self == 0 ? 1.5 : Swift.min(high, Swift.max(low, self)) }
}

@MainActor
enum ShellLaunch {
    static var defaults: UserDefaults { .standard }

    static var measurementPath: String? { defaults.string(forKey: "OPENLOGOS_SHELL_MEASURE") }

    static func apply(to model: ShellModel) async {
        #if os(macOS)
        if let size = defaults.string(forKey: "OPENLOGOS_WINDOW")?.split(separator: "x").compactMap({ Double($0) }), size.count == 2 {
            sizeWindow(width: size[0], height: size[1])
        }
        #endif
        if let name = defaults.string(forKey: "OPENLOGOS_THEME") {
            model.run("view.theme", args: ShellCommandArgs(text: name))
        }
        if let tool = defaults.string(forKey: "OPENLOGOS_TOOL") {
            model.run("tool.\(tool)")
        }
        for step in (defaults.string(forKey: "OPENLOGOS_RUN") ?? "").split(separator: ",") {
            let parts = step.split(separator: ":", maxSplits: 1).map(String.init)
            var args = ShellCommandArgs()
            if parts.count == 2 {
                if let n = UInt32(parts[1]) { args.column = n } else if parts[0] == "nav.navigate" { args.reference = parts[1] } else { args.text = parts[1] }
            }
            model.run(parts[0], args: args)
        }
        // Let the columns lay out before anything reads the surfaces.
        try? await Task.sleep(nanoseconds: 600_000_000)
        if defaults.bool(forKey: "OPENLOGOS_SHELL_DEMO") {
            demo(model)
        }
        if let target = defaults.string(forKey: "OPENLOGOS_WORD"), let (surface, range) = phrase(target, in: model) {
            surface.dragMode = .word
            surface.select(display: range)
            surface.dragEnded()
            surface.dragMode = model.dragMode
            try? await Task.sleep(nanoseconds: 400_000_000)
        }
        #if os(macOS)
        if defaults.bool(forKey: "OPENLOGOS_NOTE_DEMO") {
            noteDemo(model)
        }
        if defaults.string(forKey: "OPENLOGOS_FOCUS") == "dock", let editor = model.notes.editor(pane: "dock") {
            editor.window?.makeFirstResponder(editor)
        }
        #endif
        if let target = defaults.string(forKey: "OPENLOGOS_SELECT"), let (surface, range) = phrase(target, in: model) {
            surface.select(display: range)
        }
        if let text = defaults.string(forKey: "OPENLOGOS_PALETTE") {
            model.focusPalette()
            model.paletteText = text
            model.paletteTextChanged()
            if let section = defaults.string(forKey: "OPENLOGOS_PALETTE_EXPAND").flatMap(Self.section) {
                model.expand(section)
            }
        }
        if let path = defaults.string(forKey: "OPENLOGOS_SNAPSHOT") {
            try? await Task.sleep(nanoseconds: UInt64(defaults.double(forKey: "OPENLOGOS_SNAPSHOT_DELAY").clamped(0.3, 10) * 1_000_000_000))
            #if os(macOS)
            snapshot(to: path)
            NSApp.terminate(nil)
            #endif
        }
        if let path = measurementPath {
            let report = await ShellMeasure(model: model).run()
            ShellMeasure.write(report, to: path)
            #if os(macOS)
            NSApp.terminate(nil)
            #endif
        }
    }

    static func section(_ name: String) -> ShellPaletteSection? {
        switch name {
        case "passage": return .passage
        case "commands": return .commands
        case "tabs": return .tabs
        case "text": return .text
        default: return nil
        }
    }

    /// `2:the world`: column 2's surface and the phrase's display range in it.
    private static func phrase(_ target: String, in model: ShellModel) -> (ReaderSurface, NSRange)? {
        let parts = target.split(separator: ":", maxSplits: 1).map(String.init)
        guard parts.count == 2, let number = UInt32(parts[0]),
              let column = model.activeWorkspace.columns.first(where: { $0.number == number }),
              let surface = model.surface(for: column.shownTabId), let layout = surface.layout,
              let range = layout.coreRange(ofPhrase: parts[1]) else { return nil }
        return (surface, layout.runMap.displayRange(fromCore: range))
    }

    #if os(macOS)
    /// The shown note being edited: a heading, a bold word and a passage link, focus in it.
    private static func noteDemo(_ model: ShellModel) {
        guard let column = model.activeWorkspace.columns.first(where: { c in c.tabs.first { $0.id == c.shownTabId }?.kind == "notes" }),
              let editor = model.notes.editor(pane: column.shownTabId), let storage = editor.textStorage else { return }
        editor.window?.makeFirstResponder(editor)
        model.focusColumn(column.number)
        editor.setSelectedRange(NSRange(location: 0, length: 0))
        editor.insertText("Love that gives\n", replacementRange: editor.selectedRange())
        editor.setSelectedRange(NSRange(location: 0, length: 0))
        model.run("note.heading")
        editor.setSelectedRange(NSRange(location: storage.length, length: 0))
        editor.insertText("\nGod ", replacementRange: editor.selectedRange())
        let start = storage.length
        editor.insertText("so loved", replacementRange: editor.selectedRange())
        editor.setSelectedRange(NSRange(location: start, length: 8))
        model.run("note.bold")
        editor.setSelectedRange(NSRange(location: storage.length, length: 0))
        editor.insertText(" the world: see ", replacementRange: editor.selectedRange())
        model.run("note.link_passage", args: ShellCommandArgs(reference: "John.3.16"))
    }
    #endif

    /// A highlight in column 1, ink in column 2 and a collect toast: what the tools do.
    private static func demo(_ model: ShellModel) {
        let ws = model.activeWorkspace
        if let first = ws.columns.first, let surface = model.surface(for: first.shownTabId), let layout = surface.layout,
           let range = layout.coreRange(ofPhrase: "loved the world") {
            surface.dragMode = .highlight
            surface.select(display: layout.runMap.displayRange(fromCore: range))
            surface.dragEnded()
            surface.dragMode = model.dragMode
        }
        if ws.columns.count > 1 {
            let sheet = model.inkSheet(for: ws.columns[1].shownTabId)
            sheet.addDemoStrokes(in: CGSize(width: 420, height: 260))
            model.objectWillChange.send()
        }
        if let first = ws.columns.first, let surface = model.surface(for: first.shownTabId), let layout = surface.layout,
           let range = layout.coreRange(ofPhrase: "eternal life") {
            surface.dragMode = .collect
            surface.select(display: layout.runMap.displayRange(fromCore: range))
            surface.dragEnded()
            surface.dragMode = model.dragMode
        }
    }

    #if os(macOS)
    /// Draws the window's content view, layers included, into a PNG at the backing scale.
    static func snapshot(to path: String) {
        // The frame view, so the toolbar row with the palette field is in the picture too.
        guard let window = NSApp.windows.first(where: { $0.isVisible }), let view = window.contentView?.superview ?? window.contentView else { return }
        let scale = window.backingScaleFactor
        let size = view.bounds.size
        guard let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(size.width * scale), pixelsHigh: Int(size.height * scale),
                                         bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                                         colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0) else { return }
        rep.size = size
        view.cacheDisplay(in: view.bounds, to: rep)
        try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: path))
    }

    static func sizeWindow(width: Double, height: Double) {
        guard let window = NSApp.windows.first(where: { $0.isVisible }) ?? NSApp.windows.first,
              let screen = window.screen ?? NSScreen.main else { return }
        let frame = window.frameRect(forContentRect: NSRect(x: 0, y: 0, width: width, height: height))
        let origin = NSPoint(x: screen.visibleFrame.minX, y: screen.visibleFrame.maxY - frame.height)
        window.setFrame(NSRect(origin: origin, size: frame.size), display: true)
    }
    #endif
}
