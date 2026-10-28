// B039: golden vectors for the Rust port of perfect-freehand (crates/ink/src/freehand.rs).
//
// Runs the pinned npm package (package.json: perfect-freehand 1.2.3, package-lock.json pins the
// tarball and its integrity) on fixed input point sets with fixed options and writes
// vectors.json beside this file. The Rust test crates/ink/tests/golden.rs reads it and compares
// the port's outline point for point (same count, each coordinate within 1e-5 after this
// file's rounding to six decimals; the port reproduces V8's Math.hypot and the loop
// accumulations, so the remaining drift is the libm sin/cos of each platform).
//
//   cd crates/ink/golden && nix develop ../../..#web -c npm ci && node generate.mjs
//
// Points are the array form `[x, y, pressure?]`; a missing pressure is `null` here and
// `undefined` to the package, which gives 0.25 to a first point and 0.5 after. No randomness:
// the scribble uses a fixed linear congruential generator.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { getStroke } from "perfect-freehand";

// The package does not export its package.json; read the installed copy directly.
const upstream = JSON.parse(readFileSync(new URL("./node_modules/perfect-freehand/package.json", import.meta.url), "utf8"));
if (upstream.version !== "1.2.3") throw new Error(`perfect-freehand is ${upstream.version}, expected 1.2.3`);

const round = (n) => Math.round(n * 1e6) / 1e6;
const lcg = (seed) => () => (seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648;

function line(n, from, to, pressure = () => null) {
  return Array.from({ length: n }, (_, i) => {
    const t = i / (n - 1);
    const p = pressure(t);
    return [from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t, p];
  });
}

function sine(n) {
  return Array.from({ length: n }, (_, i) => {
    const t = i / (n - 1);
    return [20 + t * 300, 100 + Math.sin(t * Math.PI * 2) * 40, round(0.2 + 0.7 * Math.sin(t * Math.PI))];
  });
}

function scribble(n, seed) {
  const next = lcg(seed);
  let x = 50, y = 50;
  return Array.from({ length: n }, () => {
    x += (next() - 0.4) * 14;
    y += (next() - 0.5) * 10;
    return [round(x), round(y), null];
  });
}

const loop = Array.from({ length: 30 }, (_, i) => {
  const a = (i / 29) * Math.PI * 2.3;
  return [round(150 + Math.cos(a) * 60), round(150 + Math.sin(a) * 60), round(0.4 + 0.3 * Math.sin(a))];
});

// Brush definitions mirrored in crates/ink/src/brush.rs (PEN_1, HIGHLIGHTER_1).
const pen1 = { size: 4, thinning: 0.5, smoothing: 0.5, streamline: 0.5, simulatePressure: true, last: true };
const highlighter1 = { size: 12, thinning: 0, smoothing: 0.5, streamline: 0.6, simulatePressure: false, start: { cap: false }, end: { cap: false }, last: true };

const cases = [
  { name: "straight-line-default", options: {}, points: line(10, [10, 10], [190, 60]) },
  { name: "sine-with-pressure-no-simulation", options: { size: 8, thinning: 0.6, simulatePressure: false, last: true }, points: sine(24) },
  { name: "zigzag-sharp-corners", options: { size: 12, smoothing: 0.3 }, points: [[0, 0, null], [60, 0, null], [0, 40, null], [60, 80, null], [0, 120, null], [60, 160, null], [0, 200, null]] },
  { name: "single-point", options: {}, points: [[42.5, 17.25, null]] },
  { name: "single-point-with-pressure-last", options: { last: true }, points: [[42.5, 17.25, 0.9]] },
  { name: "two-points", options: {}, points: [[0, 0, null], [100, 30, null]] },
  { name: "tapered-flat-caps", options: { size: 10, streamline: 0.2, start: { taper: 30, cap: false }, end: { taper: true, cap: false } }, points: line(16, [0, 100], [240, 70], (t) => round(0.3 + 0.5 * t)) },
  { name: "highlighter-brush", options: highlighter1, points: line(14, [30, 88], [260, 92]) },
  { name: "pen-brush-simulated-pressure", options: pen1, points: scribble(40, 7) },
  { name: "pen-brush-real-pressure", options: { ...pen1, simulatePressure: false }, points: scribble(40, 11).map(([x, y], i) => [x, y, round(0.1 + ((i * 37) % 90) / 100)]) },
  { name: "duplicate-points-not-last", options: { size: 6 }, points: [[5, 5, null], [5, 5, null], [5, 5, null], [40, 5, null], [40, 5, null], [80, 20, null], [80, 20, null]] },
  { name: "loop-self-overlap", options: { size: 14, thinning: 0.5, simulatePressure: false, last: true }, points: loop },
  { name: "thinning-one-pressure-zero", options: { size: 10, thinning: 1, simulatePressure: false, last: true }, points: line(8, [0, 0], [120, 0], () => 0) },
];

const vectors = {
  upstream: { name: upstream.name, version: upstream.version, license: upstream.license, repository: String(upstream.repository) },
  generated_by: "crates/ink/golden/generate.mjs",
  rounding: "coordinates rounded to 6 decimals",
  cases: cases.map(({ name, options, points }) => ({
    name,
    options,
    points,
    outline: getStroke(points.map(([x, y, p]) => (p == null ? [x, y] : [x, y, p])), options).map(([x, y]) => [round(x), round(y)]),
  })),
};

const out = fileURLToPath(new URL("./vectors.json", import.meta.url));
writeFileSync(out, JSON.stringify(vectors, null, 1) + "\n");
console.log(`${out}: ${vectors.cases.length} cases, ${vectors.cases.reduce((n, c) => n + c.outline.length, 0)} outline points`);
