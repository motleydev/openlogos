// Display strings for the shell chrome (B072): the human form of a passage, the registry's
// shortcut strings as key glyphs, and the link state word. Short references (`Jn 3:16`) and
// disabled reasons in words come from the core (formatShort, disabled_text).

/** The human form through the core (`John 3:16`), falling back to the stored string. */
export function humanPassage(core, reference) {
  if (!reference) return "";
  try { return core.formatHuman(core.parseReference(reference)); } catch { return reference; }
}

const GLYPHS = { Cmd: "⌘", Alt: "⌥", Shift: "⇧", Ctrl: "⌃" };

/** `Alt+Shift+2` → `⌥⇧2`; `Cmd+K` → `⌘K`; `/` → `/`. Palette rows carry concrete keys (the
 *  core fills a column family's number in). */
export function shortcutGlyphs(shortcut) {
  if (!shortcut) return "";
  return shortcut.split("+").map((part) => GLYPHS[part] || part).join("");
}

/** The link state word of the identity row (specs/21 §2). */
export function linkWord(tab) {
  if (tab.pinned) return "Pinned";
  if (tab.link_group) return `Group ${tab.link_group}`;
  return "Follows";
}
