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

/// B064: with `--features retrieval`, SQLite with FTS5 and the `openlogos` tokenizer run
/// inside the worker's module and answer an unaccented query against accented text.
#[cfg(feature = "retrieval")]
#[wasm_bindgen_test]
fn retrieval_engine_runs_fts5_with_the_shared_tokenizer() {
    let summary = openlogos_ffi::wasm::retrieval::retrieval_probe().unwrap();
    console_log!("{summary}");
    // sqlite-wasm-rs 0.5.5 bundles SQLite 3.53.0; specs/19 wants 3.54+ for OPFS (see B064).
    assert!(summary.starts_with("sqlite 3.5"), "{summary}");
    assert!(summary.contains("greek_hits 1 hebrew_hits 1 mode ol-norm-1"), "{summary}");
}
