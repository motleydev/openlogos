// The OpenLogos context menu on the web (B077, specs/22 §4.2): draws the core's `ContextMenu`
// (`ShellHandle.contextMenu`) as a custom menu element, the same structure the Mac app draws as
// an NSMenu: the title as a disabled header, sections between separators, one level of
// submenus, a check for `active`, a swatch dot for `value`, keys as glyphs, and disabled rows
// with their reason in words on a second line. The element knows nothing of commands: choosing
// an enabled row calls `onChoose(item)`; shell.js applies the menu's `focus` and the row.
//
// Keyboard (WAI-ARIA menu pattern): ↑↓ Home End move (disabled rows too, so their reasons are
// read), → opens a submenu and ← closes it, Return or Space runs, Esc closes a submenu and then
// the menu, Tab closes, a printed character jumps to the next row starting with what was typed.
// It closes on a press outside, a scroll, a resize and the window losing focus.

const GAP = 8; // the menu keeps this far from the viewport's edges

let open = null; // { root, menus: [el…], options, opened }

export const isOpen = () => !!open;
export const openMenuElement = () => (open ? open.root : null);

/** Opens `menu` at viewport point (x, y); `keyboard` focuses the first row (else the menu). */
export function openContextMenu(menu, { x, y, keyboard = false, icon, glyphs, onChoose, onClose }) {
  closeContextMenu(false);
  const root = document.createElement("div");
  root.className = "ctx-layer";
  const options = { icon, glyphs, onChoose, onClose };
  open = { root, menus: [], options, typed: "", typedAt: 0 };
  const el = buildMenu(menu.sections, menu.title, options);
  el.setAttribute("aria-label", menu.title ? `${menu.title} menu` : "Context menu");
  root.appendChild(el);
  document.body.appendChild(root);
  open.menus.push(el);
  place(el, x, y);
  if (keyboard) focusRow(el, rows(el)[0]);
  else el.focus({ preventScroll: true });
  window.addEventListener("pointerdown", outside, true);
  window.addEventListener("wheel", dismiss, { capture: true, passive: true });
  window.addEventListener("scroll", dismissScroll, true);
  window.addEventListener("resize", dismiss);
  window.addEventListener("blur", dismiss);
  return el;
}

/** Closes the menu; `restore` hands focus back (onClose decides where). */
export function closeContextMenu(restore = true) {
  if (!open) return;
  const { root, options } = open;
  open = null;
  window.removeEventListener("pointerdown", outside, true);
  window.removeEventListener("wheel", dismiss, { capture: true });
  window.removeEventListener("scroll", dismissScroll, true);
  window.removeEventListener("resize", dismiss);
  window.removeEventListener("blur", dismiss);
  root.remove();
  if (options.onClose) options.onClose(restore);
}

const dismiss = (e) => { if (!(e && e.target && e.target.closest && e.target.closest(".ctx-menu"))) closeContextMenu(false); };
const dismissScroll = (e) => { if (!(e.target && e.target.closest && e.target.closest(".ctx-menu"))) closeContextMenu(false); };
function outside(e) {
  if (open && !open.root.contains(e.target)) closeContextMenu(false);
}

function buildMenu(sections, title, options) {
  const el = document.createElement("div");
  el.className = "ctx-menu";
  el.setAttribute("role", "menu");
  el.tabIndex = -1;
  if (title) {
    const header = document.createElement("div");
    header.className = "ctx-title";
    header.setAttribute("role", "presentation");
    header.textContent = title;
    el.appendChild(header);
  }
  sections.forEach((section, i) => {
    if (i > 0 || title) {
      const hr = document.createElement("div");
      hr.className = "ctx-sep";
      hr.setAttribute("role", "separator");
      el.appendChild(hr);
    }
    const group = document.createElement("div");
    group.setAttribute("role", "group");
    if (section.label) group.setAttribute("aria-label", section.label);
    const radio = section.items.some((item) => item.active);
    for (const item of section.items) group.appendChild(buildRow(item, options, radio));
    el.appendChild(group);
  });
  el.addEventListener("keydown", onKey);
  // A press in the menu keeps the text selection and the note editor's caret.
  el.addEventListener("pointerdown", (e) => e.preventDefault());
  el.addEventListener("contextmenu", (e) => e.preventDefault());
  return el;
}

function buildRow(item, options, radio) {
  const row = document.createElement("div");
  row.className = "ctx-row";
  row.tabIndex = -1;
  row.dataset.command = item.command_id;
  row.setAttribute("role", radio ? "menuitemradio" : "menuitem");
  if (radio) row.setAttribute("aria-checked", String(!!item.active));
  if (item.disabled_text) {
    row.setAttribute("aria-disabled", "true");
    row.title = item.disabled_text;
  }
  const check = document.createElement("span");
  check.className = "ctx-check";
  check.setAttribute("aria-hidden", "true");
  check.textContent = item.active ? "✓" : "";
  const icon = document.createElement("span");
  icon.className = "ctx-icon";
  icon.setAttribute("aria-hidden", "true");
  if (item.value) {
    const dot = document.createElement("span");
    dot.className = "ctx-swatch";
    dot.style.background = item.value;
    icon.appendChild(dot);
  } else {
    icon.innerHTML = options.icon(item.icon);
  }
  const text = document.createElement("span");
  text.className = "ctx-label";
  const label = document.createElement("span");
  label.textContent = item.label;
  text.appendChild(label);
  if (item.disabled_text) {
    const why = document.createElement("span");
    why.className = "ctx-why";
    why.textContent = item.disabled_text;
    text.appendChild(why);
  }
  const end = document.createElement("span");
  end.className = "ctx-end";
  end.setAttribute("aria-hidden", "true");
  const submenu = item.submenu && item.submenu.length ? item.submenu : null;
  if (submenu) {
    end.textContent = "›";
    row.setAttribute("aria-haspopup", "menu");
    row.setAttribute("aria-expanded", "false");
  } else if (item.key) {
    end.textContent = options.glyphs(item.key);
    row.setAttribute("aria-keyshortcuts", item.key);
  }
  row.append(check, icon, text, end);
  row._item = item;
  row.addEventListener("pointerenter", () => {
    focusRow(row.parentElement.closest(".ctx-menu"), row);
    if (submenu) openSubmenu(row, false);
    else closeSubmenusAfter(row.closest(".ctx-menu"));
  });
  row.addEventListener("click", () => activate(row));
  return row;
}

