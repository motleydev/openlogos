// The transient surfaces: the Study drawer with its Context → Cross references → Word → Sources
// progression, the collection drawer, and the notes dock (specs/21 §3–§5). In regular widths
// they are a trailing column over the last pane; in compact width WorkspaceView presents the
// same views as sheets.
import SwiftUI

/// The header every drawer shares: what it is, what it covers, pin and close.
struct DrawerHeader: View {
    @ObservedObject var workspace: Workspace
    let title: String
    let subtitle: String?
    var covering: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 4) {
                VStack(alignment: .leading, spacing: 1) {
                    Text(title).font(.headline).accessibilityAddTraits(.isHeader)
                    if let subtitle { Text(subtitle).font(.footnote).foregroundStyle(.secondary).lineLimit(2) }
                }
                Spacer(minLength: 4)
                ChromeButton(title: workspace.drawerPinned ? "Unpin" : "Pin as pane", symbol: workspace.drawerPinned ? "pin.fill" : "pin", isOn: workspace.drawerPinned) {
                    workspace.drawerPinned.toggle()
                }
                ChromeButton(title: "Close", symbol: "xmark") { workspace.toggle(workspace.drawer) }
                    .keyboardShortcut(.escape, modifiers: [])
            }
            if let covering, !workspace.drawerPinned {
                Text("Covering \(covering); pin to keep both.").font(.caption2).foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(.bar)
        .overlay(alignment: .bottom) { Divider() }
    }
}

/// A labelled provenance badge: Scripture, source data, attributed interpretation, personal.
struct KindBadge: View {
    let text: String
    var body: some View {
        Text(text)
            .font(.caption2)
            .padding(.horizontal, 6).padding(.vertical, 2)
            .background(Capsule().strokeBorder(.secondary.opacity(0.6), lineWidth: 1))
            .foregroundStyle(.secondary)
    }
}

// MARK: Study

struct StudyDrawer: View {
    @ObservedObject var workspace: Workspace
    let state: PrototypeState
    var covering: String?
    @State private var expanded: Set<String> = ["context", "cross", "word", "sources"]
    @State private var showCode = false
    @State private var preview: CrossReferenceFixture?

    private var selected: String {
        guard let pane = workspace.panes.first(where: { $0.id == "p1" }), let verse = pane.content.verses.first,
              let range = workspace.selectedWordRange else { return PrototypeFixtures.selectedWords }
        let list = words(of: verse.text)
        return list[range.clamped(to: 0...(list.count - 1))].joined(separator: " ")
    }

