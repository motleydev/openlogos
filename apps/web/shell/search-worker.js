// B071: text search off the main thread. The shell keeps its core on the page, where the palette
// answers within a frame; this worker holds a second instance of the module, whose only job is
// the palette's Text section over the search packs (`useEditionSearch`, the core's own query, so
// words, ranking and rows are the core's). Each pack is fetched once and held here, in SQLite's
// memory. A query that a newer one has overtaken while waiting is not run.
import init, * as core from "../core-host/pkg/openlogos_ffi.js";

let shell = null;
let latest = null;
let scheduled = false;

/** A session with one reading tab: all the core needs to build a Text row. Whether a row can
 *  run against the real session is the page's core's to say when it runs. */
const SESSION = {
  schema_version: "1.0", fixture_only: true, tool: "select", next_serial: 2, active_workspace_id: "w",
  workspaces: [{
    schema_version: "2.0", fixture_only: true, id: "w", title: "Search", active_column_id: "c", next_serial: 2,
    notes_dock: { note_id: null, open: false },
    columns: [{ id: "c", shown_tab_id: "t", width_fraction: 1, tabs: [{ id: "t", kind: "reading", content_id: "bsb", content_revision: "0", reference: "Gen.1.1", link_group: null, pinned: false }] }],
  }],
};

async function load({ urls, titles }) {
  const t0 = performance.now();
  await init();
  const t1 = performance.now();
  const sizes = urls.map(() => ({ loaded: 0, total: 0 }));
  const progress = () => postMessage({ type: "progress", loaded: sizes.reduce((a, s) => a + s.loaded, 0), total: sizes.reduce((a, s) => a + s.total, 0) });
  const packs = await Promise.all(urls.map(async (url, i) => {
    const response = await fetch(url);
    if (!response.ok) throw new Error(`${url.split("/").pop()}: HTTP ${response.status}`);
    sizes[i].total = Number(response.headers.get("content-length")) || 0;
    const parts = [];
    const reader = response.body.getReader();
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      parts.push(value);
      sizes[i].loaded += value.length;
      progress();
    }
    const bytes = new Uint8Array(sizes[i].loaded);
    let at = 0;
    for (const part of parts) { bytes.set(part, at); at += part.length; }
    return bytes;
  }));
  const t2 = performance.now();
  shell = new core.ShellHandle(SESSION);
  shell.setResourceTitles(titles);
  shell.useEditionSearch(packs);
  const t3 = performance.now();
  postMessage({
    type: "ready",
    timing: { module_ms: t1 - t0, fetch_ms: t2 - t1, attach_ms: t3 - t2, total_ms: t3 - t0 },
    bytes: packs.reduce((a, p) => a + p.length, 0),
  });
}

function drain() {
  scheduled = false;
  if (!latest) return;
  const { seq, text, expanded } = latest;
  latest = null;
  const t0 = performance.now();
  let group = null, error = null;
  try {
    const result = shell.paletteQuery(text, { platform: "web", expanded });
    group = result.groups.find((g) => g.section === "text") || null;
  } catch (e) {
    error = e.message || String(e);
  }
  postMessage({ type: "result", seq, group, error, ms: performance.now() - t0 });
}

onmessage = ({ data }) => {
  if (data.type === "load") {
    load(data).catch((error) => postMessage({ type: "failed", reason: error.message || String(error) }));
  } else if (data.type === "query" && shell) {
    latest = data;
    if (!scheduled) { scheduled = true; setTimeout(drain, 0); }
  }
};