const rows = (menu) => [...menu.querySelectorAll(":scope > [role=group] > .ctx-row")];

function focusRow(menu, row) {
  if (!row) return;
  for (const r of rows(menu)) r.classList.toggle("current", r === row);
  row.focus({ preventScroll: true });
}

/** A submenu parent opens its submenu (on the Mac it cannot run either); an enabled row runs. */
function activate(row, keyboard = false) {
  const item = row._item;
  if (item.submenu && item.submenu.length) { openSubmenu(row, keyboard); return; }
  if (item.disabled_text) return;
  const { onChoose } = open.options;
  closeContextMenu(false);
  onChoose(item);
}

function openSubmenu(row, keyboard) {
  const parent = row.closest(".ctx-menu");
  closeSubmenusAfter(parent);
  const menu = buildMenu([{ label: row._item.label, items: row._item.submenu }], null, open.options);
  menu.classList.add("ctx-submenu");
  menu.setAttribute("aria-label", row._item.label);
  menu._parentRow = row;
  open.root.appendChild(menu);
  open.menus.push(menu);
  row.setAttribute("aria-expanded", "true");
  const r = row.getBoundingClientRect();
  const { width: w, height: h } = menu.getBoundingClientRect();
  const pr = parent.getBoundingClientRect();
  let left = pr.right - 2;
  if (left + w > innerWidth - GAP) left = Math.max(GAP, pr.left - w + 2);
  const top = Math.max(GAP, Math.min(innerHeight - GAP - h, r.top - 5));
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;
  if (keyboard) focusRow(menu, rows(menu)[0]);
}

function closeSubmenusAfter(menu) {
  if (!open) return;
  const i = open.menus.indexOf(menu);
  for (const m of open.menus.splice(i + 1)) {
    if (m._parentRow) m._parentRow.setAttribute("aria-expanded", "false");
    m.remove();
  }
}

function place(el, x, y) {
  const { width: w, height: h } = el.getBoundingClientRect();
  let left = x, top = y;
  if (left + w > innerWidth - GAP) left = Math.max(GAP, x - w);
  if (top + h > innerHeight - GAP) top = Math.max(GAP, innerHeight - GAP - h);
  el.style.left = `${Math.max(GAP, Math.min(left, innerWidth - GAP - w))}px`;
  el.style.top = `${Math.max(GAP, top)}px`;
}

function onKey(e) {
  const menu = e.currentTarget;
  const list = rows(menu);
  const at = list.indexOf(document.activeElement);
  const move = (i) => focusRow(menu, list[(i + list.length) % list.length]);
  switch (e.key) {
    case "ArrowDown": move(at < 0 ? 0 : at + 1); break;
    case "ArrowUp": move(at < 0 ? list.length - 1 : at - 1); break;
    case "Home": move(0); break;
    case "End": move(list.length - 1); break;
    case "ArrowRight":
      if (at >= 0 && list[at]._item.submenu && list[at]._item.submenu.length) openSubmenu(list[at], true);
      break;
    case "ArrowLeft":
      if (menu._parentRow) { const row = menu._parentRow; closeSubmenusAfter(row.closest(".ctx-menu")); focusRow(row.closest(".ctx-menu"), row); }
      break;
    case "Enter": case " ":
      if (at >= 0) activate(list[at], true);
      break;
    case "Escape":
      if (menu._parentRow) { const row = menu._parentRow; closeSubmenusAfter(row.closest(".ctx-menu")); focusRow(row.closest(".ctx-menu"), row); }
      else closeContextMenu(true);
      break;
    case "Tab": closeContextMenu(true); break;
    default:
      if (e.key.length === 1 && !e.metaKey && !e.ctrlKey && !e.altKey) {
        // Type-ahead: the buffer resets after half a second of no typing.
        const now = performance.now();
        open.typed = now - open.typedAt > 500 ? e.key : open.typed + e.key;
        open.typedAt = now;
        const want = open.typed.toLocaleLowerCase();
        const order = [...list.slice(at + (open.typed.length > 1 ? 0 : 1)), ...list.slice(0, at + 1)];
        const hit = order.find((r) => r._item.label.toLocaleLowerCase().startsWith(want));
        if (hit) focusRow(menu, hit);
        break;
      }
      return; // other keys (⌘ chords) are not the menu's
  }
  e.preventDefault();
  e.stopPropagation();
}
