//! Brush definitions by version, and the compositing rule for rendering a document.
//!
//! A stroke names the brush that made it (`brush_version`); the renderer looks the brush up
//! here and re-derives the same outline from the stored points. A brush version is never
//! edited in place: a change to the geometry is a new version, so an older note keeps its
//! shape (specs/13 "Store completed stroke geometry so brush-version changes do not silently
//! alter older notes"). A stroke whose brush this build does not know is rendered by
//! [`Brush::fallback`] and the sheet's metadata must say so ("Shown from portable strokes").
//!
//! The compositing rule (specs/13 "A drawn highlighter composites once per stroke to avoid
//! darker self-overlaps; independent strokes can accumulate intentionally"): every stroke is
//! one fill of its whole outline polygon with the non-zero winding rule at the stroke's
//! opacity. Where the polygon overlaps itself (a loop, a sharp-corner cap) the non-zero rule
//! paints it once, so a highlighter never darkens over itself; two strokes are two fills and
//! do accumulate. A renderer that paints per segment, or strokes the centre line with a wide
//! pen, breaks the rule and must not be used for highlighter strokes.

use crate::document::{InkDocument, Stroke, Tool};
use crate::freehand::{Easing, EndOptions, InputPoint, StrokeOptions, Taper, Vec2, get_stroke};

/// The perfect-freehand release the brush geometry is ported from; part of every brush id.
pub const ENGINE_VERSION: &str = "pf-1.2.3";

/// A named, versioned brush: the outline options a tool used.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Brush {
    /// The id a stroke stores as `brush_version`, e.g. `pf-1.2.3/pen-1`.
    pub version: &'static str,
    /// The tool this brush is the current version for, if any.
    pub tool: Tool,
    /// The outline options; `size` is replaced by the stroke's width.
    pub options: StrokeOptions,
    /// Whether real pressure thins the stroke (pen) or is ignored (highlighter).
    pub uses_pressure: bool,
}

/// `pf-1.2.3/pen-1`: a pressure-thinned pen with round caps.
pub const PEN_1: Brush = Brush {
    version: "pf-1.2.3/pen-1",
    tool: Tool::Pen,
    options: StrokeOptions {
        size: 4.0,
        thinning: 0.5,
        smoothing: 0.5,
        streamline: 0.5,
        easing: Easing::Linear,
        simulate_pressure: true,
        start: EndOptions {
            cap: true,
            taper: Taper::None,
            easing: Easing::EaseOutQuad,
        },
        end: EndOptions {
            cap: true,
            taper: Taper::None,
            easing: Easing::EaseOutCubic,
        },
        last: true,
    },
    uses_pressure: true,
};

/// `pf-1.2.3/highlighter-1`: a constant-width band with flat caps, no thinning.
pub const HIGHLIGHTER_1: Brush = Brush {
    version: "pf-1.2.3/highlighter-1",
    tool: Tool::Highlighter,
    options: StrokeOptions {
        size: 12.0,
        thinning: 0.0,
        smoothing: 0.5,
        streamline: 0.6,
        easing: Easing::Linear,
        simulate_pressure: false,
        start: EndOptions {
            cap: false,
            taper: Taper::None,
            easing: Easing::EaseOutQuad,
        },
        end: EndOptions {
            cap: false,
            taper: Taper::None,
            easing: Easing::EaseOutCubic,
        },
        last: true,
    },
    uses_pressure: false,
};

/// Every brush this build knows.
pub const BRUSHES: [Brush; 2] = [PEN_1, HIGHLIGHTER_1];

impl Brush {
    /// The brush a stroke names, if this build has it.
    #[must_use]
    pub fn for_version(version: &str) -> Option<Brush> {
        BRUSHES.iter().copied().find(|b| b.version == version)
    }

    /// The current brush for a tool (what a new stroke records).
    #[must_use]
    pub fn current(tool: Tool) -> Brush {
        match tool {
            Tool::Pen => PEN_1,
            Tool::Highlighter => HIGHLIGHTER_1,
        }
    }

    /// The brush used to show a stroke whose `brush_version` is unknown: the current brush
    /// of its tool. The caller labels the result; it is not the stroke's geometry.
    #[must_use]
    pub fn fallback(tool: Tool) -> Brush {
        Self::current(tool)
    }

    /// The outline of a stroke with this brush: the stroke's width as the size, real
    /// pressure when the brush uses it and every point has one, simulated pressure for a
    /// pen without pressure, none for a highlighter.
    #[must_use]
    pub fn outline(&self, stroke: &Stroke) -> Vec<Vec2> {
        let use_pressure = self.uses_pressure && stroke.has_pressure();
        let points: Vec<InputPoint> = stroke
            .points
            .iter()
            .map(|p| InputPoint::new(p.x, p.y, if use_pressure { p.pressure } else { None }))
            .collect();
        let options = StrokeOptions {
            size: stroke.width,
            simulate_pressure: self.options.simulate_pressure && !use_pressure,
            ..self.options
        };
        get_stroke(&points, &options)
    }
}

