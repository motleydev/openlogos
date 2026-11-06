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
//   -OPENLOGOS_MENU "word:2:ἠγάπησεν"  B077: opens the context menu on a target: word:N:phrase,
//                                       selection:N:phrase, tab:N:index, note_link:pane,
//                                       column:N, background:N; with -OPENLOGOS_SNAPSHOT the
//                                       picture is taken while the menu is open
//   -OPENLOGOS_MENU_SUBMENU Highlight   walks the open menu with ↓ and opens that row's submenu (→)
//   -OPENLOGOS_MENU_CHOOSE "in this edition"  chooses the first row whose label contains it, instead of
//                                       showing the menu
//   -OPENLOGOS_TYPE "John 3:16"         B078: types a passage in the palette and runs its first row,
//                                       after the columns have laid out (so the flash is on screen)
//   -OPENLOGOS_SCROLL "1:900"           B078: scrolls column 1's reading stream by 900 pt, as the
//                                       reader would (40 pt steps, each reported); negative is up
//   -OPENLOGOS_SHELL_MEASURE <path>     the specs/22 §7 run over ten workspaces, JSON to <path>, quit
//   -OPENLOGOS_SNAPSHOT <path>          the window's content drawn by the app itself into a PNG, quit
//                                       (no screen-recording permission needed)
// B074's store arguments are in StoreLaunch.swift; B071's pack argument in PreviewPack.swift.
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
        // B074: building the acceptance store, or timing a restore, quits here.
        if await StoreLaunch.early(model) { return }
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
        if let text = defaults.string(forKey: "OPENLOGOS_TYPE") {
            model.focusPalette()
            model.paletteText = text
            model.paletteTextChanged()
            model.runSelectedRow()
            try? await Task.sleep(nanoseconds: 100_000_000)
        }
        #if os(macOS)
        for spec in (defaults.string(forKey: "OPENLOGOS_SCROLL") ?? "").split(separator: ",") {
            let parts = spec.split(separator: ":").map(String.init)
            guard parts.count == 2, let number = UInt32(parts[0]), var remaining = Double(parts[1]),
                  let column = model.activeWorkspace.columns.first(where: { $0.number == number }),
                  let stream = model.stream(for: column.shownTabId) else { continue }
            while abs(remaining) > 0.5 {
                let step = remaining > 0 ? min(40, remaining) : max(-40, remaining)
                stream.scrollAsReader(by: step)
                remaining -= step
                try? await Task.sleep(nanoseconds: 8_000_000)
            }
            try? await Task.sleep(nanoseconds: 300_000_000)
        }
        #endif
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
        #if os(macOS)
        await StoreLaunch.late(model)
        if let spec = defaults.string(forKey: "OPENLOGOS_MENU") {
            try? await Task.sleep(nanoseconds: 300_000_000)
            openMenu(spec, model: model)
        }
        #endif
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
    /// The shown note being edited: a heading, a bold word, a passage link and a quote, focus
    /// in it.
    static func noteDemo(_ model: ShellModel) {
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
        editor.typingAttributes[.font] = editor.typography.body
        editor.insertText(" the world: see ", replacementRange: editor.selectedRange())
        model.run("note.link_passage", args: ShellCommandArgs(reference: "John.3.16"))
        // What Note on it leaves: the quote paragraph with its rule and passage link.
        model.notes.appendQuote("loved the world", reference: "John.3.16", to: column.tabs.first { $0.id == column.shownTabId }?.contentId ?? ShellModel.defaultNoteID)
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
    /// The B077 menu on a target (`word:2:ἠγάπησεν`, …), popped up where a right-click on it
    /// would show it. With a snapshot path the picture is taken while it is open (the menu's
    /// own windows composited over the app's), then the app quits.
    static func openMenu(_ spec: String, model: ShellModel) {
        let parts = spec.split(separator: ":", maxSplits: 2).map(String.init)
        guard let kind = parts.first else { return }
        var menu: NSMenu?
        var view: NSView?
        var point = NSPoint.zero
        let column = parts.count > 1 ? UInt32(parts[1]) : nil
        switch kind {
        case "word", "selection":
            guard parts.count == 3, let (surface, range) = phrase("\(parts[1]):\(parts[2])", in: model), let text = surface.textView,
                  let number = column, let col = model.activeWorkspace.columns.first(where: { $0.number == number }),
                  let tab = col.tabs.first(where: { $0.id == col.shownTabId }) else { return }
            if kind == "selection" { surface.select(display: range) }
            guard let box = surface.rect(for: range) else { return }
            point = NSPoint(x: box.midX, y: box.midY)
            menu = model.nsMenu(for: surface.contextHit(at: point), tab: tab, column: col)
            view = text
            point = NSPoint(x: box.minX + 6, y: box.maxY + 2)
        case "tab":
            guard let number = column, let col = model.activeWorkspace.columns.first(where: { $0.number == number }),
                  let index = parts.count > 2 ? Int(parts[2]) : 0, let tab = col.tabs[safe: index],
                  let catcher = model.catchers.view("tab-\(tab.id)") else { return }
            menu = model.nsMenu(for: .tab(tabId: tab.id))
            view = catcher
            point = NSPoint(x: 8, y: catcher.isFlipped ? catcher.bounds.maxY : 0)
        case "column", "background":
            guard let number = column, let catcher = model.catchers.view("\(kind)-\(number)") else { return }
            menu = model.nsMenu(for: kind == "column" ? .column(column: number) : .background(column: number))
            view = catcher
            point = kind == "column" ? NSPoint(x: 12, y: catcher.isFlipped ? catcher.bounds.maxY : 0)
                : NSPoint(x: catcher.bounds.midX - 120, y: catcher.isFlipped ? 180 : catcher.bounds.maxY - 180)
        case "note_link":
            let pane = parts.count > 1 ? parts[1] : "dock"
            guard let editor = model.notes.editor(pane: pane), let storage = editor.textStorage else { return }
            var found: NSRange?
            storage.enumerateAttribute(.link, in: NSRange(location: 0, length: storage.length)) { value, range, stop in
                if value != nil, found == nil { found = range; stop.pointee = true }
            }
            guard let range = found else { return }
            editor.window?.makeFirstResponder(editor)
            menu = model.nsMenu(for: editor.hit(at: range.location), editor: editor)
            view = editor
            let caret = editor.firstRect(forCharacterRange: range, actualRange: nil)
            point = editor.window.map { editor.convert($0.convertPoint(fromScreen: NSPoint(x: caret.minX, y: caret.minY)), from: nil) } ?? .zero
        default:
            return
        }
        guard let menu, let view else { return }
        // Keep the menu inside the window, where the picture can show it.
        point.x = min(point.x, view.bounds.width - 250)
        if let label = defaults.string(forKey: "OPENLOGOS_MENU_CHOOSE") {
            if let item = menu.items.lazy.flatMap({ [$0] + ($0.submenu?.items ?? []) }).first(where: { $0.title.contains(label) && $0.target is MenuDispatcher }) {
                (item.target as? MenuDispatcher)?.choose(item)
            }
            return
        }
        let path = defaults.string(forKey: "OPENLOGOS_SNAPSHOT")
        let submenu = defaults.string(forKey: "OPENLOGOS_MENU_SUBMENU")
        var ticks = 0
        let timer = Timer(timeInterval: 0.1, repeats: true) { timer in
            MainActor.assumeIsolated {
                ticks += 1
                if ticks == 2, let submenu {
                    walk(to: submenu, in: menu)
                }
                guard ticks >= 8 else { return }
                timer.invalidate()
                if let path {
                    snapshot(to: path)
                    NSApp.terminate(nil)
                }
            }
        }
        RunLoop.main.add(timer, forMode: .common)
        menu.popUp(positioning: nil, at: point, in: view)
    }

    /// Opens a submenu of the open menu the way the keyboard does: ↓ to its row, then →.
    private static func walk(to label: String, in menu: NSMenu) {
        let rows = menu.items.filter { !$0.isSeparatorItem && !$0.isSectionHeader && $0.isEnabled }
        guard let index = rows.firstIndex(where: { $0.title == label }) else { return }
        let keys: [(UInt16, String)] = Array(repeating: (125, String(UnicodeScalar(NSDownArrowFunctionKey)!)), count: index + 1)
            + [(124, String(UnicodeScalar(NSRightArrowFunctionKey)!))]
        for (code, characters) in keys {
            for type in [NSEvent.EventType.keyDown, .keyUp] {
                if let event = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                windowNumber: 0, context: nil, characters: characters, charactersIgnoringModifiers: characters,
                                                isARepeat: false, keyCode: code) {
                    NSApp.postEvent(event, atStart: false)
                }
            }
        }
    }

    /// Draws the window's content view, layers included, into a PNG at the backing scale,
    /// with any open menu's windows drawn over it where they are on screen.
    static func snapshot(to path: String) {
        // The frame view, so the toolbar row with the palette field is in the picture too.
        guard let window = NSApp.windows.first(where: { $0.isVisible && $0.level == .normal }) ?? NSApp.windows.first(where: { $0.isVisible }),
              let view = window.contentView?.superview ?? window.contentView else { return }
        let scale = window.backingScaleFactor
        let size = view.bounds.size
        guard let rep = bitmap(of: view, scale: scale) else { return }
        let overlays = NSApp.windows.filter { $0 !== window && $0.isVisible && $0.level.rawValue > NSWindow.Level.normal.rawValue }
        guard !overlays.isEmpty, let canvas = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: rep.pixelsWide, pixelsHigh: rep.pixelsHigh,
                                                                bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                                                                colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0) else {
            try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: path))
            return
        }
        canvas.size = size
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: canvas)
        rep.draw(in: NSRect(origin: .zero, size: size))
        for overlay in overlays {
            guard let content = overlay.contentView?.superview ?? overlay.contentView else { continue }
            // A menu's backdrop is Liquid Glass (backdrop and SDF layers), which `cacheDisplay`
            // draws as plain white: the picture leaves it out and fills the menu's shape with
            // the appearance's window background instead.
            hideGlass(content.layer)
            guard let image = bitmap(of: content, scale: scale) else { continue }
            let origin = NSPoint(x: overlay.frame.minX - window.frame.minX, y: overlay.frame.minY - window.frame.minY)
            overlay.effectiveAppearance.performAsCurrentDrawingAppearance {
                NSColor.windowBackgroundColor.setFill()
                NSBezierPath(roundedRect: NSRect(origin: origin, size: overlay.frame.size).insetBy(dx: 1, dy: 1), xRadius: 10, yRadius: 10).fill()
            }

            image.draw(in: NSRect(origin: origin, size: overlay.frame.size), from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: nil)
        }
        NSGraphicsContext.restoreGraphicsState()
        try? canvas.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: path))
    }

    private static func hideGlass(_ layer: CALayer?) {
        guard let layer else { return }
        let name = String(describing: type(of: layer))
        if ["Backdrop", "SDF", "Portal"].contains(where: name.contains) { layer.isHidden = true }
        layer.sublayers?.forEach(hideGlass)
    }

    private static func bitmap(of view: NSView, scale: CGFloat) -> NSBitmapImageRep? {
        let size = view.bounds.size
        guard size.width > 0, size.height > 0,
              let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(size.width * scale), pixelsHigh: Int(size.height * scale),
                                         bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                                         colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0) else { return nil }
        rep.size = size
        view.cacheDisplay(in: view.bounds, to: rep)
        return rep
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
