// B072: launch arguments for pictures and measurements (scripts/b072-mac.sh drives them):
//   -OPENLOGOS_WINDOW 1440x900          the window's content size, at the screen's top left
//   -OPENLOGOS_THEME light|sepia|dark   the shell theme
//   -OPENLOGOS_PALETTE "text"           opens the palette with this text typed
//   -OPENLOGOS_PALETTE_EXPAND commands  expands a section (passage, commands, tabs, text)
//   -OPENLOGOS_TOOL pen                 runs tool.<name>
//   -OPENLOGOS_SHELL_DEMO 1             a highlight, ink strokes and a collect toast on screen
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
        // Let the columns lay out before anything reads the surfaces.
        try? await Task.sleep(nanoseconds: 600_000_000)
        if defaults.bool(forKey: "OPENLOGOS_SHELL_DEMO") {
            demo(model)
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
        guard let window = NSApp.windows.first(where: { $0.isVisible }), let view = window.contentView else { return }
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