    var body: some View {
        VStack(spacing: 0) {
            DrawerHeader(workspace: workspace, title: "Study", subtitle: "“\(selected)” · John 3:16 · Berean Standard Bible", covering: covering)
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    section("context", "Context") {
                        KindBadge(text: "Scripture")
                        Text("For God so loved the world that He gave His ").font(.system(.body, design: .serif))
                            + Text(selected).font(.system(.body, design: .serif)).underline()
                            + Text(" Son, that everyone who believes in Him shall not perish but have eternal life.").font(.system(.body, design: .serif))
                        Text("Paragraph: John 3:16–21. Footnote (BSB): none for this span (fixture).").font(.footnote).foregroundStyle(.secondary)
                    }
                    section("cross", "Cross references") {
                        ForEach(PrototypeFixtures.crossReferences) { reference in
                            Button { preview = reference } label: {
                                HStack(alignment: .firstTextBaseline, spacing: 8) {
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(reference.target).font(.subheadline).fontWeight(.medium)
                                        Text("\(reference.kind) · \(reference.source)").font(.caption2).foregroundStyle(.secondary)
                                        if !reference.installed {
                                            Text("Target edition not installed: metadata only").font(.caption2).foregroundStyle(.secondary)
                                        }
                                    }
                                    Spacer()
                                    Image(systemName: "chevron.right").imageScale(.small).foregroundStyle(.secondary)
                                }
                                .frame(minHeight: ChromeMetrics.minimumTarget)
                                .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                            .accessibilityHint("Opens a passage preview; the reading pane stays where it is")
                        }
                        KindBadge(text: "source data: fixture list, coverage not declared")
                    }
                    section("word", "Word") {
                        if state == .unavailable {
                            StateNotice(symbol: "questionmark.circle", text: "No alignment data for this span in this edition. Nothing is inferred from a Strong's number.")
                        } else {
                            KindBadge(text: "source data: fixture alignment")
                            HStack(alignment: .firstTextBaseline, spacing: 10) {
                                Text(PrototypeFixtures.alignedToken).font(.system(.title2, design: .serif))
                                Text(PrototypeFixtures.lemma).font(.system(.body, design: .serif)).foregroundStyle(.secondary)
                                Text(PrototypeFixtures.transliteration).font(.footnote).italic().foregroundStyle(.secondary)
                            }
                            Text("Aligns to 1 token in N1904 (fixture). " + PrototypeFixtures.morphologySummary).font(.footnote)
                            DisclosureGroup(isExpanded: $showCode) {
                                Text("Morphology code \(PrototypeFixtures.morphologyCode). Strong's: optional reference field, not shown as identity.")
                                    .font(.caption).foregroundStyle(.secondary)
                            } label: {
                                Text("Technical code").font(.caption)
                            }
                            .frame(minHeight: ChromeMetrics.minimumTarget)
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Renderings in BSB, John (fixture scope); unit: tokens; coverage: declared complete for this book").font(.caption2).foregroundStyle(.secondary)
                                ForEach(PrototypeFixtures.renderings, id: \.0) { rendering, count in
                                    HStack {
                                        Text(rendering).font(.footnote)
                                        Spacer()
                                        Text("\(count)").font(.footnote).monospacedDigit()
                                    }
                                }
                            }
                            Text("Counts are token counts in the declared scope. They do not imply meaning or importance.").font(.caption2).foregroundStyle(.secondary)
                        }
                    }
                    section("sources", "Sources") {
                        if state == .offline {
                            StateNotice(symbol: "wifi.slash", text: "Offline. Installed sources are unaffected; only the catalog's uninstalled entries cannot be fetched.")
                        }
                        HStack(alignment: .firstTextBaseline) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(PrototypeFixtures.calvinJohnMissing.fullTitle).font(.footnote)
                                Text("Not installed · catalog metadata only").font(.caption2).foregroundStyle(.secondary)
                            }
                            Spacer()
                            Button("Install") {}.font(.footnote).buttonStyle(.bordered).disabled(state == .offline)
                        }
                        KindBadge(text: "attributed interpretation, when installed")
                    }
                    Text("Fixture data for the interaction; no dataset is claimed.").font(.caption2).foregroundStyle(.secondary)
                }
                .padding(12)
            }
        }
        .background(.background)
        .sheet(item: $preview) { reference in
            CrossReferencePreview(workspace: workspace, reference: reference)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Study drawer")
    }

    @ViewBuilder private func section<Content: View>(_ id: String, _ title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Button {
                if expanded.contains(id) { expanded.remove(id) } else { expanded.insert(id) }
            } label: {
                HStack {
                    Text(title).font(.subheadline).fontWeight(.semibold)
                    Spacer()
                    Image(systemName: expanded.contains(id) ? "chevron.down" : "chevron.right").imageScale(.small)
                }
                .frame(minHeight: ChromeMetrics.minimumTarget)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(.isHeader)
            if expanded.contains(id) {
                content()
            }
        }
    }
}

/// A cross reference opens here, with its own navigation; the reading pane does not move.
struct CrossReferencePreview: View {
    @ObservedObject var workspace: Workspace
    let reference: CrossReferenceFixture
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Text(reference.target).font(.headline)
                Spacer()
                Button("Close") { dismiss() }.frame(minHeight: ChromeMetrics.minimumTarget)
            }
            Text("\(reference.kind) · source: \(reference.source)").font(.caption).foregroundStyle(.secondary)
            if reference.installed {
                Text(reference.preview).font(.system(.title3, design: .serif))
                Text("Berean Standard Bible (fixture)").font(.caption).foregroundStyle(.secondary)
            } else {
                UnavailableSourceView(content: PaneContent(id: "x", shortName: "", fullTitle: "Target edition for \(reference.target)", license: "", passage: reference.target, verses: [], isRightToLeft: false, kind: .scripture, isInstalled: false, isReferenceOnly: false))
            }
            HStack(spacing: 8) {
                Button("Previous verse") {}.buttonStyle(.bordered)
                Button("Next verse") {}.buttonStyle(.bordered)
                Spacer()
                Button("Open in pane") {}.buttonStyle(.bordered)
                Button("Collect") {
                    workspace.collect(text: reference.preview, resource: PrototypeFixtures.bsbJohn3, locator: reference.target)
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
                .disabled(!reference.installed)
            }
            .frame(minHeight: ChromeMetrics.minimumTarget)
            Spacer()
        }
        .padding(20)
        .presentationDetents([.medium])
    }
}

// MARK: Collection

struct CollectionDrawer: View {
    @ObservedObject var workspace: Workspace
    @EnvironmentObject var options: PrototypeOptions
    let state: PrototypeState
    var covering: String?

