// B039: the ink margin beside B005's reader. Left: the TextKit 2 reading surface (pane p1,
// BSB John 3:16–17 from the conformance fixture); right: the margin sheet with the ink canvas
// (or the PencilKit probe on iPad), the tool palette and a readout. Launch arguments:
//   -OPENLOGOS_VIEW ink              this view
//   -OPENLOGOS_INK_DEMO 1            fixture strokes on the sheet at launch (pictures)
//   -OPENLOGOS_INK_MEASURE 1         the in-process synthesized latency run at launch
//   -OPENLOGOS_INK_PENCILKIT 1       show the PencilKit probe instead of the canvas (iPad)
//   -OPENLOGOS_INK_PKDEMO 1          put a programmatic PKDrawing on the probe and export it
//   -OPENLOGOS_LOG 1                 append reports and exports to Documents/b039-ink.jsonl
import SwiftUI

struct InkMarginView: View {
    @ObservedObject var workspace: Workspace
    let state: PrototypeState
    let width: CGFloat
    @StateObject private var sheet = InkSheet()
    @State private var engine: InkCanvasEngine?
    @State private var showPencilKit = UserDefaults.standard.bool(forKey: "OPENLOGOS_INK_PENCILKIT")
    @State private var pencilKitExport: String?
    @State private var canvasSize: CGSize = .zero

    private var compact: Bool { WidthClass(width: width) == .compact }

    var body: some View {
        let layout = compact ? AnyLayout(VStackLayout(spacing: 0)) : AnyLayout(HStackLayout(spacing: 0))
        layout {
            if let pane = workspace.panes.first {
                VStack(spacing: 0) {
                    PaneIdentityRow(workspace: workspace, pane: pane, layout: .a, isActive: true, paneWidth: compact ? width : width * 0.45)
                    PaneTextView(workspace: workspace, pane: pane, state: state, width: width)
                }
                .frame(maxWidth: compact ? .infinity : width * 0.45, maxHeight: compact ? width * 0.5 : .infinity)
                .accessibilityElement(children: .contain)
                .accessibilityLabel("Pane \(pane.content.shortName)")
                Divider()
            }
            margin
        }
        .background(.background)
        .task { await driveFromLaunchArguments() }
    }

