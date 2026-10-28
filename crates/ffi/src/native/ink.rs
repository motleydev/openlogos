//! `inkOutline` and `inkDocumentJson` over `crates/ink` (B039): the perfect-freehand outline
//! for a host's canvas, and the portable `InkDocument` record validated and serialised in the
//! shape of `schemas/ink-document.schema.json`. Two additive functions; nothing existing is
//! renamed. Points cross as flat `f64` triples (`x, y, pressure`, pressure `< 0` meaning the
//! device reported none) and the outline comes back as flat `x, y` pairs, so a 240 Hz stroke
//! costs one buffer each way rather than one record per sample.

use openlogos_ink::{
    Easing, EndOptions, InputPoint, StrokeOptions, Taper, document, freehand::get_stroke,
};

/// How one end of a stroke tapers (`Taper` in `crates/ink`).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum InkTaper {
    /// No taper; the end gets a cap.
    None,
    /// Taper over the whole stroke (`max(size, length)`).
    Full,
    /// Taper over a distance in coordinate units.
    Distance {
        /// The distance.
        value: f64,
    },
}

impl From<InkTaper> for Taper {
    fn from(value: InkTaper) -> Self {
        match value {
            InkTaper::None => Taper::None,
            InkTaper::Full => Taper::Full,
            InkTaper::Distance { value } => Taper::Distance(value),
        }
    }
}

/// The perfect-freehand options (`StrokeOptions` in `crates/ink`, upstream's names).
/// `InkOutlineOptions::pen` / `highlighter` in the host mirror `crates/ink/src/brush.rs`.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct InkOutlineOptions {
    /// Base size (diameter) of the stroke.
    pub size: f64,
    /// Effect of pressure on the size, `0` for none.
    pub thinning: f64,
    /// Edge softening.
    pub smoothing: f64,
    /// Input streamlining.
    pub streamline: f64,
    /// Simulate pressure from velocity (used when the points carry none).
    pub simulate_pressure: bool,
    /// Treat the points as a completed stroke.
    pub last: bool,
    /// Round (`true`) or flat (`false`) start cap.
    pub cap_start: bool,
    /// Round (`true`) or flat (`false`) end cap.
    pub cap_end: bool,
    /// Start taper.
    pub taper_start: InkTaper,
    /// End taper.
    pub taper_end: InkTaper,
}

impl From<InkOutlineOptions> for StrokeOptions {
    fn from(o: InkOutlineOptions) -> Self {
        StrokeOptions {
            size: o.size,
            thinning: o.thinning,
            smoothing: o.smoothing,
            streamline: o.streamline,
            easing: Easing::Linear,
            simulate_pressure: o.simulate_pressure,
            start: EndOptions {
                cap: o.cap_start,
                taper: o.taper_start.into(),
                easing: Easing::EaseOutQuad,
            },
            end: EndOptions {
                cap: o.cap_end,
                taper: o.taper_end.into(),
                easing: Easing::EaseOutCubic,
            },
            last: o.last,
        }
    }
}

/// The outline polygon of a stroke: `points` are `x, y, pressure` triples (pressure below
/// zero means none; a trailing partial triple is ignored); the result is `x, y` pairs of the
/// closed polygon to fill once with the non-zero rule. Empty for no points or a size of zero.
#[uniffi::export]
#[must_use]
pub fn ink_outline(points: Vec<f64>, options: InkOutlineOptions) -> Vec<f64> {
    let input: Vec<InputPoint> = points
        .as_chunks::<3>()
        .0
        .iter()
        .map(|c| InputPoint::new(c[0], c[1], (c[2] >= 0.0).then_some(c[2])))
        .collect();
    get_stroke(&input, &options.into())
        .into_iter()
        .flat_map(|p| [p[0], p[1]])
        .collect()
}

/// The drawing tool of a stroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum InkTool {
    /// An opaque pressure-thinned line.
    Pen,
    /// A translucent band composited once per stroke.
    Highlighter,
}

/// One sampled point; `pressure` is `nil`/`null` when the device reported none.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct InkPoint {
    /// Horizontal position in the document's coordinate space.
    pub x: f64,
    /// Vertical position.
    pub y: f64,
    /// Normalised pressure `0..=1`, or none.
    pub pressure: Option<f64>,
    /// Milliseconds since the stroke's first point.
    pub elapsed_ms: f64,
    /// Stylus altitude in radians (`0..=π/2`), or none (schema 1.1).
    pub altitude: Option<f64>,
    /// Stylus azimuth in radians (`0..=2π`), or none (schema 1.1).
    pub azimuth: Option<f64>,
}

/// One stroke of an ink document (`strokes[]` in the schema).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct InkStroke {
    /// Stable id, unique in the document.
    pub id: String,
    /// The tool.
    pub tool: InkTool,
    /// The brush definition and version, e.g. `pf-1.2.3/pen-1`.
    pub brush_version: String,
    /// Base width in coordinate units.
    pub width: f64,
    /// CSS colour.
    pub color: String,
    /// Opacity of the stroke's single composite.
    pub opacity: f64,
    /// At least two points.
    pub points: Vec<InkPoint>,
}

/// The viewport over the snapshot.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct InkViewportTransform {
    /// Zoom, positive.
    pub scale: f64,
    /// Horizontal pan.
    pub translate_x: f64,
    /// Vertical pan.
    pub translate_y: f64,
}

/// The portable ink record of `schemas/ink-document.schema.json`.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct InkDocument {
    /// Always `1.0`.
    pub schema_version: String,
    /// Fixture data, not a user's note.
    pub fixture_only: bool,
    /// The document id.
    pub id: String,
    /// The reading snapshot.
    pub snapshot_id: String,
    /// The snapshot's immutable coordinate space.
    pub coordinate_space_id: String,
    /// Strokes in z-order, tombstoned ones included.
    pub strokes: Vec<InkStroke>,
    /// Ids of erased strokes.
    pub deleted_stroke_ids: Vec<String>,
    /// The viewport.
    pub viewport_transform: InkViewportTransform,
}

