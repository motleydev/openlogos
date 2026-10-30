// The launch session (B072 brief): workspace "Study" with three columns (1 = BSB John 3 with a
// second tab WLC Gen 1, 2 = SBLGNT John 3 in group 1, 3 = the study note pinned as a tab) and
// workspace "Hebrew" with one column (WLC Gen 1). Nothing is persisted yet (B074). The bench
// session is specs/22 §7's ten workspaces of three columns × four tabs.

function tab(id, contentId, reference, mode, kind = "reading") {
  return {
    id, kind, content_id: contentId, content_revision: "0", reference,
    link_group: mode === "pinned" || mode === "follows" ? null : mode, pinned: mode === "pinned",
  };
}

function column(id, tabs, width, shown = tabs[0].id) {
  return { id, tabs, shown_tab_id: shown, width_fraction: width };
}

function workspace(id, title, columns, serial) {
  return {
    schema_version: "2.0", fixture_only: true, id, title, columns, active_column_id: columns[0].id,
    notes_dock: { note_id: null, open: false }, next_serial: serial,
  };
}

export function seedSession() {
  return {
    schema_version: "1.0", fixture_only: true, tool: "select", next_serial: 3, active_workspace_id: "w1",
    workspaces: [
      workspace("w1", "Study", [
        column("c1", [tab("t1", "bsb", "John.3", "1"), tab("t2", "wlc", "Gen.1", "pinned")], 0.36),
        column("c2", [tab("t3", "sblgnt", "John.3", "1")], 0.36),
        column("c3", [tab("t4", "notes", null, "pinned", "notes")], 0.28),
      ], 10),
      workspace("w2", "Hebrew", [column("c1", [tab("t1", "wlc", "Gen.1", "pinned")], 1)], 10),
    ],
  };
}

export function benchSession() {
  const resources = [["bsb", "John.3"], ["sblgnt", "John.3"], ["wlc", "Gen.1"], ["bsb", "Rom.8"]];
  const workspaces = [];
  for (let w = 1; w <= 10; w++) {
    const columns = [];
    for (let c = 1; c <= 3; c++) {
      const tabs = resources.map(([id, ref], t) => tab(`t${c}${t}`, id, ref, t === 0 ? "1" : "pinned"));
      columns.push(column(`c${c}`, tabs, c === 3 ? 0.34 : 0.33));
    }
    workspaces.push(workspace(`w${w}`, `Bench ${w}`, columns, 100));
  }
  return { schema_version: "1.0", fixture_only: true, tool: "select", next_serial: 20, active_workspace_id: "w1", workspaces };
}
