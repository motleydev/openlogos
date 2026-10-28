//! The portable ink record: `schemas/ink-document.schema.json` as Rust types with serde names
//! equal to the schema's, so `serde_json::to_string(&doc)` is a schema-conformant document
//! and `examples/ink-document.json` deserialises into [`InkDocument`]. The rules the schema
//! cannot express (unique stroke ids, pressure range, monotonic time, a tombstone naming a
//! stroke) are [`InkDocument::validate`].
//!
//! Sensor data the input device did not report is `None` (JSON `null`), never a default:
//! specs/13 "Unsupported sensor data is null, not fabricated". The brush that rendered a
//! stroke is named by `brush_version`, so geometry is re-rendered only with that brush
//! ([`crate::brush::Brush::for_version`]); an unknown brush version is shown from the stored
//! points by a fallback the renderer labels, never silently re-shaped.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// The schema version this crate writes. 1.1 (B039) added optional per-sample tilt; a 1.0
/// document reads with `None` tilt (ADR 0003 `older_minor`) and is written back as 1.1.
pub const SCHEMA_VERSION: &str = "1.1";
/// The versions this crate reads.
pub const READABLE_SCHEMA_VERSIONS: [&str; 2] = ["1.0", "1.1"];

/// The drawing tool of a stroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    /// An opaque pressure-thinned line.
    Pen,
    /// A translucent constant-width band, composited once per stroke.
    Highlighter,
}

/// One sampled input point in the document's immutable coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InkPoint {
    /// Horizontal position in the coordinate space.
    pub x: f64,
    /// Vertical position in the coordinate space.
    pub y: f64,
    /// Normalised pressure `0..=1`, or `None` when the device reports none (mouse, finger
    /// without force, a pointer event whose pressure is the 0.5 placeholder).
    pub pressure: Option<f64>,
    /// Milliseconds since the stroke's first point.
    pub elapsed_ms: f64,
    /// 1.1: the stylus's angle above the surface in radians, `0..=π/2`; `None` when the device
    /// reports none (absent in a 1.0 file).
    #[serde(default)]
    pub altitude: Option<f64>,
    /// 1.1: the direction the stylus points in radians, `0..=2π`; `None` when unreported.
    #[serde(default)]
    pub azimuth: Option<f64>,
}

/// One stroke: the tool and brush that made it, its paint, and the points as sampled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    /// Stable id, unique within the document; tombstones and merges refer to it.
    pub id: String,
    /// The tool.
    pub tool: Tool,
    /// The brush definition and version that rendered this stroke when it was made.
    pub brush_version: String,
    /// Base width (diameter) in coordinate-space units.
    pub width: f64,
    /// CSS-style colour, e.g. `#FFE066`.
    pub color: String,
    /// Opacity `0..=1`; a highlighter's whole composite is drawn at this value.
    pub opacity: f64,
    /// At least two points.
    pub points: Vec<InkPoint>,
}

/// The affine viewport over the snapshot: applied equally to background and strokes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewportTransform {
    /// Zoom factor, positive.
    pub scale: f64,
    /// Horizontal pan.
    pub translate_x: f64,
    /// Vertical pan.
    pub translate_y: f64,
}

impl Default for ViewportTransform {
    fn default() -> Self {
        Self {
            scale: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
        }
    }
}

/// An ink document over one reading snapshot's coordinate space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InkDocument {
    /// Always [`SCHEMA_VERSION`].
    pub schema_version: String,
    /// `true` for fixture data that must not be mistaken for a user's note.
    pub fixture_only: bool,
    /// The document's stable id.
    pub id: String,
    /// The `ReadingSnapshot` this ink lies over.
    pub snapshot_id: String,
    /// The snapshot's immutable coordinate space; must equal the snapshot's.
    pub coordinate_space_id: String,
    /// Strokes in z-order (first drawn at the bottom), tombstoned ones included.
    pub strokes: Vec<Stroke>,
    /// Ids of strokes erased with the stroke eraser; kept for conflict-safe merges.
    pub deleted_stroke_ids: Vec<String>,
    /// The viewport.
    pub viewport_transform: ViewportTransform,
}

