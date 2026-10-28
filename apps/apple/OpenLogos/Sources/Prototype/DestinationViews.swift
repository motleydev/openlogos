// The three destination views that replace the pane region: Search results, Margin capture
// (snapshot preview only; ink is P2) and Obsidian handoff (specs/21 §6, §11, §12; specs/17).
import SwiftUI

// MARK: Search results

struct SearchResultsView: View {
    @ObservedObject var workspace: Workspace
    let state: PrototypeState
    let width: CGFloat
    /// Layout A's shared bar already holds the one field for passages and words (specs/16
    /// "One search field"); only layout B, which has no bar, puts the field here.
    var showsField = false
    @State private var scope = "BSB · Protestant canon · verse unit"

    private var invalid: Bool { state == .invalidQuery }
    private var empty: Bool { state == .empty }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 8) {
                if showsField {
                    PassageField(text: $workspace.passageFieldText, compact: true)
                } else {
                    Text("Results for “\(workspace.passageFieldText)” (the field above is the query)")
                        .font(.subheadline).accessibilityAddTraits(.isHeader)
                }
                HStack(spacing: 8) {
                    KindBadge(text: scope)
                    KindBadge(text: "exact phrase")
                    Spacer()
                    Menu("Filters") {
                        Button("Book or range…") {}
                        Button("Edition…") {}
                        Button("Search unit: verse") {}
                        Button("Expert syntax…") {}
                    }
                    .font(.footnote)
                    .frame(minHeight: ChromeMetrics.minimumTarget)
                }
                if invalid {
                    VStack(alignment: .leading, spacing: 4) {
                        Label("Unbalanced quotation mark at position 13.", systemImage: "exclamationmark.circle").font(.footnote)
                        Text("Try: \"one and only\" AND Son. The previous results stay below; nothing was sent to a model.").font(.caption).foregroundStyle(.secondary)
                    }
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(RoundedRectangle(cornerRadius: 8).strokeBorder(.secondary, lineWidth: 1))
                } else if empty {
                    Text("No verses in BSB contain “\(workspace.passageFieldText)”. Scope: Protestant canon, verse unit. Widen explicitly: all installed editions · ignore diacritics.")
                        .font(.footnote).foregroundStyle(.secondary)
                } else {
                    Text("\(PrototypeFixtures.searchHits.count) verses · \(PrototypeFixtures.searchHits.count) occurrences · index complete for BSB (fixture)")
                        .font(.footnote).foregroundStyle(.secondary)
                }
            }
            .padding(ChromeMetrics.panePadding(width))
            .background(.bar)
            .overlay(alignment: .bottom) { Divider() }

            if !empty {
                List(PrototypeFixtures.searchHits) { hit in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack(spacing: 6) {
                            Text(hit.reference).font(.subheadline).fontWeight(.medium)
                            Text(hit.edition).font(.caption).foregroundStyle(.secondary)
                            Spacer()
                            Button("Collect") { workspace.collect(text: hit.before + hit.match + hit.after, resource: PrototypeFixtures.bsbJohn3, locator: hit.reference) }
                                .font(.footnote).frame(minHeight: ChromeMetrics.minimumTarget)
                            Button("Open in pane") {}.font(.footnote).frame(minHeight: ChromeMetrics.minimumTarget)
                        }
                        (Text("…" + hit.before) + Text(hit.match).bold().underline() + Text(hit.after + "…"))
                            .font(.system(.body, design: .serif))
                    }
                    .padding(.vertical, 4)
                    .accessibilityElement(children: .combine)
                }
                .listStyle(.plain)
            } else {
                Spacer()
            }
            Text("Deterministic search over installed text. “Ask about these results” is a separate, labelled action and is not shown here.")
                .font(.caption2).foregroundStyle(.secondary).padding(ChromeMetrics.panePadding(width))
        }
        .background(.background)
    }
}

// MARK: Margin capture

struct MarginCaptureView: View {
    @ObservedObject var workspace: Workspace
    let state: PrototypeState
    let width: CGFloat
    @State private var includeNotes = false
    @State private var selectedPanes: Set<String> = ["p1", "p2"]

