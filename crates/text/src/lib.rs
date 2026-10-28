//! Render-ready text for every platform. This crate turns stored passages into attributed runs
//! (verse and footnote markers, poetry indentation, divine-name small caps, supported
//! red-letter markup) and translates token boundaries into each platform's string-offset unit,
//! UTF-16 for Apple, Android and the DOM, so renderers paint without reinterpreting text. It
//! owns the versioned normalization modes shared with indexing (case and final-sigma folding,
//! accent and breathing stripping for Greek; points and cantillation U+0591 to U+05C7 for
//! Hebrew) while keeping display text byte-exact; Unicode NFC is never applied to display
//! text. See specs/03-data-model.md, specs/19-search-and-retrieval.md and
//! docs/adr/0001-rust-core-native-ui.md.

pub mod offsets;

pub use offsets::{
    GraphemeRange, OffsetError, OffsetUnit, Utf16Range, grapheme_len, grapheme_to_scalar,
    scalar_len, scalar_to_grapheme, scalar_to_utf16, slice_scalars, snap_to_graphemes, utf16_len,
    utf16_to_scalar,
};
