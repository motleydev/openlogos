// Keyboard bindings read from the core registry (specs/22 §6, B070): every `shortcuts.web`
// string of every command becomes a matcher over KeyboardEvent.code and modifiers. Nothing is
// bound here that the registry does not list. Matching is by `code` (the physical key), so ⌥2
// works although macOS turns it into `™`; the names assume a US layout, as the registry does.
//
//   "Cmd+K"          ⌘ (metaKey on a Mac, Ctrl elsewhere) + K
//   "Alt+Shift+{n}"  ⌥⇧ + a digit 1…9, which becomes args.column
//   "V", "/"         a bare key: only when no text field has focus and no modifier is down

const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

const CODES = { "[": "BracketLeft", "]": "BracketRight", "/": "Slash", Tab: "Tab", Enter: "Enter", Escape: "Escape" };

function codeOf(key) {
  if (/^[A-Z]$/.test(key)) return `Key${key}`;
  if (/^[0-9]$/.test(key)) return `Digit${key}`;
  return CODES[key] || null;
}

function parse(commandId, shortcut, bare) {
  const parts = shortcut.split("+");
  const key = parts.pop();
  const mods = new Set(parts);
  const binding = { commandId, shortcut, cmd: mods.has("Cmd"), alt: mods.has("Alt"), shift: mods.has("Shift"), ctrl: mods.has("Ctrl"), bare: bare && parts.length === 0 };
  if (key === "{n}") binding.family = true;
  else binding.code = codeOf(key);
  return binding.family || binding.code ? binding : null;
}

/** Every web binding of the registry. */
export function bindingsFrom(registry) {
  const out = [];
  for (const spec of registry) {
    for (const s of spec.shortcuts.web) {
      const b = parse(spec.id, s, spec.bare_key);
      if (b) out.push(b);
    }
  }
  return out;
}

/** The invocation a key event triggers, or null. `inTextField` blocks bare keys. */
export function match(bindings, event, inTextField) {
  const cmd = isMac ? event.metaKey : event.ctrlKey;
  const ctrl = isMac ? event.ctrlKey : false;
  for (const b of bindings) {
    if (b.bare && inTextField) continue;
    if (b.cmd !== cmd || b.alt !== event.altKey || b.ctrl !== ctrl) continue;
    if (b.shift !== event.shiftKey) continue;
    if (b.family) {
      const m = /^Digit([1-9])$/.exec(event.code);
      if (m) return { command_id: b.commandId, args: { column: Number(m[1]) } };
      continue;
    }
    if (event.code === b.code) return { command_id: b.commandId, args: {} };
  }
  return null;
}
