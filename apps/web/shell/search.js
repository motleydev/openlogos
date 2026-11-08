// B071: the page's side of text search (search-worker.js). The core's palette on the page answers
// passages, commands and tabs within the keystroke; the Text section comes back from the worker
// and is put into that result. `state` is "loading", "ready" or "failed"; a search that is not
// ready says so in the palette rather than looking like no results.

/** Characters of plain typing before the core asks for text (palette.rs TEXT_MIN_CHARS). */
const TEXT_MIN_CHARS = 2;

/** Whether the core would ask for text for this parsed query. */
export function wantsText(query) {
  if (!query.text) return false;
  return query.mode === "text" || (query.mode === "all" && [...query.text].length >= TEXT_MIN_CHARS);
}

export class TextSearch {
  /** `onChange()`: the state or progress changed; `onResult(seq, group)`: the worker's answer. */
  constructor({ base, editions, titles, onChange, onResult }) {
    this.state = "idle";
    this.percent = 0;
    this.reason = null;
    this.timing = null;
    this.bytes = 0;
    this.seq = 0;
    this.started = null;
    this.urls = editions.map((id) => `${base}search/${id}.sqlite`);
    this.titles = titles;
    this.onChange = onChange;
    this.onResult = onResult;
  }

  /** Starts the worker and the packs' download; once. */
  start() {
    if (this.worker) return;
    this.state = "loading";
    this.started = performance.now();
    this.worker = new Worker(new URL("./search-worker.js", import.meta.url), { type: "module" });
    this.worker.onerror = (e) => this.fail(e.message || "the worker did not start");
    this.worker.onmessage = ({ data }) => this.message(data);
    this.worker.postMessage({ type: "load", urls: this.urls, titles: this.titles });
    this.onChange();
  }

  fail(reason) {
    this.state = "failed";
    this.reason = reason;
    console.warn(`text search could not load: ${reason}`);
    this.onChange();
  }

  message(data) {
    if (data.type === "progress") {
      const percent = data.total ? Math.floor((data.loaded / data.total) * 100) : 0;
      if (percent !== this.percent) { this.percent = percent; this.onChange(); }
    } else if (data.type === "ready") {
      this.state = "ready";
      this.timing = { ...data.timing, first_load_ms: performance.now() - this.started };
      this.bytes = data.bytes;
      this.memory = data.memory;
      this.onChange();
    } else if (data.type === "failed") {
      this.fail(data.reason);
    } else if (data.type === "result" && data.seq === this.seq) {
      if (data.error) console.warn(`text search: ${data.error}`);
      this.onResult(data.seq, data.group);
    }
  }

  /** Asks for the Text section of the field's text; the newest question is the only one
   *  answered on screen. Returns its number. */
  ask(text, expanded) {
    this.seq += 1;
    this.worker.postMessage({ type: "query", seq: this.seq, text, expanded });
    return this.seq;
  }

  /** The words for a search that has nothing to show yet; null when it is ready. */
  statusLine() {
    if (this.state === "failed") return "Text search could not load.";
    if (this.state === "ready") return null;
    return this.percent ? `Text search is loading… ${this.percent}%` : "Text search is loading…";
  }
}
