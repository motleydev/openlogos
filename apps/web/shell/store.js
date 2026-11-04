// The user store on the web (B074, specs/24 §7): one IndexedDB database `openlogos`, object
// store `user-store`, the JSON the core wrote (`exportStore`) under the key `current`, and at
// most five backups under `backup-<ISO8601>` (a file that could not be read whole, or whose
// restore set something aside). One `put` per save in a `readwrite` transaction, which is
// atomic: a failed write leaves the previous value. The page only reads and writes strings;
// what they mean is the core's.

export const STORE_NAME = "user-store";
export const CURRENT = "current";
export const BACKUPS_KEPT = 5;

const promised = (request) => new Promise((ok, fail) => {
  request.onsuccess = () => ok(request.result);
  request.onerror = () => fail(request.error);
});

export class UserStore {
  /** `name`: the database (`openlogos`; the tests use their own). */
  constructor(name = "openlogos") {
    this.name = name;
    this.db = null;
  }

  async open() {
    if (this.db) return this.db;
    const request = indexedDB.open(this.name, 1);
    request.onupgradeneeded = () => { if (!request.result.objectStoreNames.contains(STORE_NAME)) request.result.createObjectStore(STORE_NAME); };
    this.db = await promised(request);
    // Another tab upgrading the database closes this one; the next call opens it again.
    this.db.onversionchange = () => { this.db.close(); this.db = null; };
    return this.db;
  }

  /** The page's synchronous copy of a store kept as it was hidden (see `keepPending`). */
  get pendingKey() { return `${this.name}:${CURRENT}:pending`; }

  /** Keeps `json` in localStorage, synchronously: a page being closed or reloaded is torn down
   *  before IndexedDB commits a write started as it goes. */
  keepPending(json) {
    try { localStorage.setItem(this.pendingKey, json); } catch { /* storage full or off */ }
  }

  /** The copy kept on hide, newer than IndexedDB's when present; null otherwise. */
  pending() {
    try { return localStorage.getItem(this.pendingKey); } catch { return null; }
  }

  clearPending() {
    try { localStorage.removeItem(this.pendingKey); } catch { /* storage off */ }
  }

  /** The saved store, or null when there is none. */
  async read() {
    const db = await this.open();
    const value = await promised(db.transaction(STORE_NAME, "readonly").objectStore(STORE_NAME).get(CURRENT));
    return value == null ? null : String(value);
  }

  /** Writes `json` as the current store; resolves when the transaction has committed. */
  async write(json, key = CURRENT) {
    const db = await this.open();
    const tx = db.transaction(STORE_NAME, "readwrite", { durability: "strict" });
    tx.objectStore(STORE_NAME).put(json, key);
    await new Promise((ok, fail) => {
      tx.oncomplete = () => ok();
      tx.onerror = () => fail(tx.error);
      tx.onabort = () => fail(tx.error || new Error("the write was aborted"));
    });
  }

  /** Keeps a copy of `json` as `backup-<ISO8601>` and drops all but the newest five. */
  async backup(json, now = new Date()) {
    const key = `backup-${now.toISOString()}`;
    await this.write(json, key);
    await this.prune();
    return key;
  }

  /** Moves a store that could not be read whole aside: kept as a backup, gone as `current`
   *  (one transaction), so the next launch starts fresh without saying so again. */
  async setAside(json, now = new Date()) {
    const key = `backup-${now.toISOString()}`;
    const db = await this.open();
    const tx = db.transaction(STORE_NAME, "readwrite");
    tx.objectStore(STORE_NAME).put(json, key);
    tx.objectStore(STORE_NAME).delete(CURRENT);
    await new Promise((ok, fail) => { tx.oncomplete = () => ok(); tx.onerror = () => fail(tx.error); });
    await this.prune();
    return key;
  }

  /** Drops all but the newest five backups. */
  async prune() {
    const keys = await this.backups();
    const old = keys.slice(0, Math.max(0, keys.length - BACKUPS_KEPT));
    if (!old.length) return;
    const db = await this.open();
    const tx = db.transaction(STORE_NAME, "readwrite");
    for (const k of old) tx.objectStore(STORE_NAME).delete(k);
    await new Promise((ok, fail) => { tx.oncomplete = () => ok(); tx.onerror = () => fail(tx.error); });
  }

  /** The backup keys, oldest first (ISO 8601 sorts by time). */
  async backups() {
    const db = await this.open();
    const keys = await promised(db.transaction(STORE_NAME, "readonly").objectStore(STORE_NAME).getAllKeys());
    return keys.map(String).filter((k) => k.startsWith("backup-")).sort();
  }

  async get(key) {
    const db = await this.open();
    return promised(db.transaction(STORE_NAME, "readonly").objectStore(STORE_NAME).get(key));
  }
}

/** The quiet notice for a store the core refused whole (specs/24 §6). */
export function refusedNotice(error) {
  return error && error.kind === "unsupported_version"
    ? "Your saved workspace was written by a newer version of OpenLogos; a copy was kept."
    : "Your saved workspace could not be read; a copy was kept.";
}

const PARTS = [["note", "note", "notes"], ["highlight", "highlight", "highlights"], ["ink", "ink stroke", "ink strokes"]];

/** The notice naming what a restore set aside, or null when nothing was. */
export function issuesNotice(report) {
  const issues = (report && report.issues) || [];
  if (!issues.length) return null;
  const parts = [];
  if (issues.some((i) => i.part === "session")) parts.push("Your workspaces were reset");
  for (const [part, one, many] of PARTS) {
    const n = issues.filter((i) => i.part === part).length;
    if (n) parts.push(`${n} ${n === 1 ? one : many} could not be restored`);
  }
  const text = `${parts.join("; ")}; a copy was kept.`;
  return text[0].toUpperCase() + text.slice(1);
}

export const WRITE_FAILED_NOTICE = "Your changes could not be saved; the last saved copy is kept.";