    var body: some View {
        let compact = WidthClass(width: width) == .compact
        let layout = compact ? AnyLayout(VStackLayout(alignment: .leading, spacing: 16)) : AnyLayout(HStackLayout(alignment: .top, spacing: 20))
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Capture a margin sheet").font(.title2).fontWeight(.semibold).accessibilityAddTraits(.isHeader)
                Text("The sheet freezes the visible reading content of the panes you choose, without chrome, dock, chat or private notes unless checked. Ink tools are P2; this is the capture preview.")
                    .font(.footnote).foregroundStyle(.secondary)
                layout {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("Panes").font(.subheadline).fontWeight(.semibold)
                        ForEach(workspace.panes) { pane in
                            Toggle(isOn: Binding(get: { selectedPanes.contains(pane.id) }, set: { on in if on { selectedPanes.insert(pane.id) } else { selectedPanes.remove(pane.id) } })) {
                                VStack(alignment: .leading, spacing: 1) {
                                    Text("\(pane.content.shortName) · \(pane.content.passage)").font(.footnote)
                                    if pane.content.isReferenceOnly { Text("reference only: captured as a hatched box").font(.caption2).foregroundStyle(.secondary) }
                                    if !pane.content.isInstalled { Text("not installed: cannot be captured").font(.caption2).foregroundStyle(.secondary) }
                                }
                            }
                            .disabled(!pane.content.isInstalled)
                            .frame(minHeight: ChromeMetrics.minimumTarget)
                        }
                        Toggle("Include private notes", isOn: $includeNotes).font(.footnote).frame(minHeight: ChromeMetrics.minimumTarget)
                        Text("Extent: visible content (John 3:16–17). Never a whole document silently.").font(.caption2).foregroundStyle(.secondary)
                        Text("Metadata: editions, revisions, references, pane rectangles, workspace recipe.").font(.caption2).foregroundStyle(.secondary)
                        HStack(spacing: 8) {
                            Button("Capture") {}.buttonStyle(.borderedProminent).disabled(selectedPanes.isEmpty || state == .readOnly)
                            Button("Return to reading") {}.buttonStyle(.bordered)
                        }
                        .frame(minHeight: ChromeMetrics.minimumTarget)
                        if state == .readOnly {
                            StateNotice(symbol: "lock", text: "Read only: every selected pane is reference-only, so there is no text to freeze. Choose a pane with quotation rights.")
                        }
                    }
                    .frame(maxWidth: compact ? .infinity : 320)
                    sheetPreview
                }
                Text("Fixture preview; no snapshot is written.").font(.caption2).foregroundStyle(.secondary)
            }
            .padding(ChromeMetrics.panePadding(width))
        }
        .background(.background)
    }

    private var sheetPreview: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(workspace.panes.filter { selectedPanes.contains($0.id) && $0.content.isInstalled }) { pane in
                VStack(alignment: .leading, spacing: 6) {
                    Text("\(pane.content.fullTitle) · \(pane.content.passage)").font(.caption2).foregroundStyle(.secondary)
                    if pane.content.isReferenceOnly {
                        Rectangle().fill(.quaternary).frame(height: 60).overlay(Text("reference only").font(.caption2))
                    } else {
                        Text(pane.content.verses.map { "\($0.number) \($0.text)" }.joined(separator: " "))
                            .font(.system(.footnote, design: .serif))
                            .environment(\.layoutDirection, pane.content.isRightToLeft ? .rightToLeft : .leftToRight)
                            .frame(maxWidth: .infinity, alignment: pane.content.isRightToLeft ? .trailing : .leading)
                    }
                }
                .padding(12)
                Divider()
            }
            Text("added margin").font(.caption2).foregroundStyle(.tertiary).frame(maxWidth: .infinity, minHeight: 80)
        }
        .background(RoundedRectangle(cornerRadius: 6).fill(.background).shadow(radius: 6))
        .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(.secondary.opacity(0.3)))
        .frame(maxWidth: 520)
        .accessibilityLabel("Sheet preview")
    }
}

// MARK: Obsidian handoff

struct HandoffView: View {
    @ObservedObject var workspace: Workspace
    let state: PrototypeState
    let width: CGFloat
    @State private var step = 0
    @State private var includeObservations = true

