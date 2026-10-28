//! Offset adapters between the core's Unicode scalar values and the units platforms count in:
//! UTF-16 code units (Apple, Android, the DOM) and grapheme clusters (what a caret moves
//! over). Every conversion takes the segment text, because the same scalar offset is a
//! different UTF-16 offset in every string, and every conversion is lossless or an error:
//! an offset inside a surrogate pair, off a grapheme boundary or past the end is rejected,
//! never rounded (specs/03-data-model.md, "Token and morphology design").

use openlogos_core::anchor::ScalarRange;
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

/// A half-open range `[start, end)` of UTF-16 code unit offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Utf16Range {
    /// First code unit of the range.
    pub start: u32,
    /// One past the last code unit of the range.
    pub end: u32,
}

/// A half-open range `[start, end)` of extended grapheme cluster indexes (UAX #29).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GraphemeRange {
    /// First cluster of the range.
    pub start: u32,
    /// One past the last cluster of the range.
    pub end: u32,
}

/// The unit an offset is counted in, for error reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OffsetUnit {
    /// Unicode scalar values.
    Scalar,
    /// UTF-16 code units.
    Utf16,
    /// Extended grapheme clusters.
    Grapheme,
}

/// Why an offset cannot be converted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OffsetError {
    /// An offset past the end of the text.
    #[error("{unit:?} offset {offset} is past the end of the text ({length} {unit:?} units)")]
    OutOfBounds {
        /// The offending offset.
        offset: u32,
        /// Length of the text in the same unit.
        length: u32,
        /// The unit both numbers are counted in.
        unit: OffsetUnit,
    },
    /// A UTF-16 offset between the two halves of a surrogate pair.
    #[error("UTF-16 offset {offset} splits a surrogate pair")]
    SplitsSurrogatePair {
        /// The offending offset.
        offset: u32,
    },
    /// A scalar offset inside a grapheme cluster (between a base letter and its marks).
    #[error("scalar offset {offset} is not on a grapheme cluster boundary")]
    NotGraphemeBoundary {
        /// The offending offset.
        offset: u32,
    },
    /// A range whose end precedes its start.
    #[error("range runs backwards: {start} to {end}")]
    Reversed {
        /// Start offset.
        start: u32,
        /// End offset.
        end: u32,
    },
}

/// Length of `text` in scalar values.
#[must_use]
pub fn scalar_len(text: &str) -> u32 {
    text.chars().count() as u32
}

/// Length of `text` in UTF-16 code units.
#[must_use]
pub fn utf16_len(text: &str) -> u32 {
    text.encode_utf16().count() as u32
}

/// Length of `text` in extended grapheme clusters.
#[must_use]
pub fn grapheme_len(text: &str) -> u32 {
    text.graphemes(true).count() as u32
}

/// The text under a scalar range.
pub fn slice_scalars(text: &str, range: ScalarRange) -> Result<String, OffsetError> {
    check_order(range.start, range.end)?;
    let length = scalar_len(text);
    if range.end > length {
        return Err(OffsetError::OutOfBounds {
            offset: range.end,
            length,
            unit: OffsetUnit::Scalar,
        });
    }
    Ok(text
        .chars()
        .skip(range.start as usize)
        .take(range.len() as usize)
        .collect())
}

/// Converts a scalar range to UTF-16 code units.
pub fn scalar_to_utf16(text: &str, range: ScalarRange) -> Result<Utf16Range, OffsetError> {
    check_order(range.start, range.end)?;
    Ok(Utf16Range {
        start: scalar_offset_to_utf16(text, range.start)?,
        end: scalar_offset_to_utf16(text, range.end)?,
    })
}

/// Converts a UTF-16 range to scalar values, rejecting an offset inside a surrogate pair.
pub fn utf16_to_scalar(text: &str, range: Utf16Range) -> Result<ScalarRange, OffsetError> {
    check_order(range.start, range.end)?;
    Ok(ScalarRange {
        start: utf16_offset_to_scalar(text, range.start)?,
        end: utf16_offset_to_scalar(text, range.end)?,
    })
}