    private var margin: some View {
        VStack(spacing: 0) {
            identity
            palette
            ZStack(alignment: .topLeading) {
                ruledBackground
                #if canImport(UIKit)
                if showPencilKit {
                    PencilKitProbe(sheet: sheet, export: $pencilKitExport)
                } else {
                    InkCanvas(sheet: sheet) { engine = $0 }
                }
                #else
                InkCanvas(sheet: sheet) { engine = $0 }
                #endif
            }
            .background(GeometryReader { g in Color.clear.onAppear { canvasSize = g.size }.onChange(of: g.size) { _, s in canvasSize = s } })
            .clipped()
            .accessibilityElement(children: .contain)
            readoutView
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var identity: some View {
        HStack(spacing: 10) {
            Text("Margin").font(.footnote.weight(.semibold))
            Text("sheet over snapshot fixture:snapshot:1").font(.footnote).foregroundStyle(.secondary).lineLimit(1)
            Spacer(minLength: 0)
            Text("\(sheet.liveStrokes.count) strokes · \(sheet.document.deletedStrokeIds.count) erased · \(InkBrush.engineVersion)")
                .font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                .accessibilityIdentifier("ink-meta")
        }
        .padding(.horizontal, 12)
        .frame(height: 44)
        .overlay(alignment: .bottom) { Divider() }
    }

    private var palette: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                Picker("Tool", selection: $sheet.tool) {
                    ForEach(InkToolChoice.allCases) { Text($0.title).tag($0) }
                }
                .pickerStyle(.segmented)
                .frame(width: 250)
                .accessibilityIdentifier("ink-tool")
                Picker("Input", selection: $sheet.mode) {
                    ForEach(InkInputMode.allCases) { Text($0.rawValue.capitalized).tag($0) }
                }
                .pickerStyle(.segmented)
                .frame(width: 130)
                .accessibilityIdentifier("ink-mode")
                Button("Undo", systemImage: "arrow.uturn.backward") { sheet.undo(); engine?.syncLayers() }
                    .disabled(!sheet.canUndo).accessibilityIdentifier("ink-undo")
                Button("Redo", systemImage: "arrow.uturn.forward") { sheet.redo(); engine?.syncLayers() }
                    .disabled(!sheet.canRedo).accessibilityIdentifier("ink-redo")
                ColorPicker("Colour", selection: $sheet.color, supportsOpacity: false).labelsHidden().frame(width: 36)
                Slider(value: $sheet.width, in: 1...24, step: 1) { Text("Width") }.frame(width: 80)
                Button("Export", systemImage: "square.and.arrow.up") { export() }.accessibilityIdentifier("ink-export")
                Button("Measure", systemImage: "timer") { Task { await measure() } }.accessibilityIdentifier("ink-measure")
                #if canImport(UIKit)
                Toggle("PencilKit probe", isOn: $showPencilKit).toggleStyle(.button).accessibilityIdentifier("ink-pencilkit")
                #endif
            }
            .font(.footnote)
            .labelStyle(.iconOnly)
            .buttonStyle(.bordered)
            .controlSize(.small)
            .padding(.horizontal, 12)
            .frame(height: 44)
        }
        .overlay(alignment: .bottom) { Divider() }
    }

    private var ruledBackground: some View {
        GeometryReader { g in
            Path { p in
                var y: CGFloat = 30
                while y < g.size.height { p.move(to: CGPoint(x: 0, y: y)); p.addLine(to: CGPoint(x: g.size.width, y: y)); y += 30 }
            }
            .stroke(.secondary.opacity(0.18), lineWidth: 1)
        }
        .allowsHitTesting(false)
    }

    private var readoutView: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 4) {
                Text(sheet.readout)
                if let latency = sheet.latency { Text(latency.summary) }
                if let pencilKitExport { Text(pencilKitExport) }
            }
            .font(.system(size: 10, design: .monospaced))
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .textSelection(.enabled)
        }
        .frame(height: 112)
        .overlay(alignment: .top) { Divider() }
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("ink-readout")
        .accessibilityLabel("Ink readout")
        .accessibilityValue(sheet.readout + (sheet.latency.map { "\n" + $0.summary } ?? "") + (pencilKitExport.map { "\n" + $0 } ?? ""))
    }

    private func export() {
        switch sheet.exportJSON() {
        case .success(let json):
            sheet.readout = json.count > 1600 ? String(json.prefix(1600)) + "\n… (\(json.count) bytes, valid)" : json
            log(ExportLog(kind: "export", bytes: json.count, strokes: sheet.document.strokes.count))
        case .failure(let error):
            sheet.readout = "inkDocumentJson rejected the document: \(error)"
        }
    }

    private func measure() async {
        guard let engine else { return }
        let report = await engine.synthesize(label: "synthesized pen (in-process)")
        sheet.latency = report
        log(report)
    }

    private func driveFromLaunchArguments() async {
        let defaults = UserDefaults.standard
        try? await Task.sleep(for: .milliseconds(600))
        if defaults.bool(forKey: "OPENLOGOS_INK_DEMO") {
            sheet.addDemoStrokes(in: canvasSize == .zero ? CGSize(width: 500, height: 600) : canvasSize)
            engine?.syncLayers()
        }
        if defaults.bool(forKey: "OPENLOGOS_INK_MEASURE") {
            await measure()
            let touch = await engine?.synthesize(points: 60, contact: .finger, label: "synthesized finger (in-process, finger mode)")
            if let touch { log(touch); sheet.readout += "\n" + touch.summary }
            engine?.syncLayers()
        }
    }

    private func log(_ value: Encodable) {
        guard UserDefaults.standard.bool(forKey: "OPENLOGOS_LOG") else { return }
        let encoder = JSONEncoder()
        guard let data = try? encoder.encode(AnyEncodable(value)), let line = String(data: data, encoding: .utf8) else { return }
        let directory = UserDefaults.standard.string(forKey: "OPENLOGOS_LOG_DIR").map { URL(fileURLWithPath: $0) }
            ?? FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        let url = directory.appendingPathComponent("b039-ink.jsonl")
        if let handle = try? FileHandle(forWritingTo: url) {
            handle.seekToEndOfFile(); handle.write((line + "\n").data(using: .utf8)!); try? handle.close()
        } else {
            try? (line + "\n").write(to: url, atomically: true, encoding: .utf8)
        }
    }
}

private struct AnyEncodable: Encodable {
    let value: Encodable
    init(_ value: Encodable) { self.value = value }
    func encode(to encoder: Encoder) throws { try value.encode(to: encoder) }
}

private struct ExportLog: Encodable { let kind: String; let bytes: Int; let strokes: Int }