/// Why a document is not a valid ink record (`InkError` in `crates/ink`, flattened).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum InkError {
    /// The document breaks a rule of the schema or of `crates/ink`'s validation.
    #[error("{message}")]
    Invalid {
        /// The serde tag of the rule (`duplicate_stroke_id`, `too_few_points`, ...).
        kind: String,
        /// The rule in words.
        message: String,
    },
}

impl From<document::InkError> for InkError {
    fn from(error: document::InkError) -> Self {
        let kind = serde_json::to_value(&error)
            .ok()
            .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_owned))
            .unwrap_or_default();
        InkError::Invalid {
            kind,
            message: error.to_string(),
        }
    }
}

impl From<InkDocument> for document::InkDocument {
    fn from(d: InkDocument) -> Self {
        document::InkDocument {
            schema_version: d.schema_version,
            fixture_only: d.fixture_only,
            id: d.id,
            snapshot_id: d.snapshot_id,
            coordinate_space_id: d.coordinate_space_id,
            strokes: d
                .strokes
                .into_iter()
                .map(|s| document::Stroke {
                    id: s.id,
                    tool: match s.tool {
                        InkTool::Pen => document::Tool::Pen,
                        InkTool::Highlighter => document::Tool::Highlighter,
                    },
                    brush_version: s.brush_version,
                    width: s.width,
                    color: s.color,
                    opacity: s.opacity,
                    points: s
                        .points
                        .into_iter()
                        .map(|p| document::InkPoint {
                            x: p.x,
                            y: p.y,
                            pressure: p.pressure,
                            elapsed_ms: p.elapsed_ms,
                            altitude: p.altitude,
                            azimuth: p.azimuth,
                        })
                        .collect(),
                })
                .collect(),
            deleted_stroke_ids: d.deleted_stroke_ids,
            viewport_transform: document::ViewportTransform {
                scale: d.viewport_transform.scale,
                translate_x: d.viewport_transform.translate_x,
                translate_y: d.viewport_transform.translate_y,
            },
        }
    }
}

/// Validates an ink document and returns it as schema-conformant JSON (pretty-printed, field
/// names as the schema, `null` for an unreported pressure).
///
/// # Errors
/// The first rule the document breaks.
#[uniffi::export]
pub fn ink_document_json(document: InkDocument) -> Result<String, InkError> {
    let document: document::InkDocument = document.into();
    Ok(document.to_json()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> InkOutlineOptions {
        InkOutlineOptions {
            size: 8.0,
            thinning: 0.5,
            smoothing: 0.5,
            streamline: 0.5,
            simulate_pressure: true,
            last: true,
            cap_start: true,
            cap_end: true,
            taper_start: InkTaper::None,
            taper_end: InkTaper::None,
        }
    }

    #[test]
    fn outline_is_flat_pairs_of_the_core_polygon() {
        let points = vec![
            0.0, 0.0, -1.0, 40.0, 10.0, -1.0, 80.0, 0.0, 0.7, 120.0, 20.0, 0.9,
        ];
        let flat = ink_outline(points, options());
        assert!(
            flat.len() >= 8 && flat.len().is_multiple_of(2),
            "{}",
            flat.len()
        );
        let core = get_stroke(
            &[
                InputPoint::new(0.0, 0.0, None),
                InputPoint::new(40.0, 10.0, None),
                InputPoint::new(80.0, 0.0, Some(0.7)),
                InputPoint::new(120.0, 20.0, Some(0.9)),
            ],
            &options().into(),
        );
        assert_eq!(flat.len(), core.len() * 2);
        assert!(ink_outline(vec![], options()).is_empty());
        assert!(
            ink_outline(vec![1.0, 2.0], options()).is_empty(),
            "a partial triple is no point"
        );
    }

    #[test]
    fn document_json_validates_and_keeps_null_pressure() {
        let point = |x: f64, pressure: Option<f64>, t: f64| InkPoint {
            x,
            y: 10.0,
            pressure,
            elapsed_ms: t,
            altitude: pressure.map(|_| 1.2),
            azimuth: None,
        };
        let mut doc = InkDocument {
            schema_version: "1.1".into(),
            fixture_only: true,
            id: "ink-1".into(),
            snapshot_id: "snap-1".into(),
            coordinate_space_id: "space-1".into(),
            strokes: vec![InkStroke {
                id: "s1".into(),
                tool: InkTool::Pen,
                brush_version: "pf-1.2.3/pen-1".into(),
                width: 3.0,
                color: "#000".into(),
                opacity: 1.0,
                points: vec![point(0.0, None, 0.0), point(5.0, Some(0.4), 8.0)],
            }],
            deleted_stroke_ids: vec![],
            viewport_transform: InkViewportTransform {
                scale: 1.0,
                translate_x: 0.0,
                translate_y: 0.0,
            },
        };
        let json = ink_document_json(doc.clone()).unwrap();
        assert!(json.contains("\"pressure\": null"));
        assert!(json.contains("\"altitude\": 1.2") && json.contains("\"azimuth\": null"));
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["strokes"][0]["tool"], "pen");
        doc.deleted_stroke_ids.push("nope".into());
        let error = ink_document_json(doc).unwrap_err();
        assert_eq!(
            error,
            InkError::Invalid {
                kind: "unknown_deleted_stroke".into(),
                message: "deleted_stroke_ids names unknown stroke `nope`".into()
            }
        );
    }
}
