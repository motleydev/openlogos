// B074: launch arguments for the user store's pictures and measurements (scripts/b074-mac-shots.sh
// drives them). Every one of them needs `-OPENLOGOS_STORE <directory>`; without it a scripted
// run keeps no store, so the real Application Support file is never written by them.
//   -OPENLOGOS_STORE <dir>              the store directory (user-store.json and its backups)
//   -OPENLOGOS_BUILD_STORE 1            writes the acceptance store (10 workspaces × 12 tabs,
//                                       200 notes, 1,000 highlights, 200 strokes, built through
//                                       the API) into the store directory, then quits
//   -OPENLOGOS_STORE_DEMO 1             the owner's scenario: three workspaces, column 2 at Acts 2,
//                                       a note with bold and a passage link, a highlight on
//                                       BSB John 3:16, ink on John 3:17, the Highlight tool
//   -OPENLOGOS_DRAG "t2>2:0"            drags tab t2 to column 2, slot 0 (`>new` a new column)
//                                       through the drag's own path; -OPENLOGOS_DRAG_HOLD 1 keeps
//                                       the pointer down (the picture shows the slot)
//   -OPENLOGOS_THEN "edit.undo"         commands run after the drag
//   -OPENLOGOS_RESTORE_MEASURE <path>   appends file read → first painted text as a JSON line, quits
//   -OPENLOGOS_STORE_MEASURE <path>     saves, workspace commands and drags at the store's size,
//                                       JSON to <path>, quits
#if os(macOS)
import AppKit
import QuartzCore
import SwiftUI

@MainActor
enum StoreLaunch {
    static var defaults: UserDefaults { .standard }

    /// Before the columns are used: the store's own scripted steps. True when the run quits.
    static func early(_ model: ShellModel) async -> Bool {
        if defaults.bool(forKey: "OPENLOGOS_BUILD_STORE"), let file = model.persistence?.file {
            let json = buildAcceptanceStore()
            file.writeNow(json)
            print("acceptance store: \(json.utf8.count) bytes at \(file.url.path)")
            NSApp.terminate(nil)
            return true
        }
        if let path = defaults.string(forKey: "OPENLOGOS_RESTORE_MEASURE") {
            for _ in 0..<200 where ShellPersistence.firstPaint == nil {
                try? await Task.sleep(nanoseconds: 10_000_000)
            }
            let restored = model.restored
            let line: [String: Any] = [
                "bytes": restored?.bytes ?? 0, "read_ms": restored?.readMs ?? 0, "import_ms": restored?.importMs ?? 0,
                "laid_out_ms": ShellPersistence.firstPaint?.laidOut ?? -1, "first_frame_ms": ShellPersistence.firstPaint?.frame ?? -1,
                "workspaces": model.session.workspaces.count, "notice": restored?.notice ?? "",
                "highlights_ms": ShellPersistence.launchCosts["highlights"] ?? 0, "ink_ms": ShellPersistence.launchCosts["ink"] ?? 0,
                "pack_open_ms": PreviewPack.openMs, "process_to_frame_ms": processToFrame(restored?.started, ShellPersistence.firstPaint?.frame),
                "frame_epoch": Date().timeIntervalSince1970 - (CACurrentMediaTime() - (restored?.started ?? 0) - (ShellPersistence.firstPaint?.frame ?? 0) / 1000),
                "relayout_ms": ReadingStream.timings.reduce(0) { $0 + ($1.0 + $1.1 + $1.2) * 1000 }, "relayouts": ReadingStream.timings.count,
            ]
            if let data = try? JSONSerialization.data(withJSONObject: line, options: [.sortedKeys]) {
                append(data + Data("\n".utf8), to: path)
            }
            NSApp.terminate(nil)
            return true
        }
        return false
    }

