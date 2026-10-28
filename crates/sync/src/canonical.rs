//! The byte encoding that signatures, hashes and AEAD associated data are computed over.
//!
//! This is an encoding, not a cryptographic construction: a fixed domain string followed by
//! length-prefixed fields, so that two records agree on their signing input exactly when
//! they agree field by field and no field can absorb part of the next. JSON is never signed
//! directly because its serialization is not canonical across hosts.

/// A writer of length-prefixed fields behind a domain string.
#[derive(Debug, Default, Clone)]
pub struct Canonical {
    bytes: Vec<u8>,
}

impl Canonical {
    /// Starts an encoding under `domain` (for example `openlogos-sync-op/1`).
    #[must_use]
    pub fn new(domain: &str) -> Canonical {
        let mut c = Canonical { bytes: Vec::new() };
        c.bytes(domain.as_bytes());
        c
    }

    /// Appends a byte string with a 4-byte big-endian length prefix.
    pub fn bytes(&mut self, field: &[u8]) -> &mut Canonical {
        let len = u32::try_from(field.len()).expect("a canonical field is shorter than 4 GiB");
        self.bytes.extend_from_slice(&len.to_be_bytes());
        self.bytes.extend_from_slice(field);
        self
    }

    /// Appends a UTF-8 string as a field.
    pub fn text(&mut self, field: &str) -> &mut Canonical {
        self.bytes(field.as_bytes())
    }

    /// Appends an unsigned 64-bit number as an 8-byte big-endian field.
    pub fn u64(&mut self, field: u64) -> &mut Canonical {
        self.bytes(&field.to_be_bytes())
    }

    /// Appends an unsigned 32-bit number as a 4-byte big-endian field.
    pub fn u32(&mut self, field: u32) -> &mut Canonical {
        self.bytes(&field.to_be_bytes())
    }

    /// Appends a list of strings: its length, then each item as a field.
    pub fn list(&mut self, items: &[String]) -> &mut Canonical {
        self.u64(items.len() as u64);
        for item in items {
            self.text(item);
        }
        self
    }

    /// The encoded bytes.
    #[must_use]
    pub fn finish(&self) -> Vec<u8> {
        self.bytes.clone()
    }
}

/// Lower-case hexadecimal of `bytes`.
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[usize::from(b >> 4)] as char);
        out.push(DIGITS[usize::from(b & 0x0f)] as char);
    }
    out
}

/// Parses lower-case or upper-case hexadecimal.
pub fn from_hex(text: &str) -> Result<Vec<u8>, HexError> {
    let digits = text.as_bytes();
    if !digits.len().is_multiple_of(2) {
        return Err(HexError::OddLength(digits.len()));
    }
    let nibble = |d: u8| match d {
        b'0'..=b'9' => Ok(d - b'0'),
        b'a'..=b'f' => Ok(d - b'a' + 10),
        b'A'..=b'F' => Ok(d - b'A' + 10),
        other => Err(HexError::NotHex(other as char)),
    };
    digits
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| Ok((nibble(pair[0])? << 4) | nibble(pair[1])?))
        .collect()
}

/// Parses hexadecimal into a fixed-size array.
pub fn from_hex_array<const N: usize>(text: &str) -> Result<[u8; N], HexError> {
    let bytes = from_hex(text)?;
    <[u8; N]>::try_from(bytes).map_err(|v| HexError::WrongLength {
        expected: N,
        actual: v.len(),
    })
}

/// A hexadecimal string that does not decode.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HexError {
    /// An odd number of digits.
    #[error("hex string has an odd length ({0})")]
    OddLength(usize),
    /// A character outside `0-9a-fA-F`.
    #[error("`{0}` is not a hex digit")]
    NotHex(char),
    /// Decoded to a different number of bytes than the field holds.
    #[error("expected {expected} bytes, found {actual}")]
    WrongLength {
        /// Bytes the field holds.
        expected: usize,
        /// Bytes decoded.
        actual: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_cannot_absorb_their_neighbours() {
        let ab = Canonical::new("t").text("ab").text("c").finish();
        let a_bc = Canonical::new("t").text("a").text("bc").finish();
        assert_ne!(ab, a_bc);
        assert_eq!(ab, Canonical::new("t").text("ab").text("c").finish());
    }

    #[test]
    fn hex_round_trips_and_rejects_junk() {
        assert_eq!(to_hex(&[0, 15, 255]), "000fff");
        assert_eq!(from_hex("000FFf").unwrap(), vec![0, 15, 255]);
        assert_eq!(from_hex("abc"), Err(HexError::OddLength(3)));
        assert_eq!(from_hex("zz"), Err(HexError::NotHex('z')));
        assert!(matches!(
            from_hex_array::<4>("0000"),
            Err(HexError::WrongLength {
                expected: 4,
                actual: 2
            })
        ));
    }
}
