//! The browser side of B061 item 3: the wasm-bindgen surface exercised inside a dedicated
//! worker, where the web reader runs the core (docs/adr/0001-rust-core-native-ui.md). The
//! same four checks run in Node from apps/web/core-host. `just test-wasm` drives headless
//! Chrome through `wasm-pack test --headless --chrome`.

#![cfg(target_arch = "wasm32")]

use openlogos_conformance::{ConformanceReport, Fixture};
use openlogos_core::anchor::{Anchor, ScalarRange, Segment, TokenId, TokenSpan};
use openlogos_ffi::wasm::{
    Paragraph, anchor_to_utf16, api_version, conformance_fixtures_json, format_osis,
    format_token_id, parse_reference, run_conformance, selection_to_anchor,
};
use openlogos_text::Utf16Range;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::{console_log, wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_dedicated_worker);

fn property(value: &JsValue, name: &str) -> JsValue {
    js_sys::Reflect::get(value, &JsValue::from_str(name)).unwrap()
}

#[wasm_bindgen_test]
fn runs_in_a_dedicated_worker() {
    // `DedicatedWorkerGlobalScope` exists only inside a worker; a window has none.
    let scope = property(&js_sys::global(), "DedicatedWorkerGlobalScope");
    assert!(
        !scope.is_undefined(),
        "tests are running in a window, not a worker"
    );
}

#[wasm_bindgen_test]
fn api_version_is_the_core_version() {
    assert_eq!(api_version(), openlogos_core::API_VERSION);
}

#[wasm_bindgen_test]
fn parses_a_compound_citation() {
    let set = parse_reference("Jn 3:16, 18; Rom 8:28-30").unwrap();
    assert_eq!(js_sys::Array::from(&property(&set, "ranges")).length(), 3);
    assert_eq!(
        format_osis(set).unwrap(),
        "John.3.16;John.3.18;Rom.8.28-Rom.8.30"
    );

    let error = parse_reference("Hezekiah 3:16").unwrap_err();
    assert!(error.is_instance_of::<js_sys::Error>());
    assert_eq!(
        property(&error, "kind").as_string().as_deref(),
        Some("unknown_book")
    );
    assert_eq!(
        property(&error, "message").as_string().as_deref(),
        Some("unknown book `Hezekiah`")
    );
}

#[wasm_bindgen_test]
fn utf16_selection_round_trips_through_an_anchor() {
    let fixtures: Vec<Fixture> = serde_json::from_str(&conformance_fixtures_json()).unwrap();
    let Some(Fixture::Text(hebrew)) = fixtures
        .into_iter()
        .find(|f| f.id() == "hebrew-cantillation-genesis-1")
    else {
        panic!("Hebrew fixture missing");
    };
    assert!(hebrew.selections.len() >= 5);
    for selection in &hebrew.selections {
        let segment = hebrew
            .segments
            .iter()
            .find(|s| s.segment_id == selection.segment_id)
            .unwrap();
        let paragraph = Paragraph {
            edition_id: hebrew.edition_id.clone(),
            content_revision: hebrew.content_revision.clone(),
            joiner: String::new(),
            segments: vec![Segment {
                segment_id: segment.segment_id.clone(),
                text: segment.text.clone(),
                tokens: segment
                    .tokens
                    .iter()
                    .map(|t| TokenSpan {
                        id: TokenId::parse(&t.id).unwrap(),
                        range: ScalarRange {
                            start: t.start,
                            end: t.end,
                        },
                    })
                    .collect(),
            }],
        };
        let paragraph = serde_wasm_bindgen::to_value(&paragraph).unwrap();
        let (start, end) = (selection.utf16.start, selection.utf16.end);

        let anchors = selection_to_anchor(paragraph, start, end).unwrap();
        let anchors: Vec<Anchor> = serde_wasm_bindgen::from_value(anchors).unwrap();
        let [anchor] = anchors.as_slice() else {
            panic!("{}: expected one anchor, got {anchors:?}", selection.name);
        };
        assert_eq!(anchor.range, selection.scalar.into(), "{}", selection.name);
        assert_eq!(
            anchor.quote.as_ref().map(|q| q.exact.as_str()),
            Some(selection.exact.as_str()),
            "{}",
            selection.name
        );
        // The JS string slice a text view would show for the same indexes.
        let text = js_sys::JsString::from(segment.text.as_str());
        assert_eq!(
            String::from(text.slice(start, end)),
            selection.exact,
            "{}",
            selection.name
        );
        let ids: Vec<String> = anchor
            .tokens
            .iter()
            .map(|id| format_token_id(serde_wasm_bindgen::to_value(id).unwrap()).unwrap())
            .collect();
        assert_eq!(ids, selection.tokens, "{}", selection.name);

        let back =
            anchor_to_utf16(serde_wasm_bindgen::to_value(anchor).unwrap(), &segment.text).unwrap();
        let back: Utf16Range = serde_wasm_bindgen::from_value(back).unwrap();
        assert_eq!(back, Utf16Range { start, end }, "{}", selection.name);
    }
}