    private var destination: String {
        switch state {
        case .offline, .unavailable: return "Export file (no vault reachable on this device): OpenLogos-John-3-study.zip"
        default: return "Vault “Sermons” · folder OpenLogos/ (app-owned, chosen by you)"
        }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Hand off to Obsidian").font(.title2).fontWeight(.semibold).accessibilityAddTraits(.isHeader)
                Picker("Step", selection: $step) {
                    Text("Preview").tag(0)
                    Text("Result").tag(1)
                    Text("Re-export").tag(2)
                }
                .pickerStyle(.segmented)
                .frame(maxWidth: 420)
                if step == 0 { preview } else if step == 1 { result } else { reexport }
                Text("Nothing here writes into your manuscript note. Observations stay local; no model is called. Fixture data.").font(.caption2).foregroundStyle(.secondary)
            }
            .padding(ChromeMetrics.panePadding(width))
            .frame(maxWidth: 760, alignment: .leading)
        }
        .background(.background)
    }

    private var preview: some View {
        VStack(alignment: .leading, spacing: 12) {
            row("Destination", destination)
            if state == .unavailable {
                StateNotice(symbol: "externaldrive.badge.xmark", text: "The vault folder you chose is not available (unmounted or permission revoked). Export file is offered instead; nothing is dropped.")
            }
            row("Collection", "\(workspace.collectionName) · \(workspace.collectionItems.count) items · stable IDs ol-c1 … ol-c\(workspace.collectionItems.count)")
            Toggle("Include personal observations", isOn: $includeObservations).font(.footnote).frame(minHeight: ChromeMetrics.minimumTarget)
            VStack(alignment: .leading, spacing: 6) {
                ForEach(workspace.collectionItems) { item in
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Image(systemName: item.rights == .quotable ? "checkmark.circle" : "minus.circle").foregroundStyle(.secondary)
                        VStack(alignment: .leading, spacing: 2) {
                            Text("\(item.resource) \(item.locator)").font(.footnote).fontWeight(.medium)
                            if item.rights == .quotable {
                                Text(item.kind == .observation && !includeObservations ? "excluded by your choice" : item.text).font(.system(.caption, design: .serif)).foregroundStyle(.secondary).lineLimit(2)
                            } else {
                                Text("reference only · reason: quotation not permitted in this edition").font(.caption).foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }
            row("Scripture blocks", "John-003 note for BSB (rev. 2024) with ^v-016, ^v-017; links pinned to edition and revision")
            row("Attachments", "none selected")
            HStack(spacing: 8) {
                Button(state == .offline || state == .unavailable ? "Export file" : "Export to vault") { step = 1 }.buttonStyle(.borderedProminent)
                Button("Cancel") {}.buttonStyle(.bordered)
            }
            .frame(minHeight: ChromeMetrics.minimumTarget)
        }
    }

    private var result: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("Written", systemImage: "checkmark.circle").font(.headline)
            row("Index", "OpenLogos/Collections/John 3 study.md (4 blocks, each with a return-to-source link)")
            row("Scripture", "OpenLogos/Scripture/BSB-2024/John-003.md (materialized verses 16–17)")
            row("Not written", "1 John 4:9 from the borrowed edition: reference metadata only")
            HStack(spacing: 8) {
                Button("Insert links into current note") {}.buttonStyle(.bordered).disabled(state == .offline || state == .unavailable)
                Button("Open in Obsidian") {}.buttonStyle(.bordered).disabled(state == .offline || state == .unavailable)
            }
            .frame(minHeight: ChromeMetrics.minimumTarget)
            Text("Insert adds links or embeds at the cursor of the note you have open in Obsidian. It never replaces that note's body.").font(.caption).foregroundStyle(.secondary)
        }
    }

    private var reexport: some View {
        VStack(alignment: .leading, spacing: 12) {
            if state == .conflict {
                Label("Concurrent edit", systemImage: "exclamationmark.triangle").font(.headline)
                Text("Block ol-c3 (your observation) was edited in the vault after the last export. Both versions are kept; choose which the managed block carries. Unmanaged text around it is untouched.").font(.footnote)
                HStack(spacing: 8) {
                    Button("Keep app version") {}.buttonStyle(.bordered)
                    Button("Keep vault version") {}.buttonStyle(.bordered)
                    Button("Keep both") {}.buttonStyle(.borderedProminent)
                }
                .frame(minHeight: ChromeMetrics.minimumTarget)
            } else {
                Label("Reconciled by ID and last-export hash", systemImage: "arrow.triangle.2.circlepath").font(.headline)
                row("Unchanged", "ol-c1, ol-c2, ol-c4")
                row("Changed source", "ol-c1: BSB revision 2025 differs in verse 16. A new resource revision is offered; the existing embed keeps pointing at rev. 2024.")
                row("Renamed in vault", "John 3 study.md → John 3 (series).md: links updated by ID; no embed retargeted")
                HStack(spacing: 8) {
                    Button("Re-export") {}.buttonStyle(.borderedProminent)
                    Button("Offer new revision") {}.buttonStyle(.bordered)
                }
                .frame(minHeight: ChromeMetrics.minimumTarget)
            }
        }
    }

    private func row(_ title: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).font(.caption).foregroundStyle(.secondary)
            Text(value).font(.footnote)
        }
        .accessibilityElement(children: .combine)
    }
}
