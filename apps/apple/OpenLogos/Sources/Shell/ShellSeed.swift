// B072: the session the shell opens with (nothing is persisted until B074). Workspace
// "Study": column 1 BSB John 3 with a second tab WLC Genesis 1, column 2 SBLGNT John 3 in
// link group 1 with column 1's BSB tab, column 3 the study note pinned as a tab. Workspace
// "Hebrew": one column, WLC Genesis 1. The measurement session is ten workspaces of three
// columns × four tabs (specs/22 §7).
import Foundation

enum ShellSeed {
    private static func tab(_ id: String, _ kind: String, _ content: String, _ reference: String?, group: String? = nil) -> [String: Any] {
        [
            "id": id, "kind": kind, "content_id": content, "content_revision": "0",
            "reference": reference.map { $0 as Any } ?? NSNull(), "link_group": group.map { $0 as Any } ?? NSNull(), "pinned": group == nil,
        ]
    }

    private static func column(_ id: String, _ tabs: [[String: Any]], shown: String, width: Double) -> [String: Any] {
        ["id": id, "tabs": tabs, "shown_tab_id": shown, "width_fraction": width]
    }

    private static func workspace(_ id: String, _ title: String, _ columns: [[String: Any]], note: String? = nil) -> [String: Any] {
        [
            "schema_version": "2.0", "fixture_only": true, "id": id, "title": title, "columns": columns,
            "active_column_id": (columns.first?["id"] as? String) ?? "c1",
            "notes_dock": ["note_id": note.map { $0 as Any } ?? NSNull(), "open": false] as [String: Any], "next_serial": 50,
        ]
    }

    private static func session(_ workspaces: [[String: Any]]) -> String {
        let object: [String: Any] = [
            "schema_version": "1.0", "fixture_only": true, "workspaces": workspaces,
            "active_workspace_id": (workspaces.first?["id"] as? String) ?? "w1", "tool": "select", "next_serial": 50,
            // The tokens' default theme and the first two highlight swatches (design/tokens/shell.json).
            "reading": ["text_size_step": 0, "line_spacing": 1.5, "theme": "light"] as [String: Any],
            "ink": ["width": 4, "swatch": "#F5D90A", "alternate_swatch": "#4CC38A"] as [String: Any],
        ]
        let data = try! JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
        return String(decoding: data, as: UTF8.self)
    }

    /// The preview session.
    static var json: String {
        session([
            workspace("w1", "Study", [
                column("c1", [tab("t1", "reading", "bsb", "John.3.16", group: "1"), tab("t2", "reading", "wlc", "Gen.1.1")], shown: "t1", width: 0.4),
                column("c2", [tab("t3", "reading", "sblgnt", "John.3.16", group: "1")], shown: "t3", width: 0.35),
                column("c3", [tab("t4", "notes", "fixture:note:1", nil)], shown: "t4", width: 0.25),
            ], note: "fixture:note:1"),
            workspace("w2", "Hebrew", [
                column("c1", [tab("t1", "reading", "wlc", "Gen.1.1")], shown: "t1", width: 1),
            ]),
        ])
    }

    /// specs/22 §7's load: ten workspaces × three columns × four tabs over the fixture texts.
    static var measurementJSON: String {
        let references: [(String, String)] = [("bsb", "John.3.16"), ("wlc", "Gen.1.1"), ("sblgnt", "John.3.16"), ("bsb", "Gen.1.1")]
        let workspaces = (1...10).map { w -> [String: Any] in
            let columns = (1...3).map { c -> [String: Any] in
                let tabs = (1...4).map { t -> [String: Any] in
                    let (content, reference) = references[(c + t) % references.count]
                    return tab("t\(c * 10 + t)", "reading", content, reference, group: t == 1 ? "1" : nil)
                }
                return column("c\(c)", tabs, shown: "t\(c * 10 + 1)", width: 1.0 / 3)
            }
            return workspace("w\(w)", "Workspace \(w)", columns)
        }
        return session(workspaces)
    }
}
