// The workspace the prototype renders: ordered panes with independent link modes, an active
// pane, a collection, and the transient surfaces (specs/21 §1–§5). State only; no persistence.
import SwiftUI

struct Pane: Identifiable {
    let id: String
    var content: PaneContent
    var link: LinkMode
}

enum Drawer: Equatable {
    case none, study, collection, notes
}

@MainActor
final class Workspace: ObservableObject {
    @Published var panes: [Pane]
    @Published var activePaneID: String
    @Published var drawer: Drawer = .none
    @Published var drawerPinned = false
    @Published var collectionItems: [CollectionItem]
    @Published var collectionName: String
    /// The index of the pane shown in compact width (others are tabs).
    @Published var compactIndex = 0
    /// A transient confirmation line ("Added to John 3 study (5)") with its undo.
    @Published var confirmation: Confirmation?
    @Published var passageFieldText: String
    /// The current text selection, as the core anchored it (B005); one pane at a time.
    @Published var selection: ReaderSelection?
    /// Pending commands for the reading surfaces, by pane id (and "note" for the notes dock).
    @Published var commands: [String: ReaderCommand] = [:]
    /// The B005 measurement summary, shown under the first pane once it has run.
    @Published var measurement: String?
    /// A note the notes dock shows in a reading surface (the B005 bidi study note).
    @Published var notePane: PaneContent?

    struct Confirmation: Equatable {
        let message: String
        let detail: String?
    }

    init(panes: [Pane], active: String? = nil, collectionItems: [CollectionItem] = PrototypeFixtures.collectionItems,
         collectionName: String = PrototypeFixtures.collectionName, passage: String = "John 3:16") {
        self.panes = panes
        activePaneID = active ?? panes.first?.id ?? ""
        self.collectionItems = collectionItems
        self.collectionName = collectionName
        passageFieldText = passage
    }

    var activePane: Pane? { panes.first { $0.id == activePaneID } }

    func activate(_ id: String) {
        activePaneID = id
        if let index = panes.firstIndex(where: { $0.id == id }) { compactIndex = index }
    }

    func setLink(_ mode: LinkMode, for id: String) {
        guard let index = panes.firstIndex(where: { $0.id == id }) else { return }
        panes[index].link = mode
        confirmation = Confirmation(message: "\(panes[index].content.shortName) is now \(mode.label)", detail: nil)
    }

    /// Toggle link for the active pane: Pinned ↔ Group 1 (specs/21 §9).
    func toggleLink() {
        guard let pane = activePane else { return }
        setLink(pane.link == .pinned ? .group(1) : .pinned, for: pane.id)
    }

    func toggle(_ wanted: Drawer) {
        drawer = drawer == wanted ? .none : wanted
        if drawer == .none { drawerPinned = false }
    }

    /// Capture the current selection (or the active passage) into the collection (specs/21 §5).
    func collect(text: String, resource: PaneContent, locator: String) {
        if collectionItems.contains(where: { $0.resource == resource.shortName && $0.locator == locator }) {
            confirmation = Confirmation(message: "Already in \(collectionName)", detail: "Show · Add observation anyway")
            return
        }
        let item = CollectionItem(
            id: UUID().uuidString,
            kind: resource.isReferenceOnly ? .crossReference : .excerpt,
            resource: resource.shortName,
            locator: locator,
            text: resource.isReferenceOnly ? "reference only: quotation not permitted in this edition" : text,
            rights: resource.isReferenceOnly ? .metadataOnly : .quotable
        )
        collectionItems.append(item)
        let count = collectionItems.count
        confirmation = Confirmation(
            message: resource.isReferenceOnly
                ? "Added reference only to \(collectionName) (\(count))"
                : "Added to \(collectionName) (\(count))",
            detail: "Undo · Change collection · Add observation"
        )
    }

    /// A surface reports its selection; a selection in one pane clears another's.
    func setSelection(_ selection: ReaderSelection?, in paneID: String) {
        if let selection {
            self.selection = selection
        } else if self.selection?.paneID == paneID {
            self.selection = nil
        }
    }