    /// After the columns have laid out: the demo, the drag and the measurement.
    static func late(_ model: ShellModel) async {
        if defaults.bool(forKey: "OPENLOGOS_STORE_DEMO") {
            await demo(model)
        }
        if let spec = defaults.string(forKey: "OPENLOGOS_DRAG") {
            await drag(spec, model: model, hold: defaults.bool(forKey: "OPENLOGOS_DRAG_HOLD"))
        }
        for id in (defaults.string(forKey: "OPENLOGOS_THEN") ?? "").split(separator: ",") {
            model.run(String(id))
            try? await Task.sleep(nanoseconds: 200_000_000)
        }
        if let path = defaults.string(forKey: "OPENLOGOS_STORE_MEASURE") {
            let report = await ShellMeasure(model: model, samples: 200).runStore()
            ShellMeasure.write(report, to: path)
            NSApp.terminate(nil)
        }
    }

    /// From the process's start (the kernel's clock) to the first frame with the restored text.
    private static func processToFrame(_ started: CFTimeInterval?, _ frameMs: Double?) -> Double {
        guard let started, let frameMs else { return -1 }
        var info = kinfo_proc()
        var size = MemoryLayout<kinfo_proc>.stride
        var name: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()]
        guard sysctl(&name, 4, &info, &size, nil, 0) == 0 else { return -1 }
        let start = info.kp_proc.p_starttime
        let began = Double(start.tv_sec) + Double(start.tv_usec) / 1_000_000
        let frameEpoch = Date().timeIntervalSince1970 - (CACurrentMediaTime() - started - frameMs / 1000)
        return (frameEpoch - began) * 1000
    }

    private static func append(_ data: Data, to path: String) {
        let url = URL(fileURLWithPath: path)
        if let handle = try? FileHandle(forWritingTo: url) {
            handle.seekToEndOfFile()
            handle.write(data)
            try? handle.close()
        } else {
            try? data.write(to: url)
        }
    }

    // MARK: The owner's scenario

    static func demo(_ model: ShellModel) async {
        func settle(_ seconds: Double = 0.4) async { try? await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000)) }
        // A third workspace: Romans 8 beside Psalm 23.
        model.run("workspace.new", args: ShellCommandArgs(text: "Romans"))
        model.run("nav.navigate", args: ShellCommandArgs(column: 1, reference: "Rom.8.1"))
        model.run("column.new")
        model.run("tab.pin")
        model.run("nav.navigate", args: ShellCommandArgs(column: 2, reference: "Ps.23.1"))
        await settle()
        model.run("workspace.switch", args: ShellCommandArgs(workspaceId: "w1"))
        await settle()
        // Column 2 on its own at Acts 2, scrolled by the reader.
        model.focusColumn(2)
        model.run("tab.pin")
        model.run("nav.navigate", args: ShellCommandArgs(column: 2, reference: "Acts.2.1"))
        await settle(0.6)
        let ws = model.activeWorkspace
        if let column = ws.columns.first(where: { $0.number == 2 }), let stream = model.stream(for: column.shownTabId) {
            var remaining: CGFloat = 360
            while remaining > 0 {
                stream.scrollAsReader(by: min(40, remaining))
                remaining -= 40
                try? await Task.sleep(nanoseconds: 8_000_000)
            }
        }
        await settle()
        // A highlight on BSB John 3:16 and ink on John 3:17 in column 1.
        model.focusColumn(1)
        if let first = ws.columns.first, let stream = model.stream(for: first.shownTabId), let layout = stream.surface.layout,
           let range = layout.coreRange(ofPhrase: "loved the world") {
            let surface = stream.surface
            surface.dragMode = .highlight
            surface.select(display: layout.runMap.displayRange(fromCore: range))
            surface.dragEnded()
            surface.dragMode = model.dragMode
            drawInk(model, stream: stream, on: "John.3.17")
        }
        // The note: a heading, a bold phrase, a passage link and a quote.
        ShellLaunch.noteDemo(model)
        model.notes.flush()
        model.focusColumn(1)
        model.run("tool.highlight")
        await settle()
    }

    /// A pen line under the first line of a verse, through the sheet's own commit.
    static func drawInk(_ model: ShellModel, stream: ReadingStream, on segment: String) {
        guard let box = stream.canvasBox(of: segment) else { return }
        let sheet = model.inkSheet(for: stream.tabID)
        let left = Double(box.maxX) + 4
        let y = Double(box.maxY) + 1
        let t0 = CACurrentMediaTime()
        let samples = (0..<60).map { i -> InkSample in
            let f = Double(i) / 59
            return InkSample(x: left + f * 220, y: y + sin(f * .pi * 3) * 3, pressure: nil, time: t0 + f * 0.4, altitude: nil, azimuth: nil)
        }
        if let stroke = sheet.makeStroke(samples: samples) { sheet.commit(stroke) }
        model.objectWillChange.send()
    }

    /// `t2>2:0`: the drag's own path (pointer positions from the laid-out tab frames).
    static func drag(_ spec: String, model: ShellModel, hold: Bool) async {
        let parts = spec.split(separator: ">").map(String.init)
        guard parts.count == 2, let from = model.tabFrames[parts[0]] else { return }
        var target: CGPoint?
        let ws = model.activeWorkspace
        if parts[1] == "new", let last = ws.columns.last, let frame = model.columnFrames[last.number] {
            target = CGPoint(x: frame.maxX - 20, y: frame.midY)
        } else {
            let slot = parts[1].split(separator: ":").compactMap { Int($0) }
            if slot.count == 2, let column = ws.columns.first(where: { $0.number == UInt32(slot[0]) }) {
                if let tab = column.tabs[safe: slot[1]], let frame = model.tabFrames[tab.id] {
                    target = CGPoint(x: frame.minX + 3, y: frame.midY)
                } else if let tab = column.tabs.last, let frame = model.tabFrames[tab.id] {
                    target = CGPoint(x: frame.maxX + 6, y: frame.midY)
                }
            }
        }
        guard let target else { return }
        let start = CGPoint(x: from.midX, y: from.midY)
        for i in 1...12 {
            let f = CGFloat(i) / 12
            model.dragTab(parts[0], to: CGPoint(x: start.x + (target.x - start.x) * f, y: start.y + (target.y - start.y) * f))
            try? await Task.sleep(nanoseconds: 16_000_000)
        }
        if !hold { model.dropTab() }
        try? await Task.sleep(nanoseconds: 300_000_000)
    }

    // MARK: The acceptance store

    /// Ten workspaces of twelve tabs (every tab scrolled), 200 notes of five blocks, 1,000
    /// highlights and 200 strokes of 120 points with every sensor: built through the API, as
    /// the core's bench builds it, and exported.
    static func buildAcceptanceStore() -> String {
        let handle = try! ShellHandle(sessionJson: ShellSeed.measurementJSON)
        handle.useFixtureText()
        handle.setResourceSchemes(schemes: ReadingHandle.placeholder().schemes())
        let session = handle.session()
        let chapters = ["John.1", "John.3", "John.5", "Gen.1", "Gen.2", "Acts.2", "Rom.8", "Ps.23"]
        for (w, ws) in session.workspaces.enumerated() {
            for column in ws.columns {
                for (t, tab) in column.tabs.enumerated() {
                    let chapter = chapters[(w + t) % chapters.count]
                    let resource = tab.contentId
                    let reference = resource == "wlc" ? "Gen.1.\(2 + t)" : "\(chapter.hasPrefix("Gen") && resource == "sblgnt" ? "John.3" : chapter).\(3 + t)"
                    _ = try? handle.apply(commandId: "tab.record_scroll",
                                          args: ShellCommandArgs(workspaceId: ws.id, tabId: tab.id, segmentId: reference, scalar: UInt32(t)),
                                          hasSelection: false)
                }
            }
        }
        for n in 0..<200 {
            let note = StoreNote(id: "note:\(n)", blocks: [
                StoreNoteBlock(kind: .heading, spans: [StoreNoteSpan(text: "Study \(n) "), StoreNoteSpan(text: "Jn 3:16", link: "John.3.16")]),
                StoreNoteBlock(kind: .paragraph, spans: [StoreNoteSpan(text: "Not by "), StoreNoteSpan(text: "works", italic: true),
                                                          StoreNoteSpan(text: " but "), StoreNoteSpan(text: "from above", bold: true), StoreNoteSpan(text: ".")]),
                StoreNoteBlock(kind: .listItem, spans: [StoreNoteSpan(text: "Compare "), StoreNoteSpan(text: "Jn 1:12–13", link: "John.1.12-John.1.13")]),
                StoreNoteBlock(kind: .quote, spans: [StoreNoteSpan(text: "A fixture quote, not Scripture, number \(n).")]),
                StoreNoteBlock(kind: .paragraph, spans: []),
            ])
            try? handle.putNote(note: note)
        }
        let colours = ShellTokens.swatches.map { ShellTokens.hex($0.rgb) }
        for n in 0..<1000 {
            let resource = ["bsb", "sblgnt", "bsb", "wlc"][n % 4]
            let segment = resource == "wlc" ? "Gen.\(1 + n % 3).\(1 + n % 25)" : "John.\(1 + n % 5).\(1 + n % 20)"
            let highlight = StoreHighlight(resourceId: resource, contentRevision: "0", spans: [StoreSegmentSpan(segmentId: segment, start: UInt32(n % 7), end: UInt32(n % 7 + 6))],
                                           color: colours[n % colours.count])
            _ = try? handle.addHighlight(highlight: highlight, reading: nil)
        }
        for n in 0..<200 {
            let points = (0..<120).map { i -> InkPoint in
                let f = Double(i) / 119
                return InkPoint(x: 20 + f * 300, y: sin(f * .pi * 4) * 8, pressure: 0.3 + 0.5 * f, elapsedMs: Double(i) * 8,
                                altitude: 0.9 + 0.2 * f, azimuth: 1 + f)
            }
            let stroke = InkStroke(id: "s\(n)", tool: .pen, brushVersion: InkBrush.pen.rawValue, width: 4, color: "#1F6FEB", opacity: 1, points: points)
            _ = try? handle.addInk(mark: StoreInkMark(resourceId: n % 2 == 0 ? "bsb" : "sblgnt", segmentId: "John.\(1 + n % 5).\(1 + n % 20)", stroke: stroke))
        }
        return handle.exportStore()
    }
}

