// Display strings for the shell chrome (B072): short passages for tab labels and toasts, the
// registry's shortcut strings as key glyphs, and the core's disabled reasons as sentences.

// SBL Handbook abbreviations for the OSIS book ids the core uses, for compact tab labels
// (`BSB Jn 3`). An id missing here shows as itself.
const SHORT_BOOKS = {
  Gen: "Gen", Exod: "Exod", Lev: "Lev", Num: "Num", Deut: "Deut", Josh: "Josh", Judg: "Judg", Ruth: "Ruth",
  "1Sam": "1 Sam", "2Sam": "2 Sam", "1Kgs": "1 Kgs", "2Kgs": "2 Kgs", "1Chr": "1 Chr", "2Chr": "2 Chr",
  Ezra: "Ezra", Neh: "Neh", Esth: "Esth", Job: "Job", Ps: "Ps", Prov: "Prov", Eccl: "Eccl", Song: "Song",
  Isa: "Isa", Jer: "Jer", Lam: "Lam", Ezek: "Ezek", Dan: "Dan", Hos: "Hos", Joel: "Joel", Amos: "Amos",
  Obad: "Obad", Jonah: "Jonah", Mic: "Mic", Nah: "Nah", Hab: "Hab", Zeph: "Zeph", Hag: "Hag", Zech: "Zech",
  Mal: "Mal", Matt: "Mt", Mark: "Mk", Luke: "Lk", John: "Jn", Acts: "Acts", Rom: "Rom", "1Cor": "1 Cor",
  "2Cor": "2 Cor", Gal: "Gal", Eph: "Eph", Phil: "Phil", Col: "Col", "1Thess": "1 Thess", "2Thess": "2 Thess",
  "1Tim": "1 Tim", "2Tim": "2 Tim", Titus: "Titus", Phlm: "Phlm", Heb: "Heb", Jas: "Jas", "1Pet": "1 Pet",
  "2Pet": "2 Pet", "1John": "1 Jn", "2John": "2 Jn", "3John": "3 Jn", Jude: "Jude", Rev: "Rev",
};

function shortPoint(osis) {
  const [book, chapter, verse] = osis.split(".");
  const b = SHORT_BOOKS[book] || book;
  return { book: b, chapter, verse, text: verse ? `${b} ${chapter}:${verse}` : chapter ? `${b} ${chapter}` : b };
}

/** `John.3.16` → `Jn 3:16`; `John.3.16-John.3.17` → `Jn 3:16–17`; null → "". */
export function shortPassage(reference) {
  if (!reference) return "";
  const [from, to] = reference.split("-");
  const a = shortPoint(from);
  if (!to) return a.text;
  const b = shortPoint(to);
  if (a.book === b.book && a.chapter === b.chapter && b.verse) return `${a.text}–${b.verse}`;
  if (a.book === b.book) return `${a.text}–${b.chapter}${b.verse ? `:${b.verse}` : ""}`;
  return `${a.text}–${b.text}`;
}

/** The human form through the core (`John 3:16`), falling back to the stored string. */
export function humanPassage(core, reference) {
  if (!reference) return "";
  try { return core.formatHuman(core.parseReference(reference)); } catch { return reference; }
}

const GLYPHS = { Cmd: "⌘", Alt: "⌥", Shift: "⇧", Ctrl: "⌃" };

/** `Alt+Shift+{n}` → `⌥⇧1…9`; `Cmd+K` → `⌘K`; `/` → `/`. */
export function shortcutGlyphs(shortcut) {
  if (!shortcut) return "";
  return shortcut.split("+").map((part) => GLYPHS[part] || (part === "{n}" ? "1…9" : part)).join("");
}

// The core's DisabledReason texts (crates/workspaces/src/commands.rs, thiserror messages): the
// bindings return the reason as data (`{ kind, ... }`), so the sentence is written here.
const REASONS = {
  needs_selection: () => "Select some text first",
  nothing_to_undo: () => "Nothing to undo",
  nothing_to_redo: () => "Nothing to redo",
  no_back_history: () => "No earlier passage in this tab",
  no_forward_history: () => "No later passage in this tab",
  only_workspace: () => "This is the only workspace",
  only_column: () => "This is the only column",
  last_tab: () => "This is the last tab",
  one_tab: () => "This column has one tab",
  no_closed_tab: () => "No closed tab to reopen",
  at_edge: (r) => `Column ${r.number} is already at the edge`,
  no_such_column: (r) => `There is no column ${r.number}`,
  already_in_column: (r) => `The tab is already in column ${r.number}`,
  already_in_mode: (r) => `The tab is already ${r.mode}`,
  no_such_workspace: (r) => `There is no workspace ${r.workspace_id}`,
  no_such_tab: (r) => `There is no tab ${r.tab_id}`,
  needs_argument: (r) => `Needs ${r.argument}`,
  at_limit: (r) => `The ${r.setting} is already at its limit`,
  invalid_value: (r) => r.detail,
};

export function reasonText(disabled) {
  if (!disabled) return "";
  const f = REASONS[disabled.kind];
  return f ? f(disabled) : disabled.kind.replaceAll("_", " ");
}

/** The link state word of the identity row (specs/21 §2). */
export function linkWord(tab) {
  if (tab.pinned) return "Pinned";
  if (tab.link_group) return `Group ${tab.link_group}`;
  return "Follows";
}