/// One paint operation of a rendered document: fill `outline` once, non-zero winding, with
/// `color` at `opacity`. Operations are in z-order.
#[derive(Debug, Clone, PartialEq)]
pub struct Composite {
    /// The stroke's id.
    pub stroke_id: String,
    /// The polygon to fill, closed implicitly.
    pub outline: Vec<Vec2>,
    /// The stroke's colour.
    pub color: String,
    /// The opacity of the single fill.
    pub opacity: f64,
    /// Whether the stroke's own brush was used (`false`: a fallback brush; label it).
    pub exact_brush: bool,
}

/// The render plan for a document: exactly one [`Composite`] per live stroke, in z-order.
#[must_use]
pub fn composite_plan(document: &InkDocument) -> Vec<Composite> {
    document
        .live_strokes()
        .map(|stroke| {
            let (brush, exact) = Brush::for_version(&stroke.brush_version)
                .map_or_else(|| (Brush::fallback(stroke.tool), false), |b| (b, true));
            Composite {
                stroke_id: stroke.id.clone(),
                outline: brush.outline(stroke),
                color: stroke.color.clone(),
                opacity: stroke.opacity,
                exact_brush: exact,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::InkPoint;

    fn stroke(id: &str, tool: Tool, brush: &str, pressure: Option<f64>) -> Stroke {
        let points = (0..12)
            .map(|i| InkPoint {
                x: f64::from(i) * 8.0,
                y: 50.0 + f64::from(i % 3),
                pressure,
                elapsed_ms: f64::from(i) * 8.0,
                altitude: None,
                azimuth: None,
            })
            .collect();
        Stroke {
            id: id.to_owned(),
            tool,
            brush_version: brush.to_owned(),
            width: 10.0,
            color: "#FFE066".to_owned(),
            opacity: 0.35,
            points,
        }
    }

    #[test]
    fn one_composite_per_live_stroke_in_z_order() {
        let mut doc = InkDocument::new("d", "s", "c");
        doc.strokes
            .push(stroke("h1", Tool::Highlighter, HIGHLIGHTER_1.version, None));
        doc.strokes
            .push(stroke("p1", Tool::Pen, PEN_1.version, Some(0.7)));
        doc.strokes
            .push(stroke("h2", Tool::Highlighter, HIGHLIGHTER_1.version, None));
        doc.deleted_stroke_ids.push("p1".into());
        let plan = composite_plan(&doc);
        assert_eq!(
            plan.iter()
                .map(|c| c.stroke_id.as_str())
                .collect::<Vec<_>>(),
            ["h1", "h2"]
        );
        assert!(
            plan.iter()
                .all(|c| c.exact_brush && (c.opacity - 0.35).abs() < f64::EPSILON)
        );
        assert!(plan.iter().all(|c| c.outline.len() > 8));
    }

    #[test]
    fn an_unknown_brush_version_is_rendered_by_a_labelled_fallback() {
        let mut doc = InkDocument::new("d", "s", "c");
        doc.strokes
            .push(stroke("p", Tool::Pen, "pf-9.9.9/pen-7", None));
        let plan = composite_plan(&doc);
        assert_eq!(plan.len(), 1);
        assert!(!plan[0].exact_brush);
        assert_eq!(plan[0].outline, PEN_1.outline(&doc.strokes[0]));
    }

    #[test]
    fn a_pen_with_pressure_differs_from_the_same_pen_without() {
        let with = PEN_1.outline(&stroke("p", Tool::Pen, PEN_1.version, Some(1.0)));
        let without = PEN_1.outline(&stroke("p", Tool::Pen, PEN_1.version, None));
        assert_ne!(with, without);
        // The highlighter ignores pressure: same band either way.
        let hl_with = HIGHLIGHTER_1.outline(&stroke(
            "h",
            Tool::Highlighter,
            HIGHLIGHTER_1.version,
            Some(1.0),
        ));
        let hl_without =
            HIGHLIGHTER_1.outline(&stroke("h", Tool::Highlighter, HIGHLIGHTER_1.version, None));
        assert_eq!(hl_with, hl_without);
    }

    #[test]
    fn brush_ids_carry_the_engine_version() {
        for brush in BRUSHES {
            assert!(brush.version.starts_with(ENGINE_VERSION));
            assert_eq!(Brush::for_version(brush.version), Some(brush));
            assert_eq!(Brush::current(brush.tool), brush);
        }
        assert_eq!(Brush::for_version("pen"), None);
    }
}