/// Why a document is not a valid ink record.
#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InkError {
    /// `schema_version` is not the one this crate writes.
    #[error("schema_version is `{found}`, expected `{expected}`")]
    SchemaVersion {
        /// What the document says.
        found: String,
        /// What it must say.
        expected: String,
    },
    /// An id or string field that must be non-empty is empty.
    #[error("{field} must not be empty")]
    Empty {
        /// The field.
        field: String,
    },
    /// Two strokes share an id.
    #[error("stroke id `{id}` is used twice")]
    DuplicateStrokeId {
        /// The id.
        id: String,
    },
    /// A stroke has fewer than two points.
    #[error("stroke `{id}` has {count} points; at least 2 are required")]
    TooFewPoints {
        /// The stroke.
        id: String,
        /// How many points it has.
        count: usize,
    },
    /// A numeric field is outside its range (width, opacity, pressure, scale, elapsed time).
    #[error("stroke `{id}`: {field} = {value} is out of range")]
    OutOfRange {
        /// The stroke, or `document` for the viewport.
        id: String,
        /// The field.
        field: String,
        /// The offending value.
        value: f64,
    },
    /// A point's elapsed time is earlier than the previous point's.
    #[error("stroke `{id}`: elapsed_ms goes backwards at point {index}")]
    TimeNotMonotonic {
        /// The stroke.
        id: String,
        /// The point index.
        index: usize,
    },
    /// A tombstone names a stroke the document does not contain.
    #[error("deleted_stroke_ids names unknown stroke `{id}`")]
    UnknownDeletedStroke {
        /// The id.
        id: String,
    },
}

impl InkDocument {
    /// A new, empty document over a snapshot's coordinate space.
    #[must_use]
    pub fn new(id: &str, snapshot_id: &str, coordinate_space_id: &str) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_owned(),
            fixture_only: false,
            id: id.to_owned(),
            snapshot_id: snapshot_id.to_owned(),
            coordinate_space_id: coordinate_space_id.to_owned(),
            strokes: Vec::new(),
            deleted_stroke_ids: Vec::new(),
            viewport_transform: ViewportTransform::default(),
        }
    }

    /// The strokes that are shown, in z-order: every stroke not named in
    /// `deleted_stroke_ids`.
    pub fn live_strokes(&self) -> impl Iterator<Item = &Stroke> {
        let deleted: HashSet<&str> = self.deleted_stroke_ids.iter().map(String::as_str).collect();
        self.strokes
            .iter()
            .filter(move |s| !deleted.contains(s.id.as_str()))
    }

    /// Checks the rules the JSON schema states and the ones it cannot (see the module docs).
    ///
    /// # Errors
    /// The first rule the document breaks.
    pub fn validate(&self) -> Result<(), InkError> {
        if !READABLE_SCHEMA_VERSIONS.contains(&self.schema_version.as_str()) {
            return Err(InkError::SchemaVersion {
                found: self.schema_version.clone(),
                expected: SCHEMA_VERSION.to_owned(),
            });
        }
        for (field, value) in [
            ("id", &self.id),
            ("snapshot_id", &self.snapshot_id),
            ("coordinate_space_id", &self.coordinate_space_id),
        ] {
            if value.is_empty() {
                return Err(InkError::Empty {
                    field: field.to_owned(),
                });
            }
        }
        if self.viewport_transform.scale.is_nan() || self.viewport_transform.scale <= 0.0 {
            return Err(InkError::OutOfRange {
                id: "document".to_owned(),
                field: "viewport_transform.scale".to_owned(),
                value: self.viewport_transform.scale,
            });
        }
        let mut ids = HashSet::new();
        for stroke in &self.strokes {
            stroke.validate()?;
            if !ids.insert(stroke.id.as_str()) {
                return Err(InkError::DuplicateStrokeId {
                    id: stroke.id.clone(),
                });
            }
        }
        for id in &self.deleted_stroke_ids {
            if !ids.contains(id.as_str()) {
                return Err(InkError::UnknownDeletedStroke { id: id.clone() });
            }
        }
        Ok(())
    }

    /// The document as schema-conformant JSON at [`SCHEMA_VERSION`], after [`Self::validate`]
    /// (a 1.0 document comes out as 1.1 with `null` tilt: the reader fills the defaults).
    ///
    /// # Errors
    /// A validation error; serialisation itself cannot fail for these types.
    pub fn to_json(&self) -> Result<String, InkError> {
        self.validate()?;
        let mut written = self.clone();
        written.schema_version = SCHEMA_VERSION.to_owned();
        Ok(serde_json::to_string_pretty(&written).unwrap_or_default())
    }
}

