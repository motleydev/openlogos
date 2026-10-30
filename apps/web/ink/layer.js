// B039's ink brushes, compositing and palm rules, shared by the ink prototype (ink.js) and the
// workspace shell's margin layer (B072, ../shell). Pure functions over a 2D context and the
// core's wasm module; no page state.
//
//   - brush.rs: PEN_1 and HIGHLIGHTER_1 options, the one-composite-per-stroke highlighter rule
//     (a stroke's outline is filled once at its opacity; it never darkens over itself).
//   - touch.rs: the palm and finger/stylus arbiter (TouchArbiter, same case names).

// MARK: Brushes (mirror of crates/ink/src/brush.rs)

export const ENGINE = "pf-1.2.3";
export const BRUSHES = {
  pen: { version: `${ENGINE}/pen-1`, tool: "pen", usesPressure: true, opacity: 1, options: { thinning: 0.5, smoothing: 0.5, streamline: 0.5, simulatePressure: true, last: true, capStart: true, capEnd: true } },
  highlighter: { version: `${ENGINE}/highlighter-1`, tool: "highlighter", usesPressure: false, opacity: 0.35, options: { thinning: 0, smoothing: 0.5, streamline: 0.6, simulatePressure: false, last: true, capStart: false, capEnd: false } },
};
export const HIGHLIGHTER_WIDTH_FACTOR = 3; // the width slider is a pen width; the band is wider

// MARK: Touch arbiter (mirror of crates/ink/src/touch.rs)

export const PALM_RADIUS_PX = 20;
export class TouchArbiter {
  constructor(mode) { this.mode = mode; this.active = null; this.live = new Map(); }
  get strokeInProgress() { return this.active !== null; }
  /** event: { id, contact: "stylus"|"finger"|"pointer", phase: "began"|"moved"|"ended"|"cancelled", pane, radius } */
  decide(event) {
    if (event.phase === "began") {
      const decision = this.decideBegan(event);
      if (decision === "draw") this.active = { id: event.id, contact: event.contact };
      if (decision === "cancel-stroke-then-draw") { if (this.active) this.live.delete(this.active.id); this.active = { id: event.id, contact: event.contact }; }
      this.live.set(event.id, decision === "cancel-stroke-then-draw" ? "draw" : decision);
      return decision;
    }
    const decision = this.live.get(event.id) ?? "ignore:unknown";
    if (event.phase === "ended" || event.phase === "cancelled") {
      this.live.delete(event.id);
      if (this.active && this.active.id === event.id) this.active = null;
    }
    return decision;
  }
  decideBegan(event) {
    const inCanvas = event.pane === 0;
    if (event.contact === "stylus") {
      if (this.active && this.active.contact === "finger" && inCanvas) return "cancel-stroke-then-draw";
      if (this.active && inCanvas) return "ignore:second-finger";
      return inCanvas ? "draw" : `scroll:${event.pane}`;
    }
    if (event.contact === "pointer") {
      if (inCanvas && !this.active) return "draw";
      return inCanvas ? "ignore:second-finger" : `scroll:${event.pane}`;
    }
    if (event.radius != null && event.radius >= PALM_RADIUS_PX) return "ignore:large-contact";
    if (this.active) return inCanvas && this.mode === "finger" ? "ignore:second-finger" : "ignore:palm-during-stroke";
    if (inCanvas) return this.mode === "finger" ? "draw" : "ignore:finger-in-stylus-mode";
    return `scroll:${event.pane}`;
  }
}

export function outlinePath(flat) {
  const path = new Path2D();
  if (flat.length < 4) return path;
  path.moveTo(flat[0], flat[1]);
  for (let i = 2; i < flat.length; i += 2) path.lineTo(flat[i], flat[i + 1]);
  path.closePath();
  return path;
}

export function brushOptions(stroke, brush, hasPressure) {
  const usePressure = brush.usesPressure && hasPressure;
  return { ...brush.options, size: stroke.width, simulatePressure: brush.options.simulatePressure && !usePressure };
}

export function flatPoints(stroke, usePressure) {
  const flat = new Float64Array(stroke.points.length * 3);
  stroke.points.forEach((p, i) => { flat[i * 3] = p.x; flat[i * 3 + 1] = p.y; flat[i * 3 + 2] = usePressure && p.pressure != null ? p.pressure : -1; });
  return flat;
}

/** The compositing rule: one fill of the whole outline, non-zero, at the stroke's opacity. */
export function compositeAt(target, stroke, flatOutline, dpr) {
  target.save();
  target.scale(dpr, dpr);
  target.globalAlpha = stroke.opacity;
  target.fillStyle = stroke.color;
  target.fill(outlinePath(flatOutline), "nonzero");
  target.restore();
}

export function strokeOutline(core, stroke) {
  const brush = BRUSHES[stroke.tool];
  const hasPressure = stroke.points.every((p) => p.pressure != null);
  const usePressure = brush.usesPressure && hasPressure;
  return core.inkOutline(flatPoints(stroke, usePressure), brushOptions(stroke, brush, hasPressure));
}

export function distanceToSegment(p, a, b) {
  const dx = b.x - a.x, dy = b.y - a.y;
  const l2 = dx * dx + dy * dy;
  const t = l2 === 0 ? 0 : Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / l2));
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}