extension ShellMeasure {
    /// B074 rows at the store's size: a save after a scroll, a stroke and a note edit (the export
    /// on the main thread and the write off it), and the workspace commands and a tab drag on
    /// screen (the command, then the window laid out and drawn).
    func runStore() async -> Report {
        let model = model
        guard let persistence = model.persistence else {
            return Report(host: "mac", window: "", workspaces: 0, tabs: 0, stats: [])
        }
        var stats: [Stat] = []
        if let restored = model.restored {
            stats.append(Self.stat("restore: read the file (\(restored.bytes) bytes)", budget: 100, [restored.readMs]))
            stats.append(Self.stat("restore: importStore", budget: 100, [restored.importMs]))
        }
        let ws = model.activeWorkspace
        let tab = ws.columns[0].shownTabId
        // A save: what changed, then `exportStore` on the main thread, timed; the write follows
        // off it and is awaited before the next sample.
        func saves(_ name: String, change: (Int) -> Void) async {
            var exports: [Double] = []
            var writes: [Double] = []
            for i in 0..<200 {
                change(i)
                let before = persistence.saves, writesBefore = persistence.writeSamples.count, exportsBefore = persistence.exportSamples.count
                persistence.save()
                for _ in 0..<500 where persistence.saves == before {
                    try? await Task.sleep(nanoseconds: 1_000_000)
                }
                if persistence.exportSamples.count > exportsBefore, let last = persistence.exportSamples.last { exports.append(last) }
                if persistence.writeSamples.count > writesBefore, let last = persistence.writeSamples.last { writes.append(last) }
            }
            stats.append(Self.stat("save after \(name): exportStore on the main thread", budget: 4, exports))
            stats.append(Self.stat("save after \(name): write off the main thread", budget: 100, writes))
        }
        let verses = ["John.3.16", "John.3.17", "John.3.18", "John.3.19"]
        await saves("a scroll") { i in
            model.recordScroll(tabID: tab, segmentID: verses[i % verses.count])
        }
        await saves("a stroke") { i in
            let points = (0..<60).map { j in InkPoint(x: Double(j) * 3, y: Double(j % 7), pressure: nil, elapsedMs: Double(j) * 8, altitude: nil, azimuth: nil) }
            _ = try? model.handle.addInk(mark: StoreInkMark(resourceId: "bsb", segmentId: "John.3.17",
                                                            stroke: InkStroke(id: "m\(i)", tool: .pen, brushVersion: InkBrush.pen.rawValue, width: 4, color: "#1F6FEB", opacity: 1, points: points)))
        }
        await saves("a note edit") { i in
            try? model.handle.putNote(note: StoreNote(id: "note:0", blocks: [StoreNoteBlock(kind: .paragraph, spans: [StoreNoteSpan(text: "Edit \(i)")])]))
        }
        // The workspace commands, each drawn on screen, at the store's size (what one sample
        // adds, the next one's preparation takes away).
        let ids = model.session.workspaces.map(\.id)
        let home = ShellCommandArgs(workspaceId: ids[0])
        stats.append(await measure("workspace new (at the store's size)", budget: 100, count: 200, prepare: { i in
            if i > 0 { model.run("workspace.close") }
            model.run("workspace.switch", args: home)
        }) { _ in
            model.run("workspace.new")
        })
        model.run("workspace.close")
        model.run("workspace.switch", args: home)
        stats.append(await measure("workspace rename", budget: 100, count: 200) { i in
            model.run("workspace.rename", args: ShellCommandArgs(text: "Renamed \(i)"))
        })
        stats.append(await measure("workspace close", budget: 100, count: 200, prepare: { _ in
            model.run("workspace.switch", args: home)
            model.run("workspace.new")
        }) { _ in
            model.run("workspace.close")
        })
        stats.append(await measure("workspace duplicate (12 tabs)", budget: 100, count: 200, prepare: { i in
            if i > 0 { model.run("workspace.close") }
            model.run("workspace.switch", args: home)
        }) { _ in
            model.run("workspace.duplicate")
        })
        model.run("workspace.close")
        stats.append(await measure("workspace switch (10 × 12 with the store's annotations)", budget: 100, count: 200) { i in
            model.run("workspace.switch", args: ShellCommandArgs(workspaceId: ids[(i + 1) % ids.count]))
        })
        model.run("workspace.switch", args: ShellCommandArgs(workspaceId: ids[0]))
        Self.flush()
        // A tab drag into the next column and its undo.
        stats.append(await measure("tab drag into another column (tab.move), drawn", budget: 16, count: 200, prepare: { i in
            if i > 0 { model.run("edit.undo") }
        }) { _ in
            let ws = model.activeWorkspace
            if let tab = ws.columns[0].tabs.last { model.moveTab(tab.id, to: ShellModel.TabDrop(column: 2, index: 0)) }
        })
        model.run("edit.undo")
        stats.append(await measure("column resize (column.resize), drawn", budget: 16, count: 200) { i in
            model.commitResize(column: 1, share: i % 2 == 0 ? 0.4 : 0.3)
        })
        let tabs = model.session.workspaces.reduce(0) { $0 + $1.columns.reduce(0) { $0 + $1.tabs.count } }
        let size = Self.windowSize
        return Report(host: "mac", window: "\(Int(size.width))x\(Int(size.height))", workspaces: model.session.workspaces.count, tabs: tabs, stats: stats)
    }
}
#endif