    func send(_ kind: ReaderCommand.Kind, to paneID: String) {
        commands[paneID] = ReaderCommand.make(kind)
    }

    func undoLastCapture() {
        guard !collectionItems.isEmpty else { return }
        collectionItems.removeLast()
        confirmation = nil
    }
}

// MARK: Scenario construction

/// Words of a verse as the selection prototype counts them (whitespace-separated, punctuation
/// kept with its word) and the index range of a phrase within them.
func words(of text: String) -> [String] {
    text.split(whereSeparator: { $0.isWhitespace }).map(String.init)
}

func wordRange(of phrase: String, in text: String) -> ClosedRange<Int>? {
    let haystack = words(of: text).map { $0.trimmingCharacters(in: .punctuationCharacters).lowercased() }
    let needle = words(of: phrase).map { $0.lowercased() }
    guard !needle.isEmpty, haystack.count >= needle.count else { return nil }
    for start in 0...(haystack.count - needle.count) where Array(haystack[start..<(start + needle.count)]) == needle {
        return start...(start + needle.count - 1)
    }
    return nil
}

extension Workspace {
    /// The workspaces behind the seven views, in the fixture's happy path and each state.
    static func make(view: PrototypeView, state: PrototypeState) -> Workspace {
        let f = PrototypeFixtures.self
        switch view {
        case .shell:
            return Workspace(panes: [Pane(id: "p1", content: f.bsbJohn3, link: .pinned)])
        case .read:
            let content = state == .readOnly ? f.referenceOnlyJohn3 : f.bsbJohn3
            return Workspace(panes: [Pane(id: "p1", content: content, link: .pinned)])
        case .three, .collection, .margin, .handoff:
            var third = f.kjvJohn3
            if state == .unavailable { third = f.calvinJohnMissing }
            if state == .readOnly { third = f.referenceOnlyJohn3 }
            let ws = Workspace(panes: [
                Pane(id: "p1", content: f.bsbJohn3, link: .group(1)),
                Pane(id: "p2", content: f.greekJohn3, link: .group(1)),
                Pane(id: "p3", content: third, link: .pinned),
            ])
            if view == .collection {
                ws.drawer = .collection
                if state == .empty { ws.collectionItems = [] }
            }
            return ws
        case .selection:
            let ws = Workspace(panes: [
                Pane(id: "p1", content: f.bsbJohn3, link: .group(1)),
                Pane(id: "p2", content: f.greekJohn3, link: .group(1)),
                Pane(id: "p3", content: f.kjvJohn3, link: .pinned),
            ])
            ws.send(.selectPhrase(f.selectedWords), to: "p1")
            ws.drawer = state == .empty ? .none : .study
            return ws
        case .anchors:
            // B005: three texts the conformance fixtures and the N1904 pack cover (English,
            // pointed Hebrew right to left, Greek with diacritics) and the bidi study note in a
            // pinned notes dock; `-OPENLOGOS_CASE` and `-OPENLOGOS_PERF` drive the surfaces.
            let synthetic = UserDefaults.standard.bool(forKey: "OPENLOGOS_PERF")
            let count = max(10, UserDefaults.standard.integer(forKey: "OPENLOGOS_SYNTH"))
            let ws = Workspace(panes: [
                Pane(id: "p1", content: synthetic ? f.synthetic(verses: count == 10 ? 1000 : count) : f.bsbJohn3, link: .group(1)),
                Pane(id: "p2", content: f.wlcGenesis1, link: .group(1)),
                Pane(id: "p3", content: f.greekJohn3, link: .pinned),
            ])
            ws.notePane = f.studyNote
            ws.drawer = .notes
            ws.drawerPinned = true
            return ws
        case .search:
            return Workspace(panes: [Pane(id: "p1", content: f.bsbJohn3, link: .pinned)], passage: "\"one and only\"")
        case .ink:
            // B039: one reading pane beside the ink margin.
            return Workspace(panes: [Pane(id: "p1", content: f.bsbJohn3, link: .pinned)])
        }
    }
}