/// Converts a scalar range to grapheme cluster indexes, rejecting an offset inside a cluster.
pub fn scalar_to_grapheme(text: &str, range: ScalarRange) -> Result<GraphemeRange, OffsetError> {
    check_order(range.start, range.end)?;
    let boundaries = grapheme_boundaries(text);
    let find = |offset: u32| -> Result<u32, OffsetError> {
        let length = *boundaries.last().unwrap_or(&0);
        if offset > length {
            return Err(OffsetError::OutOfBounds {
                offset,
                length,
                unit: OffsetUnit::Scalar,
            });
        }
        boundaries
            .binary_search(&offset)
            .map(|index| index as u32)
            .map_err(|_| OffsetError::NotGraphemeBoundary { offset })
    };
    Ok(GraphemeRange {
        start: find(range.start)?,
        end: find(range.end)?,
    })
}

/// Converts grapheme cluster indexes to a scalar range.
pub fn grapheme_to_scalar(text: &str, range: GraphemeRange) -> Result<ScalarRange, OffsetError> {
    check_order(range.start, range.end)?;
    let boundaries = grapheme_boundaries(text);
    let length = (boundaries.len() - 1) as u32;
    let find = |index: u32| -> Result<u32, OffsetError> {
        boundaries
            .get(index as usize)
            .copied()
            .ok_or(OffsetError::OutOfBounds {
                offset: index,
                length,
                unit: OffsetUnit::Grapheme,
            })
    };
    Ok(ScalarRange {
        start: find(range.start)?,
        end: find(range.end)?,
    })
}

/// Widens a scalar range outwards to the nearest grapheme cluster boundaries, so a selection
/// never ends between a letter and its marks. A range already on boundaries is unchanged.
pub fn snap_to_graphemes(text: &str, range: ScalarRange) -> Result<ScalarRange, OffsetError> {
    check_order(range.start, range.end)?;
    let boundaries = grapheme_boundaries(text);
    let length = *boundaries.last().unwrap_or(&0);
    if range.end > length {
        return Err(OffsetError::OutOfBounds {
            offset: range.end,
            length,
            unit: OffsetUnit::Scalar,
        });
    }
    let start = boundaries
        .iter()
        .rev()
        .find(|b| **b <= range.start)
        .copied()
        .unwrap_or(0);
    let end = boundaries
        .iter()
        .find(|b| **b >= range.end)
        .copied()
        .unwrap_or(length);
    Ok(ScalarRange { start, end })
}

/// Scalar offsets of every grapheme cluster boundary, starting with 0 and ending with the
/// length of the text.
fn grapheme_boundaries(text: &str) -> Vec<u32> {
    let mut boundaries = Vec::with_capacity(text.len() / 2 + 1);
    let mut scalars = 0u32;
    boundaries.push(0);
    for cluster in text.graphemes(true) {
        scalars += scalar_len(cluster);
        boundaries.push(scalars);
    }
    boundaries
}

fn scalar_offset_to_utf16(text: &str, offset: u32) -> Result<u32, OffsetError> {
    let mut scalars = 0u32;
    let mut units = 0u32;
    for c in text.chars() {
        if scalars == offset {
            return Ok(units);
        }
        scalars += 1;
        units += c.len_utf16() as u32;
    }
    if scalars == offset {
        Ok(units)
    } else {
        Err(OffsetError::OutOfBounds {
            offset,
            length: scalars,
            unit: OffsetUnit::Scalar,
        })
    }
}

fn utf16_offset_to_scalar(text: &str, offset: u32) -> Result<u32, OffsetError> {
    let mut scalars = 0u32;
    let mut units = 0u32;
    for c in text.chars() {
        if units == offset {
            return Ok(scalars);
        }
        let width = c.len_utf16() as u32;
        if units < offset && offset < units + width {
            return Err(OffsetError::SplitsSurrogatePair { offset });
        }
        scalars += 1;
        units += width;
    }
    if units == offset {
        Ok(scalars)
    } else {
        Err(OffsetError::OutOfBounds {
            offset,
            length: units,
            unit: OffsetUnit::Utf16,
        })
    }
}

