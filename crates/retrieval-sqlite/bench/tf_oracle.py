#!/usr/bin/env python3
"""Text-Fabric oracle for benchmark R6 (specs/19).

Evaluates the same query list the engine runs (bench/r6-queries.json) directly against the
Text-Fabric data with Text-Fabric's own API (F, L, E), independently of the engine, and
compares the counts with the engine's JSON output. Semantics implemented, as the retrieval
traits document them:

- a slot matches a word when every `feature=value` predicate holds (`F.<feature>.v(w)`);
- every slot of a match lies in the same unit of `scope`: verse/chapter/book from the section
  features, or the *innermost* container of a kind (the smallest container node of that type
  containing the word, `L.u(w, otype)` by slot count);
- slots bind distinct words; counts are ordered tuples;
- WITHIN n: |a-b| <= n; BEFORE n: 0 < b-a <= n (any gap when n is null);
- AGREE(f...): equal non-empty values on both words;
- PEER kind: same innermost container; XPEER: both inside containers of the kind, different
  ones; ANCESTOR: some container of the kind contains both (exact slot membership).

Two numbers are reported for container semantics where they can differ: `exact`, with
Text-Fabric's true (possibly discontinuous) slot membership, and `hull`, treating each
container as the contiguous span from its first to its last slot, which is how the engine
stores containers in this spike. Agreement with `exact` is the correctness claim; a match
with `hull` only is reported as such.

Usage:
    tf_oracle.py <tf-dir> <queries.json> [--engine engine.json] [--out oracle.json]
"""

import json
import sys
import time
import unicodedata
from collections import defaultdict

from tf.fabric import Fabric

BOOKS = {
    "Matthew": 41, "Mark": 42, "Luke": 43, "John": 44, "Acts": 45, "Romans": 46,
    "I_Corinthians": 47, "II_Corinthians": 48, "Galatians": 49, "Ephesians": 50,
    "Philippians": 51, "Colossians": 52, "I_Thessalonians": 53, "II_Thessalonians": 54,
    "I_Timothy": 55, "II_Timothy": 56, "Titus": 57, "Philemon": 58, "Hebrews": 59,
    "James": 60, "I_Peter": 61, "II_Peter": 62, "I_John": 63, "II_John": 64,
    "III_John": 65, "Jude": 66, "Revelation": 67,
}

FEATURES = "lemma sp case number gender tense voice mood person degree book chapter verse otype oslots"


def arg(flag, default=None):
    if flag in sys.argv:
        return sys.argv[sys.argv.index(flag) + 1]
    return default