impl Stroke {
    fn validate(&self) -> Result<(), InkError> {
        let out_of_range = |field: &str, value: f64| InkError::OutOfRange {
            id: self.id.clone(),
            field: field.to_owned(),
            value,
        };
        for (field, value) in [
            ("id", &self.id),
            ("brush_version", &self.brush_version),
            ("color", &self.color),
        ] {
            if value.is_empty() {
                return Err(InkError::Empty {
                    field: format!("stroke.{field}"),
                });
            }
        }
        if self.width.is_nan() || self.width <= 0.0 {
            return Err(out_of_range("width", self.width));
        }
        if !(0.0..=1.0).contains(&self.opacity) {
            return Err(out_of_range("opacity", self.opacity));
        }
        if self.points.len() < 2 {
            return Err(InkError::TooFewPoints {
                id: self.id.clone(),
                count: self.points.len(),
            });
        }
        let mut last_time = 0.0;
        for (index, point) in self.points.iter().enumerate() {
            if let Some(p) = point.pressure
                && !(0.0..=1.0).contains(&p)
            {
                return Err(out_of_range("pressure", p));
            }
            if let Some(a) = point.altitude
                && !(0.0..=std::f64::consts::FRAC_PI_2).contains(&a)
            {
                return Err(out_of_range("altitude", a));
            }
            if let Some(a) = point.azimuth
                && !(0.0..=std::f64::consts::TAU).contains(&a)
            {
                return Err(out_of_range("azimuth", a));
            }
            if point.elapsed_ms.is_nan() || point.elapsed_ms < 0.0 {
                return Err(out_of_range("elapsed_ms", point.elapsed_ms));
            }
            if point.elapsed_ms < last_time {
                return Err(InkError::TimeNotMonotonic {
                    id: self.id.clone(),
                    index,
                });
            }
            last_time = point.elapsed_ms;
        }
        Ok(())
    }