    var body: some View {
        VStack(spacing: 0) {
            DrawerHeader(workspace: workspace, title: workspace.collectionName, subtitle: "\(workspace.collectionItems.count) items · fixture collection", covering: covering)
            if workspace.collectionItems.isEmpty {
                VStack(alignment: .leading, spacing: 10) {
                    Text("Nothing collected yet.").font(.headline)
                    Text("Select text and choose Collect. The indicator shows 0 until then.").font(.footnote).foregroundStyle(.secondary)
                }
                .padding(16)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            } else {
                List {
                    Section("Renderings") {
                        ForEach(workspace.collectionItems.prefix(2)) { item in CollectionRow(item: item) }
                    }
                    Section("Observations and references") {
                        ForEach(workspace.collectionItems.dropFirst(2)) { item in CollectionRow(item: item) }
                    }
                }
                .listStyle(.plain)
            }
            HStack(spacing: 8) {
                Button {
                    options.view = .handoff
                } label: {
                    Label("Hand off to Obsidian", systemImage: "square.and.arrow.up")
                        .frame(minHeight: ChromeMetrics.minimumTarget)
                }
                .buttonStyle(.bordered)
                .disabled(workspace.collectionItems.isEmpty)
                Spacer()
                Button("Reorder") {}.font(.footnote).frame(minHeight: ChromeMetrics.minimumTarget)
            }
            .padding(.horizontal, 12)
            .background(.bar)
            .overlay(alignment: .top) { Divider() }
        }
        .background(.background)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Collection drawer")
    }
}

struct CollectionRow: View {
    let item: CollectionItem
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                Image(systemName: symbol).imageScale(.small).foregroundStyle(.secondary)
                Text(item.resource).font(.subheadline).fontWeight(.medium)
                Text(item.locator).font(.subheadline).foregroundStyle(.secondary)
                Spacer()
                if item.rights == .metadataOnly { KindBadge(text: "reference only") }
            }
            Text(item.text)
                .font(item.kind == .observation ? .system(.footnote) : .system(.footnote, design: .serif))
                .foregroundStyle(item.rights == .metadataOnly ? .secondary : .primary)
                .lineLimit(3)
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(item.kind.rawValue), \(item.resource) \(item.locator), \(item.text)")
    }

    private var symbol: String {
        switch item.kind {
        case .passage: return "book"
        case .excerpt: return "text.quote"
        case .observation: return "text.bubble"
        case .crossReference: return "arrow.triangle.branch"
        case .marginSheet: return "pencil.and.outline"
        }
    }
}

// MARK: Notes dock

struct NotesDock: View {
    @ObservedObject var workspace: Workspace
    let state: PrototypeState
    var covering: String?
    @State private var follows = false
    @State private var body_ = "Compare \"one and only\" (BSB) with \"only begotten\" (KJV) and μονογενῆ before teaching John 3:16."

    var body: some View {
        VStack(spacing: 0) {
            DrawerHeader(workspace: workspace, title: "Notes", subtitle: status, covering: covering)
            Toggle("Follow active passage", isOn: $follows).font(.footnote).padding(.horizontal, 12).frame(minHeight: ChromeMetrics.minimumTarget)
            if state == .conflict {
                ConflictView()
            } else {
                HStack(spacing: 4) {
                    KindBadge(text: "John 3:16 · BSB")
                    KindBadge(text: "John 3:16 · KJV")
                    Spacer()
                }
                .padding(.horizontal, 12)
                TextEditor(text: $body_)
                    .font(.system(.body))
                    .padding(8)
                    .accessibilityLabel("Note body")
                Text("Markdown · formatting controls show while editing · app-managed note (fixture)")
                    .font(.caption2).foregroundStyle(.secondary).padding(12)
            }
        }
        .background(.background)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Notes dock")
    }

    private var status: String {
        switch state {
        case .offline: return "Offline, saved locally"
        case .conflict: return "Conflict"
        case .loading: return "Saving…"
        default: return "Saved"
        }
    }
}

/// Both bodies, timestamps, and a choice; nothing is overwritten until chosen (specs/21 §11).
struct ConflictView: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("Edited in two places", systemImage: "exclamationmark.triangle").font(.headline)
            Text("This note's vault file changed after the app's last save. Both versions are kept until you choose.").font(.footnote).foregroundStyle(.secondary)
            Group {
                conflictBody("Mine · app, 12:04", "Compare \"one and only\" (BSB) with \"only begotten\" (KJV) and μονογενῆ before teaching John 3:16.")
                conflictBody("Theirs · vault, 12:06", "Compare \"one and only\" (BSB) with \"only begotten\" (KJV). See also John 1:14, 18.")
            }
            HStack(spacing: 8) {
                Button("Keep mine") {}.buttonStyle(.bordered)
                Button("Keep theirs") {}.buttonStyle(.bordered)
                Button("Keep both") {}.buttonStyle(.borderedProminent)
            }
            .frame(minHeight: ChromeMetrics.minimumTarget)
            Spacer()
        }
        .padding(12)
    }

    private func conflictBody(_ title: String, _ text: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(.caption).foregroundStyle(.secondary)
            Text(text).font(.footnote).padding(8).frame(maxWidth: .infinity, alignment: .leading)
                .background(RoundedRectangle(cornerRadius: 6).fill(.quaternary.opacity(0.5)))
        }
    }
}
