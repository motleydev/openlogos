// The preview pack in the browser (B071, docs/preview-pack.md, specs/23 §4): the editions'
// manifests go into the core's ReadingHandle at launch, each chapter's chunk is fetched when a
// stream needs it and handed back (`supplyChunk`, which checks its SHA-256). No format of the
// host's own: the core decides everything about the text.
//
// A pack is looked for at `../preview-pack/` (what `just preview-web` serves, from outside the
// repository) or `?pack=<url>`; `?pack=off` is the placeholder. An edition without a manifest
// there stays the core's placeholder, which says so on every block (specs/23 §5).

export const PACK_EDITIONS = ["bsb", "sblgnt", "wlc"];
/** A chunk that failed is not asked for again for this long, so scrolling past does not hammer. */
const RETRY_MS = 5000;

/** Where the pack is, from the page's address; null for none. */
export function packBase(params) {
  const asked = params.get("pack");
  if (asked === "off") return null;
  const url = new URL(asked || "../preview-pack/", location.href);
  if (!url.pathname.endsWith("/")) url.pathname += "/";
  return url.href;
}

/** Installs what the pack holds into `reading`. Returns the editions installed (the core's
 *  `ReadingEdition`) and those refused, with why. A missing pack is not an error. */
export async function installPacks(reading, base, editions = PACK_EDITIONS) {
  const installed = [], refused = [];
  if (!base) return { installed, refused };
  const texts = await Promise.all(editions.map(async (id) => {
    try {
      const response = await fetch(`${base}packs/${id}/manifest.json`);
      return response.ok ? await response.text() : null;
    } catch {
      return null;
    }
  }));
  texts.forEach((text, i) => {
    if (text == null) return;
    try {
      installed.push(reading.addPackManifest(text));
    } catch (error) {
      refused.push({ id: editions[i], reason: error.message || String(error) });
    }
  });
  return { installed, refused };
}

/** Each pack chapter's chunk, fetched once and supplied to the core. */
export class Chunks {
  constructor(reading, base, editions) {
    this.reading = reading;
    this.base = base;
    this.packs = new Set(editions);
    this.supplied = new Set();
    this.pending = new Map();
    this.failures = new Map();
    this.stats = { fetched: 0, bytes: 0, fetch_ms: 0, supply_ms: 0, failed: 0 };
  }

  /** Whether `chapterContent` can answer now (a generated edition always can). */
  ready(id, key) {
    return !this.packs.has(id) || this.supplied.has(`${id}|${key}`);
  }

  /** Whether the edition has that chapter at all. */
  has(id, key) {
    try { this.reading.chapterNeighbors(id, key); return true; } catch { return false; }
  }

  /** A recent failure for this chapter, or null. */
  failure(id, key) {
    const f = this.failures.get(`${id}|${key}`);
    return f && performance.now() - f.at < RETRY_MS ? f.error : null;
  }

  /** Fetches and supplies a chapter once; resolves when `chapterContent` can answer. */
  request(id, key) {
    if (this.ready(id, key)) return Promise.resolve();
    const name = `${id}|${key}`;
    let pending = this.pending.get(name);
    if (!pending) {
      pending = this.fetch(id, key).then(() => {
        this.supplied.add(name);
        this.failures.delete(name);
      }, (error) => {
        this.stats.failed += 1;
        this.failures.set(name, { at: performance.now(), error });
        throw error;
      }).finally(() => this.pending.delete(name));
      this.pending.set(name, pending);
    }
    return pending;
  }

  async fetch(id, key) {
    const t0 = performance.now();
    const chunk = this.reading.chunkFor(id, key);
    const url = `${this.base}packs/${id}/${chunk.path}`;
    // A bundle is read by range; a server that ignores Range sends it whole, and it is cut here.
    const ranged = chunk.offset != null;
    const headers = ranged ? { Range: `bytes=${chunk.offset}-${chunk.offset + chunk.length - 1}` } : undefined;
    const response = await fetch(url, { headers });
    if (!response.ok) throw new Error(`${chunk.path}: HTTP ${response.status}`);
    let bytes = new Uint8Array(await response.arrayBuffer());
    if (ranged && response.status !== 206) bytes = bytes.subarray(chunk.offset, chunk.offset + chunk.length);
    const t1 = performance.now();
    this.reading.supplyChunk(id, key, bytes);
    this.stats.fetched += 1;
    this.stats.bytes += bytes.length;
    this.stats.fetch_ms += t1 - t0;
    this.stats.supply_ms += performance.now() - t1;
  }
}