    /// Whether every point carries a reported pressure (so a pressure brush uses it rather
    /// than simulating one from velocity).
    #[must_use]
    pub fn has_pressure(&self) -> bool {
        !self.points.is_empty() && self.points.iter().all(|p| p.pressure.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = include_str!("../../../examples/ink-document.json");

    fn stroke(id: &str) -> Stroke {
        Stroke {
            id: id.to_owned(),
            tool: Tool::Pen,
            brush_version: "pf-1.2.3/pen-1".to_owned(),
            width: 3.0,
            color: "#000000".to_owned(),
            opacity: 1.0,
            points: vec![
                InkPoint {
                    x: 0.0,
                    y: 0.0,
                    pressure: None,
                    elapsed_ms: 0.0,
                    altitude: None,
                    azimuth: None,
                },
                InkPoint {
                    x: 10.0,
                    y: 4.0,
                    pressure: Some(0.5),
                    elapsed_ms: 16.0,
                    altitude: Some(1.1),
                    azimuth: Some(0.4),
                },
            ],
        }
    }

    #[test]
    fn the_example_fixture_round_trips_through_the_types() {
        let doc: InkDocument = serde_json::from_str(EXAMPLE).expect("example parses");
        assert_eq!(doc.schema_version, SCHEMA_VERSION);
        assert!(doc.fixture_only);
        assert_eq!(doc.strokes[0].tool, Tool::Highlighter);
        assert_eq!(doc.strokes[0].points[0].pressure, Some(0.5));
        assert_eq!(doc.strokes[0].points[0].altitude, None);
        assert_eq!(doc.strokes[1].points[0].altitude, Some(1.05));
        doc.validate().expect("the example is valid");
        let back: serde_json::Value = serde_json::from_str(&doc.to_json().unwrap()).unwrap();
        let original: serde_json::Value = serde_json::from_str(EXAMPLE).unwrap();
        assert_eq!(
            canon(back),
            canon(original),
            "field names and nulls are the schema's"
        );
    }

    /// Integers and floats compare equal (`40` in the fixture, `40.0` from f64).
    fn canon(value: serde_json::Value) -> serde_json::Value {
        use serde_json::Value;
        match value {
            Value::Number(n) => serde_json::json!(n.as_f64().unwrap_or(0.0)),
            Value::Array(a) => Value::Array(a.into_iter().map(canon).collect()),
            Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, canon(v))).collect()),
            other => other,
        }
    }

    const ARCHIVED: &str = include_str!("../../../examples/archive/ink-document-1.0.json");

    #[test]
    fn a_1_0_document_reads_with_null_tilt_and_is_written_as_1_1() {
        let doc: InkDocument = serde_json::from_str(ARCHIVED).expect("1.0 example parses");
        assert_eq!(doc.schema_version, "1.0");
        assert_eq!(doc.strokes[0].points[0].altitude, None);
        doc.validate().expect("1.0 is readable");
        let json = doc.to_json().unwrap();
        assert!(json.contains("\"schema_version\": \"1.1\""));
        assert!(json.contains("\"altitude\": null"));
        let mut steep = doc.clone();
        steep.strokes[0].points[0].altitude = Some(2.0);
        assert!(
            matches!(steep.validate(), Err(InkError::OutOfRange { field, .. }) if field == "altitude")
        );
    }

    #[test]
    fn null_pressure_is_kept_as_null() {
        let mut doc = InkDocument::new("d", "s", "c");
        doc.strokes.push(stroke("a"));
        let json = doc.to_json().unwrap();
        assert!(json.contains("\"pressure\": null"), "{json}");
        assert!(json.contains("\"pressure\": 0.5"));
    }

    #[test]
    fn validation_names_the_broken_rule() {
        let mut doc = InkDocument::new("d", "s", "c");
        doc.strokes.push(stroke("a"));
        doc.strokes.push(stroke("a"));
        assert_eq!(
            doc.validate(),
            Err(InkError::DuplicateStrokeId { id: "a".into() })
        );
        doc.strokes.pop();
        doc.deleted_stroke_ids.push("ghost".into());
        assert_eq!(
            doc.validate(),
            Err(InkError::UnknownDeletedStroke { id: "ghost".into() })
        );
        doc.deleted_stroke_ids.clear();
        doc.strokes[0].points[1].elapsed_ms = -1.0;
        assert!(
            matches!(doc.validate(), Err(InkError::OutOfRange { field, .. }) if field == "elapsed_ms")
        );
        doc.strokes[0].points[1].elapsed_ms = 16.0;
        doc.strokes[0].points[0].elapsed_ms = 20.0;
        assert_eq!(
            doc.validate(),
            Err(InkError::TimeNotMonotonic {
                id: "a".into(),
                index: 1
            })
        );
        doc.strokes[0].points[0].elapsed_ms = 0.0;
        doc.strokes[0].points.pop();
        assert_eq!(
            doc.validate(),
            Err(InkError::TooFewPoints {
                id: "a".into(),
                count: 1
            })
        );
        let mut wrong = InkDocument::new("d", "s", "c");
        wrong.schema_version = "0.9".into();
        assert!(matches!(
            wrong.validate(),
            Err(InkError::SchemaVersion { .. })
        ));
    }

    #[test]
    fn live_strokes_skip_tombstones_and_keep_z_order() {
        let mut doc = InkDocument::new("d", "s", "c");
        doc.strokes.extend([stroke("a"), stroke("b"), stroke("c")]);
        doc.deleted_stroke_ids.push("b".into());
        let live: Vec<_> = doc.live_strokes().map(|s| s.id.as_str()).collect();
        assert_eq!(live, ["a", "c"]);
        doc.validate().unwrap();
    }
}