def main():
    tf_dir, queries_path = sys.argv[1], sys.argv[2]
    engine_path = arg("--engine")
    out_path = arg("--out")
    queries = json.load(open(queries_path, encoding="utf-8"))

    t0 = time.time()
    TF = Fabric(locations=tf_dir, silent="deep")
    api = TF.load(FEATURES, silent="deep")
    F, L, E = api.F, api.L, api.E
    max_slot = F.otype.maxSlot
    words = range(1, max_slot + 1)
    load_s = time.time() - t0

    # Section keys per word: verse id in BBCCCVVV form, like the engine.
    verse_id = {}
    for w in words:
        b = BOOKS[F.book.v(w)]
        verse_id[w] = b * 1_000_000 + int(F.chapter.v(w)) * 1000 + int(F.verse.v(w))

    # Containers per kind: node -> (first, last, slotset); innermost per word, both exact
    # (true membership) and hull (span containment).
    kinds = ["sentence", "clause", "phrase", "wg"]
    containers = {}
    innermost_exact = {}
    innermost_hull = {}
    for kind in kinds:
        nodes = {}
        for n in F.otype.s(kind):
            slots = E.oslots.s(n)
            nodes[n] = (slots[0], slots[-1], frozenset(slots))
        containers[kind] = nodes
        exact = {}
        for w in words:
            ups = [n for n in L.u(w, otype=kind)]
            exact[w] = min(ups, key=lambda n: len(nodes[n][2])) if ups else None
        innermost_exact[kind] = exact
        # Hull: assign largest spans first so smaller ones overwrite, as the pack builder does.
        hull = [None] * (max_slot + 2)
        for n, (first, last, _) in sorted(nodes.items(), key=lambda kv: -(kv[1][1] - kv[1][0])):
            for s in range(first, last + 1):
                hull[s] = n
        innermost_hull[kind] = hull
    prep_s = time.time() - t0 - load_s

    feature_cache = {}

    def fval(feature, w):
        key = feature
        if key not in feature_cache:
            feature_cache[key] = getattr(F, feature).v
        v = feature_cache[key](w)
        if v in (None, ""):
            return None
        # Feature values compare in NFC on both sides (N1904 lemmas carry oxia code points).
        return unicodedata.normalize("NFC", str(v))

    def candidates(slot, book):
        out = []
        for w in words:
            if book is not None and BOOKS[F.book.v(w)] != book:
                continue
            ok = True
            for p in slot["predicates"]:
                if fval(p["feature"], w) != unicodedata.normalize("NFC", p["value"]):
                    ok = False
                    break
            if ok:
                out.append(w)
        return out

    def unit_key(scope, w, mode):
        if scope == "Verse":
            return verse_id[w]
        if scope == "Chapter":
            return verse_id[w] // 1000
        if scope == "Book":
            return verse_id[w] // 1_000_000
        kind = scope["Container"]
        return innermost_exact[kind][w] if mode == "exact" else innermost_hull[kind][w]

    def ancestor(kind, a, b, mode):
        if mode == "exact":
            ups_a = set(L.u(a, otype=kind))
            return any(b in containers[kind][n][2] for n in ups_a)
        # hull: walk from innermost hull container upwards through enclosing spans
        nodes = containers[kind]
        n = innermost_hull[kind][a]
        if n is None:
            return False
        first, last, _ = nodes[n]
        if first <= b <= last:
            return True
        # any container of this kind whose hull contains both
        return any(f <= a <= l and f <= b <= l for (f, l, _) in nodes.values())

    def satisfied(c, bound, mode):
        (name, p), = c.items()
        a, b = p["a"], p["b"]
        if a >= len(bound) or b >= len(bound):
            return True
        x, y = bound[a], bound[b]
        if name == "Within":
            return abs(x - y) <= p["distance"]
        if name == "Before":
            d = p["distance"]
            return x < y and (d is None or y - x <= d)
        if name == "Agree":
            for f in p["features"]:
                vx, vy = fval(f, x), fval(f, y)
                if vx is None or vx != vy:
                    return False
            return True
        kind = p["unit"]["Container"]
        inner = innermost_exact[kind] if mode == "exact" else innermost_hull[kind]
        if name == "Peer":
            return inner[x] is not None and inner[x] == inner[y]
        if name == "XPeer":
            return inner[x] is not None and inner[y] is not None and inner[x] != inner[y]
        if name == "Ancestor":
            return ancestor(kind, x, y, mode) or ancestor(kind, y, x, mode)
        raise ValueError(name)

    def evaluate(q, mode):
        slots = q["slots"]
        cands = [candidates(s, q["book"]) for s in slots]
        if len(slots) == 1:
            return len(cands[0])
        grouped = [None]
        for c in cands[1:]:
            g = defaultdict(list)
            for w in c:
                k = unit_key(q["scope"], w, mode)
                if k is not None:
                    g[k].append(w)
            grouped.append(g)
        count = 0

        def extend(bound, key):
            nonlocal count
            i = len(bound)
            if i == len(slots):
                count += 1
                return
            for w in grouped[i].get(key, ()):
                if w in bound:
                    continue
                bound.append(w)
                if all(satisfied(c, bound, mode) for c in q["constraints"]):
                    extend(bound, key)
                bound.pop()

        for w in cands[0]:
            key = unit_key(q["scope"], w, mode)
            if key is None:
                continue
            extend([w], key)
        return count

    def uses_containers(q):
        if isinstance(q["scope"], dict):
            return True
        return any(name in ("Peer", "XPeer", "Ancestor") for c in q["constraints"] for name in c)

    engine = {}
    if engine_path:
        for r in json.load(open(engine_path, encoding="utf-8")):
            engine[r["id"]] = r

    results = []
    matches = 0
    hull_only = 0
    mismatches = 0
    print(f"Text-Fabric load {load_s:.1f} s, container preparation {prep_s:.1f} s, {max_slot} words")
    print("\n| id | family | TF exact | TF hull | engine | verdict | oracle s |\n|---|---|---:|---:|---:|---|---:|")
    for q in queries:
        t = time.time()
        exact = evaluate(q["query"], "exact")
        hull = evaluate(q["query"], "hull") if uses_containers(q["query"]) else exact
        took = time.time() - t
        e = engine.get(q["id"], {}).get("count")
        if e is None:
            verdict = "no engine result"
        elif e == exact:
            verdict = "MATCH"
            matches += 1
        elif e == hull:
            verdict = "hull-only match"
            hull_only += 1
        else:
            verdict = "MISMATCH"
            mismatches += 1
        results.append({"id": q["id"], "family": q["family"], "tf_exact": exact, "tf_hull": hull, "engine": e, "verdict": verdict, "oracle_s": took})
        print(f"| {q['id']} | {q['family']} | {exact} | {hull} | {e if e is not None else '-'} | {verdict} | {took:.2f} |")
    print(f"\n{matches} exact matches, {hull_only} hull-only matches, {mismatches} mismatches of {len(queries)} queries")
    if out_path:
        json.dump(results, open(out_path, "w", encoding="utf-8"), indent=1, ensure_ascii=False)
        print(f"wrote {out_path}")


if __name__ == "__main__":
    main()