#[wasm_bindgen_test]
fn conformance_suite_passes() {
    let report: ConformanceReport =
        serde_wasm_bindgen::from_value(run_conformance().unwrap()).unwrap();
    let failures: Vec<String> = report
        .failures()
        .iter()
        .map(|c| {
            format!(
                "{}/{}: expected {} got {}",
                c.fixture, c.name, c.expected, c.actual
            )
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(report.failed, 0);
    assert!(report.passed > 100, "{}", report.passed);
    assert_eq!(report.api_version, openlogos_core::API_VERSION);
    console_log!(
        "conformance cases: {} passed, {} failed (api {})",
        report.passed,
        report.failed,
        report.api_version
    );
}

/// B070: the shell handle runs a command, undoes it and answers the palette in the worker.
#[wasm_bindgen_test]
fn shell_handle_runs_commands_and_the_palette() {
    let mut shell = openlogos_ffi::wasm::ShellHandle::fixture();
    // A plain object, as a page passes it (serde_wasm_bindgen's default serializer would
    // turn a JSON map into a JS Map, which the surface does not read).
    let invocation = js_sys::JSON::parse(r#"{"command_id": "column.new"}"#).unwrap();
    shell.apply(invocation, JsValue::UNDEFINED).unwrap();
    assert_eq!(shell.revision(), 1.0);
    shell.undo().unwrap();
    // A disabled command throws with the reason in words beside its kind.
    let focus = js_sys::JSON::parse(r#"{"command_id": "tab.focus"}"#).unwrap();
    let error = shell.apply(focus, JsValue::UNDEFINED).unwrap_err();
    assert_eq!(
        property(&error, "reason_text").as_string().as_deref(),
        Some("Choose a tab")
    );
    let result: serde_json::Value =
        serde_wasm_bindgen::from_value(shell.palette_query("Jn 3:16", JsValue::UNDEFINED).unwrap())
            .unwrap();
    assert_eq!(result["groups"][0]["section"], "passage");
    console_log!("palette: {}", result["groups"][0]["items"][0]["title"]);
}

/// B076: the tool well follows focus; the fixture handle registers the edition languages.
#[wasm_bindgen_test]
fn shell_handle_returns_the_tool_well() {
    let mut shell = openlogos_ffi::wasm::ShellHandle::fixture();
    let well: serde_json::Value =
        serde_wasm_bindgen::from_value(shell.tool_well(JsValue::UNDEFINED).unwrap()).unwrap();
    assert_eq!(well["context"], "translation");
    assert_eq!(well["groups"][1]["kind"], "ink");
    let dock = js_sys::JSON::parse(r#"{"focus": "notes_dock", "has_selection": true}"#).unwrap();
    let notes: serde_json::Value =
        serde_wasm_bindgen::from_value(shell.tool_well(dock.clone()).unwrap()).unwrap();
    assert_eq!(notes["effective_tool"], "type");
    assert_eq!(notes["groups"][0]["items"][1]["key"], "Cmd+B");
    assert_eq!(notes["groups"][1]["kind"], "selection");
    let pen = js_sys::JSON::parse(r#"{"command_id": "tool.pen"}"#).unwrap();
    let error = shell.apply(pen, dock).unwrap_err();
    assert_eq!(
        property(&error, "reason_text").as_string().as_deref(),
        Some("Pen works on Bible text")
    );
    let focus =
        js_sys::JSON::parse(r#"{"command_id": "column.focus", "args": {"column": 2}}"#).unwrap();
    shell.apply(focus, JsValue::UNDEFINED).unwrap();
    let view: serde_json::Value = serde_wasm_bindgen::from_value(shell.view().unwrap()).unwrap();
    assert_eq!(view["tool_context"], "original");
}

/// B077: the context menu for a word in another column, its focus change and a row.
#[wasm_bindgen_test]
fn shell_handle_returns_the_context_menu() {
    let mut shell = openlogos_ffi::wasm::ShellHandle::fixture();
    let word = js_sys::JSON::parse(
        r#"{"kind": "word", "text": "ἠγάπησεν", "reference": "John.3.16", "tab_id": "t3"}"#,
    )
    .unwrap();
    let menu: serde_json::Value =
        serde_wasm_bindgen::from_value(shell.context_menu(word, JsValue::UNDEFINED).unwrap())
            .unwrap();
    assert_eq!(menu["target_kind"], "word");
    assert_eq!(menu["focus"]["args"]["tab_id"], "t3");
    let compare = &menu["sections"][1]["items"][0];
    assert_eq!(compare["label"], "Compare in bsb");
    let context = serde_wasm_bindgen::to_value(&menu["context"]).unwrap();
    let focus = js_sys::JSON::parse(&menu["focus"].to_string()).unwrap();
    shell.apply(focus, context.clone()).unwrap();
    let row = js_sys::JSON::parse(
        &serde_json::json!({"command_id": compare["command_id"], "args": compare["args"]})
            .to_string(),
    )
    .unwrap();
    let outcome: serde_json::Value =
        serde_wasm_bindgen::from_value(shell.apply(row, context).unwrap()).unwrap();
    assert_eq!(outcome["recorded"], true);
    let gone = js_sys::JSON::parse(r#"{"kind": "tab", "tab_id": "t9"}"#).unwrap();
    let error = shell.context_menu(gone, JsValue::UNDEFINED).unwrap_err();
    assert_eq!(
        property(&error, "kind").as_string().as_deref(),
        Some("no_such_tab")
    );
}

/// B078: the placeholder library streams across books, a fixture pack is read through the
/// browser's fetch-and-supply path, and the shell follows a scroll through versification.
#[wasm_bindgen_test]
fn reading_handle_streams_chapters_and_the_shell_follows_scrolling() {
    let reading = openlogos_ffi::wasm::ReadingHandle::placeholder();
    let location: serde_json::Value =
        serde_wasm_bindgen::from_value(reading.locate("bsb", "Jn 3:16-18").unwrap()).unwrap();
    assert_eq!(location["highlight_range"], "John.3.16-John.3.18");
    let neighbors: serde_json::Value =
        serde_wasm_bindgen::from_value(reading.chapter_neighbors("bsb", "Mal.4").unwrap()).unwrap();
    assert_eq!(neighbors["next"], "Matt.1");
    let chapter: serde_json::Value =
        serde_wasm_bindgen::from_value(reading.chapter_content("wlc", "Gen.1").unwrap()).unwrap();
    assert_eq!(chapter["verses"][0]["placeholder"], false);
    assert_eq!(chapter["verses"][2]["text"], "טקסט ממלא מקום — 1:3");
    let error = reading.locate("wlc", "John 3:16").unwrap_err();
    assert_eq!(
        property(&error, "kind").as_string().as_deref(),
        Some("book_not_in_edition")
    );
    assert_eq!(
        openlogos_ffi::wasm::reference_for_segment("Ps.3.0").as_deref(),
        Some("Ps.3.0")
    );

    let mut browser = openlogos_ffi::wasm::ReadingHandle::new();
    let files = openlogos_conformance::FIXTURE_PACK_FILES;
    let file = |path: &str| files.iter().find(|(p, _)| *p == path).unwrap().1;
    browser
        .add_pack_manifest(std::str::from_utf8(file("wlc/manifest.json")).unwrap())
        .unwrap();
    let chunk: serde_json::Value =
        serde_wasm_bindgen::from_value(browser.chunk_for("wlc", "Gen.1").unwrap()).unwrap();
    assert_eq!(chunk["path"], "chapters.bin");
    let not_loaded = browser.chapter_content("wlc", "Gen.1").unwrap_err();
    assert_eq!(
        property(&not_loaded, "kind").as_string().as_deref(),
        Some("chunk_not_loaded")
    );
    let start = chunk["offset"].as_u64().unwrap() as usize;
    let end = start + chunk["length"].as_u64().unwrap() as usize;
    browser
        .supply_chunk("wlc", "Gen.1", &file("wlc/chapters.bin")[start..end])
        .unwrap();
    let gen1: serde_json::Value =
        serde_wasm_bindgen::from_value(browser.chapter_content("wlc", "Gen.1").unwrap()).unwrap();
    assert_eq!(gen1["verses"].as_array().unwrap().len(), 2);

    let mut shell = openlogos_ffi::wasm::ShellHandle::fixture();
    shell
        .set_resource_schemes(reading.schemes().unwrap())
        .unwrap();
    let scroll = js_sys::JSON::parse(
        r#"{"command_id": "tab.record_scroll", "args": {"tab_id": "t1", "segment_id": "John.3.17", "scalar": 0}}"#,
    )
    .unwrap();
    let outcome: serde_json::Value =
        serde_wasm_bindgen::from_value(shell.apply(scroll, JsValue::UNDEFINED).unwrap()).unwrap();
    let positions = outcome["effects"]["positions"].as_array().unwrap();
    assert_eq!(positions[0]["reference_label"], "Jn 3:17");
    assert_eq!(positions[1]["tab_id"], "t3");
    assert_eq!(outcome["recorded"], false);
}

/// B074: a session worked on, a note, a highlight (one refused on placeholder text) and a
/// stroke saved as the user store and restored into a fresh shell; a broken file throws.
#[wasm_bindgen_test]
fn shell_handle_saves_and_restores_the_user_store() {
    let reading = openlogos_ffi::wasm::ReadingHandle::placeholder();
    let mut shell = openlogos_ffi::wasm::ShellHandle::fixture();
    let parse = |text: &str| js_sys::JSON::parse(text).unwrap();
    shell
        .apply(
            parse(
                r#"{"command_id": "tab.move", "args": {"tab_id": "t2", "column": 2, "index": 0}}"#,
            ),
            JsValue::UNDEFINED,
        )
        .unwrap();
    shell
        .apply(
            parse(r#"{"command_id": "tab.record_scroll", "args": {"tab_id": "t3", "segment_id": "John.3.17", "scalar": 4}}"#),
            JsValue::UNDEFINED,
        )
        .unwrap();
    shell
        .put_note(parse(
            r#"{"id": "fixture:note:1", "blocks": [{"kind": "heading", "spans": [{"text": "See "}, {"text": "Jn 3:16", "link": "John.3.16"}]}]}"#,
        ))
        .unwrap();
    let refused = shell
        .add_highlight(
            parse(r##"{"resource_id": "bsb", "content_revision": "bsb-2024", "spans": [{"segment_id": "John.3.2", "start": 0, "end": 4}], "color": "#F5D90A"}"##),
            &reading,
        )
        .unwrap_err();
    assert_eq!(
        property(&refused, "kind").as_string().as_deref(),
        Some("placeholder_text")
    );
    let id = shell
        .add_highlight(
            parse(r##"{"resource_id": "bsb", "content_revision": "bsb-2024", "spans": [{"segment_id": "John.3.16", "start": 0, "end": 4}], "color": "#f5d90a"}"##),
            &reading,
        )
        .unwrap();
    assert_eq!(id, "h1");
    let ink = shell
        .add_ink(parse(
            r##"{"resource_id": "bsb", "segment_id": "John.3.16", "stroke": {"id": "", "tool": "pen", "brush_version": "pf-1.2.3/pen-1", "width": 4, "color": "#000000", "opacity": 1, "points": [{"x": 0, "y": 0, "pressure": null, "elapsed_ms": 0}, {"x": 9, "y": 3, "pressure": 0.5, "elapsed_ms": 12}]}}"##,
        ))
        .unwrap();
    assert_eq!(ink, "k2");
    assert_eq!(shell.change_count(), 5.0);
    let saved = shell.export_store();

    let mut restored = openlogos_ffi::wasm::ShellHandle::fixture();
    let report: serde_json::Value =
        serde_wasm_bindgen::from_value(restored.import_store(&saved).unwrap()).unwrap();
    assert_eq!(report["issues"], serde_json::json!([]));
    assert_eq!(
        (&report["notes"], &report["highlights"], &report["ink"]),
        (
            &serde_json::json!(1),
            &serde_json::json!(1),
            &serde_json::json!(1)
        )
    );
    assert_eq!(restored.export_store(), saved);
    let view: serde_json::Value = serde_wasm_bindgen::from_value(restored.view().unwrap()).unwrap();
    let column = &view["workspaces"][0]["columns"][1];
    assert_eq!(column["tabs"][0]["id"], "t2");
    assert_eq!(column["tabs"][1]["scroll_segment_id"], "John.3.17");
    let highlights: serde_json::Value =
        serde_wasm_bindgen::from_value(restored.highlights_in("bsb", "John.3").unwrap()).unwrap();
    assert_eq!(highlights[0]["color"], "#F5D90A");
    let broken = restored.import_store("{\"format\": ").unwrap_err();
    assert_eq!(
        property(&broken, "kind").as_string().as_deref(),
        Some("not_json")
    );
}

/// B064: with `--features retrieval`, SQLite with FTS5 and the `openlogos` tokenizer run
/// inside the worker's module and answer an unaccented query against accented text.
#[cfg(feature = "retrieval")]
#[wasm_bindgen_test]
fn retrieval_engine_runs_fts5_with_the_shared_tokenizer() {
    let summary = openlogos_ffi::wasm::retrieval::retrieval_probe().unwrap();
    console_log!("{summary}");
    // sqlite-wasm-rs 0.5.5 bundles SQLite 3.53.0; specs/19 wants 3.54+ for OPFS (see B064).
    assert!(summary.starts_with("sqlite 3.5"), "{summary}");
    assert!(
        summary.contains("greek_hits 1 hebrew_hits 1 mode ol-norm-1"),
        "{summary}"
    );
}

/// B071: bytes that are no search pack are refused, and the fixture provider stays in place.
#[cfg(feature = "retrieval")]
#[wasm_bindgen_test]
fn edition_search_refuses_bytes_that_are_no_pack() {
    let mut shell = openlogos_ffi::wasm::ShellHandle::fixture();
    let garbage = js_sys::Uint8Array::from(&b"not a database"[..]);
    assert!(shell.use_edition_search(vec![garbage]).is_err());
    let answer = shell.palette_query("loved", JsValue::UNDEFINED).unwrap();
    assert!(
        js_sys::JSON::stringify(&answer)
            .unwrap()
            .as_string()
            .unwrap()
            .contains("John"),
    );
}

/// B071: keystroke to text results in this worker, against the 50 ms of specs/22 §7, over the
/// real search packs. They are corpus-derived and never in the repository, so
/// `just preview-pack-bench-wasm` serves them and names the URL when it builds this test;
/// without it the test says so and passes.
#[cfg(feature = "retrieval")]
#[wasm_bindgen_test]
async fn edition_search_latency_over_the_preview_pack() {
    let Some(base) = option_env!("OPENLOGOS_SEARCH_BENCH_URL") else {
        console_log!(
            "SKIPPED edition_search_latency: OPENLOGOS_SEARCH_BENCH_URL is not set (just preview-pack-bench-wasm)"
        );
        return;
    };
    let global = js_sys::global();
    let performance = property(&global, "performance");
    let now = js_sys::Function::from(property(&performance, "now"));
    let clock = || now.call0(&performance).unwrap().as_f64().unwrap();

    let started = clock();
    let fetch = js_sys::Function::from(property(&global, "fetch"));
    let mut packs = Vec::new();
    let mut bytes = 0;
    for name in ["bsb", "sblgnt", "wlc"] {
        let url = format!("{base}/{name}.sqlite");
        let response = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(
            fetch.call1(&global, &JsValue::from_str(&url)).unwrap(),
        ))
        .await
        .unwrap();
        let read = js_sys::Function::from(property(&response, "arrayBuffer"));
        let buffer = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(
            read.call0(&response).unwrap(),
        ))
        .await
        .unwrap();
        let pack = js_sys::Uint8Array::new(&buffer);
        bytes += pack.length();
        packs.push(pack);
    }
    let fetched = clock();
    let mut shell = openlogos_ffi::wasm::ShellHandle::fixture();
    shell.use_edition_search(packs).unwrap();
    let attached = clock();

    let list: serde_json::Value =
        serde_json::from_str(include_str!("../../preview-pack/bench-queries.json")).unwrap();
    let mut strokes = Vec::new();
    for script in ["en", "grc", "hbo"] {
        for query in list[script].as_array().unwrap() {
            let chars: Vec<char> = query.as_str().unwrap().chars().collect();
            for n in 2.min(chars.len())..=chars.len() {
                strokes.push(chars[..n].iter().collect::<String>());
            }
        }
    }
    let mut samples = Vec::new();
    let mut with_hits = 0;
    for _round in 0..3 {
        for stroke in &strokes {
            let start = clock();
            let answer = shell.palette_query(stroke, JsValue::UNDEFINED).unwrap();
            samples.push((clock() - start, stroke.clone()));
            let groups = js_sys::JSON::stringify(&answer)
                .unwrap()
                .as_string()
                .unwrap();
            if groups.contains("\"section\":\"text\"") {
                with_hits += 1;
            }
        }
    }
    samples.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let rank = |p: f64| samples[((p / 100.0 * samples.len() as f64).ceil() as usize).max(1) - 1].0;
    let (p50, p95) = (rank(50.0), rank(95.0));
    let (max, slowest) = samples.last().unwrap();
    console_log!(
        "B071 wasm: {} samples, {with_hits} with text hits; palette with text results p50 {p50:.2} ms, p95 {p95:.2} ms, max {max:.2} ms ({slowest}); {bytes} pack bytes fetched in {:.0} ms, attached in {:.0} ms",
        samples.len(),
        fetched - started,
        attached - fetched,
    );
    assert!(samples.len() >= 200);
    assert!(with_hits > 200, "{with_hits}");
}

/// B026: bytes that are no morphology layer are refused, and with none installed a token's
/// analysis is absent with its reason.
#[cfg(feature = "retrieval")]
#[wasm_bindgen_test]
fn morphology_refuses_bytes_that_are_no_layer() {
    let mut shell = openlogos_ffi::wasm::ShellHandle::fixture();
    let garbage = js_sys::Uint8Array::from(&b"not a database"[..]);
    assert!(shell.use_morphology(vec![garbage]).is_err());
    let answer = shell.token_analysis("sblgnt:43003016004").unwrap();
    let text = js_sys::JSON::stringify(&answer)
        .unwrap()
        .as_string()
        .unwrap();
    assert!(text.contains("\"absent_reason\":\"no_layer\""), "{text}");
}

/// B026: token lookups and lemma searches in this worker over the real layers, against the
/// 50 ms of specs/22 §7, replaying the samples the native bench wrote. The layers are
/// corpus-derived and never in the repository, so `just preview-pack-morphology-bench-wasm`
/// serves them and names the URL when it builds this test; without it the test says so.
#[cfg(feature = "retrieval")]
#[wasm_bindgen_test]
async fn morphology_latency_over_the_layers() {
    let Some(base) = option_env!("OPENLOGOS_MORPHOLOGY_BENCH_URL") else {
        console_log!(
            "SKIPPED morphology_latency: OPENLOGOS_MORPHOLOGY_BENCH_URL is not set (just preview-pack-morphology-bench-wasm)"
        );
        return;
    };
    let global = js_sys::global();
    let performance = property(&global, "performance");
    let now = js_sys::Function::from(property(&performance, "now"));
    let clock = || now.call0(&performance).unwrap().as_f64().unwrap();
    let fetch = js_sys::Function::from(property(&global, "fetch"));
    let get = |path: String| {
        let fetch = fetch.clone();
        let global = global.clone();
        async move {
            let response = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(
                fetch.call1(&global, &JsValue::from_str(&path)).unwrap(),
            ))
            .await
            .unwrap();
            let read = js_sys::Function::from(property(&response, "arrayBuffer"));
            let buffer = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(
                read.call0(&response).unwrap(),
            ))
            .await
            .unwrap();
            js_sys::Uint8Array::new(&buffer)
        }
    };
    let started = clock();
    let mut layers = Vec::new();
    let mut bytes = 0;
    for name in ["sblgnt.macula-greek", "wlc.macula-hebrew"] {
        let layer = get(format!("{base}/layers/{name}.sqlite")).await;
        bytes += layer.length();
        layers.push(layer);
    }
    let samples: serde_json::Value =
        serde_json::from_slice(&get(format!("{base}/bench-samples.json")).await.to_vec()).unwrap();
    let fetched = clock();
    let mut shell = openlogos_ffi::wasm::ShellHandle::fixture();
    shell.use_morphology(layers).unwrap();
    let attached = clock();

    let tokens: Vec<&str> = samples["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect();
    let lemmas: Vec<(String, String, String)> =
        serde_json::from_value(samples["lemmas"].clone()).unwrap();
    let mut lookups = Vec::new();
    let mut searches = Vec::new();
    for _round in 0..3 {
        for t in &tokens {
            let start = clock();
            shell.token_analysis(t).unwrap();
            lookups.push((clock() - start, (*t).to_owned()));
        }
        for (edition, lemma, key) in &lemmas {
            for typed in [key, lemma] {
                let start = clock();
                shell
                    .lemma_search(vec![edition.clone()], typed, 50)
                    .unwrap();
                searches.push((clock() - start, typed.clone()));
            }
        }
    }
    let summary = |samples: &mut Vec<(f64, String)>| {
        samples.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let rank =
            |p: f64| samples[((p / 100.0 * samples.len() as f64).ceil() as usize).max(1) - 1].0;
        let (max, slowest) = samples.last().unwrap();
        format!(
            "{} samples, p50 {:.3} ms, p95 {:.3} ms, max {max:.3} ms ({slowest})",
            samples.len(),
            rank(50.0),
            rank(95.0)
        )
    };
    console_log!(
        "B026 wasm: token lookup {}; lemma search {}; {bytes} layer bytes fetched in {:.0} ms, attached in {:.0} ms",
        summary(&mut lookups),
        summary(&mut searches),
        fetched - started,
        attached - fetched,
    );
    assert!(lookups.len() >= 200 && searches.len() >= 200);
}