fn check_order(start: u32, end: u32) -> Result<(), OffsetError> {
    if end < start {
        Err(OffsetError::Reversed { start, end })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Genesis 1:1 from the Westminster Leningrad Codex, byte-exact, with cantillation.
    const GEN_1_1: &str = "בְּרֵאשִׁ֖ית בָּרָ֣א אֱלֹהִ֑ים אֵ֥ת הַשָּׁמַ֖יִם וְאֵ֥ת הָאָֽרֶץ׃";
    /// A note mixing scripts: combining marks, an emoji with a skin-tone modifier and a
    /// ZWJ family, and Hebrew inside first-strong isolates (U+2068, U+2069).
    const NOTE: &str = "Café 👍🏽 👨‍👩‍👧 see \u{2068}אֱלֹהִ֑ים\u{2069} here";

    fn sr(start: u32, end: u32) -> ScalarRange {
        ScalarRange { start, end }
    }

    #[test]
    fn lengths_differ_per_unit() {
        assert_eq!(scalar_len(GEN_1_1), 65);
        assert_eq!(utf16_len(GEN_1_1), 65);
        // 28 letters, each with its marks, plus six spaces and the sof pasuq.
        assert_eq!(grapheme_len(GEN_1_1), 35);
        // 👍🏽 is two scalars and four UTF-16 units; the family is five scalars, eight units.
        assert_eq!(scalar_len("👍🏽"), 2);
        assert_eq!(utf16_len("👍🏽"), 4);
        assert_eq!(grapheme_len("👍🏽"), 1);
        assert_eq!(scalar_len("👨‍👩‍👧"), 5);
        assert_eq!(utf16_len("👨‍👩‍👧"), 8);
        assert_eq!(grapheme_len("👨‍👩‍👧"), 1);
        assert_eq!(scalar_len(""), 0);
        assert_eq!(grapheme_len(""), 0);
    }

    #[test]
    fn utf16_round_trips_through_scalars_for_every_boundary() {
        for text in [GEN_1_1, NOTE, "", "plain ascii", "a\u{0301}b"] {
            let units = utf16_len(text);
            let mut valid: Vec<u32> = Vec::new();
            for unit in 0..=units {
                match utf16_to_scalar(
                    text,
                    Utf16Range {
                        start: unit,
                        end: unit,
                    },
                ) {
                    Ok(_) => valid.push(unit),
                    Err(OffsetError::SplitsSurrogatePair { .. }) => {}
                    Err(other) => panic!("{text:?} at {unit}: {other}"),
                }
            }
            for (i, &start) in valid.iter().enumerate() {
                for &end in &valid[i..] {
                    let range = Utf16Range { start, end };
                    let scalars = utf16_to_scalar(text, range).unwrap();
                    assert_eq!(scalar_to_utf16(text, scalars).unwrap(), range, "{text:?}");
                }
            }
        }
    }

    #[test]
    fn surrogate_pairs_cannot_be_split() {
        // "a👍🏽b": a=0, 👍=1..3, 🏽=3..5, b=5..6 in UTF-16.
        let text = "a👍🏽b";
        for offset in [2u32, 4] {
            assert_eq!(
                utf16_to_scalar(
                    text,
                    Utf16Range {
                        start: offset,
                        end: offset
                    }
                ),
                Err(OffsetError::SplitsSurrogatePair { offset })
            );
        }
        assert_eq!(
            utf16_to_scalar(text, Utf16Range { start: 1, end: 5 }).unwrap(),
            sr(1, 3)
        );
        assert_eq!(
            scalar_to_utf16(text, sr(1, 3)).unwrap(),
            Utf16Range { start: 1, end: 5 }
        );
    }

    #[test]
    fn out_of_range_and_reversed_offsets_are_errors() {
        assert_eq!(
            scalar_to_utf16("abc", sr(2, 4)),
            Err(OffsetError::OutOfBounds {
                offset: 4,
                length: 3,
                unit: OffsetUnit::Scalar
            })
        );
        assert_eq!(
            utf16_to_scalar("abc", Utf16Range { start: 0, end: 9 }),
            Err(OffsetError::OutOfBounds {
                offset: 9,
                length: 3,
                unit: OffsetUnit::Utf16
            })
        );
        assert_eq!(
            grapheme_to_scalar("abc", GraphemeRange { start: 0, end: 4 }),
            Err(OffsetError::OutOfBounds {
                offset: 4,
                length: 3,
                unit: OffsetUnit::Grapheme
            })
        );
        assert_eq!(
            scalar_to_grapheme("abc", sr(0, 4)),
            Err(OffsetError::OutOfBounds {
                offset: 4,
                length: 3,
                unit: OffsetUnit::Scalar
            })
        );
        assert_eq!(
            scalar_to_utf16("abc", sr(2, 1)),
            Err(OffsetError::Reversed { start: 2, end: 1 })
        );
        assert!(slice_scalars("abc", sr(1, 5)).is_err());
        assert_eq!(slice_scalars("abc", sr(3, 3)).unwrap(), "");
    }

    #[test]
    fn hebrew_letters_keep_their_points_and_accents_in_one_cluster() {
        // בְּרֵאשִׁ֖ית: bet+dagesh+sheva, resh+tsere, alef, shin+shin-dot+hiriq+tipcha, yod, tav.
        let word = "בְּרֵאשִׁ֖ית";
        assert_eq!(scalar_len(word), 12);
        assert_eq!(grapheme_len(word), 6);
        assert_eq!(
            grapheme_to_scalar(word, GraphemeRange { start: 0, end: 6 }).unwrap(),
            sr(0, 12)
        );
        assert_eq!(
            grapheme_to_scalar(word, GraphemeRange { start: 1, end: 2 }).unwrap(),
            sr(3, 5)
        );
        assert_eq!(
            scalar_to_grapheme(word, sr(3, 5)).unwrap(),
            GraphemeRange { start: 1, end: 2 }
        );
        // Between bet and its dagesh is not a boundary.
        assert_eq!(
            scalar_to_grapheme(word, sr(1, 5)),
            Err(OffsetError::NotGraphemeBoundary { offset: 1 })
        );
        assert_eq!(snap_to_graphemes(word, sr(1, 4)).unwrap(), sr(0, 5));
        assert_eq!(snap_to_graphemes(word, sr(3, 5)).unwrap(), sr(3, 5));
        // The whole verse: every cluster boundary round-trips.
        let clusters = grapheme_len(GEN_1_1);
        for index in 0..=clusters {
            let scalars = grapheme_to_scalar(
                GEN_1_1,
                GraphemeRange {
                    start: index,
                    end: index,
                },
            )
            .unwrap();
            assert_eq!(
                scalar_to_grapheme(GEN_1_1, scalars).unwrap(),
                GraphemeRange {
                    start: index,
                    end: index
                }
            );
        }
    }

    #[test]
    fn maqqef_is_its_own_cluster_and_sof_pasuq_too() {
        // עַל־פְּנֵ֣י: ayin+patah, lamed, maqqef, pe+dagesh+sheva, nun+tsere+munah, yod.
        let text = "עַל־פְּנֵ֣י";
        assert_eq!(grapheme_len(text), 6);
        assert_eq!(
            grapheme_to_scalar(text, GraphemeRange { start: 2, end: 3 }).unwrap(),
            sr(3, 4)
        );
        assert_eq!(slice_scalars(text, sr(3, 4)).unwrap(), "\u{05BE}");
        assert_eq!(slice_scalars(GEN_1_1, sr(64, 65)).unwrap(), "\u{05C3}");
    }

    #[test]
    fn isolates_and_emoji_in_a_note_convert_losslessly() {
        // NOTE: "Café " (5 scalars; é is one precomposed scalar here), 👍🏽 (2), " " (1),
        // family (5), " see " (5), U+2068 (1), Hebrew word (9), U+2069 (1), " here" (5).
        assert_eq!(scalar_len(NOTE), 34);
        let hebrew_start = 5 + 2 + 1 + 5 + 5 + 1;
        let hebrew = sr(hebrew_start, hebrew_start + 9);
        assert_eq!(slice_scalars(NOTE, hebrew).unwrap(), "אֱלֹהִ֑ים");
        // UTF-16 offsets: 👍🏽 adds 2 extra units, the family adds 3.
        let utf16 = scalar_to_utf16(NOTE, hebrew).unwrap();
        assert_eq!(
            utf16,
            Utf16Range {
                start: hebrew_start + 5,
                end: hebrew_start + 5 + 9
            }
        );
        assert_eq!(utf16_to_scalar(NOTE, utf16).unwrap(), hebrew);
        // The isolate characters are clusters of their own.
        let clusters = scalar_to_grapheme(NOTE, hebrew).unwrap();
        assert_eq!(grapheme_to_scalar(NOTE, clusters).unwrap(), hebrew);
        let isolate = sr(hebrew_start - 1, hebrew_start);
        assert_eq!(slice_scalars(NOTE, isolate).unwrap(), "\u{2068}");
        assert_eq!(
            scalar_to_grapheme(NOTE, isolate).unwrap().end
                - scalar_to_grapheme(NOTE, isolate).unwrap().start,
            1
        );
    }

    #[test]
    fn errors_serialize_with_a_kind_tag() {
        let err = utf16_to_scalar("a👍🏽b", Utf16Range { start: 2, end: 2 }).unwrap_err();
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "{\"kind\":\"splits_surrogate_pair\",\"offset\":2}");
    }
}
