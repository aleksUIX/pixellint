//! Explicit captured wire bytes have bounded, shared native and WASM decoding.

use std::io::Read;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use flate2::bufread::GzDecoder;

pub(crate) const MAX_WIRE_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_ENTITY_BYTES: usize = 16 * 1024 * 1024;
const MAX_MEMBERS: usize = 1024;
const MAX_HEADER_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BinaryBodyError {
    Base64,
    Gzip,
    Utf8,
    Limit,
}

pub(crate) struct BinaryBody {
    pub(crate) text: String,
    pub(crate) wire_bytes: usize,
}

#[derive(Clone, Copy)]
struct Limits {
    wire: usize,
    entity: usize,
    members: usize,
    header: usize,
}

const LIMITS: Limits = Limits {
    wire: MAX_WIRE_BYTES,
    entity: MAX_ENTITY_BYTES,
    members: MAX_MEMBERS,
    header: MAX_HEADER_BYTES,
};

pub(crate) fn decode(encoded: &str, gzip: bool) -> Result<BinaryBody, BinaryBodyError> {
    decode_with_limits(encoded, gzip, LIMITS)
}

fn decode_with_limits(
    encoded: &str,
    gzip: bool,
    limits: Limits,
) -> Result<BinaryBody, BinaryBodyError> {
    // Check the encoded length before allocating a decoded byte buffer.
    if encoded.len() > limits.wire.div_ceil(3) * 4 {
        return Err(BinaryBodyError::Limit);
    }
    let wire = STANDARD
        .decode(encoded)
        .map_err(|_| BinaryBodyError::Base64)?;
    if wire.len() > limits.wire {
        return Err(BinaryBodyError::Limit);
    }
    let wire_bytes = wire.len();
    let entity = if gzip {
        inflate(&wire, limits)?
    } else {
        if wire.len() > limits.entity {
            return Err(BinaryBodyError::Limit);
        }
        wire
    };
    let text = String::from_utf8(entity).map_err(|_| BinaryBodyError::Utf8)?;
    Ok(BinaryBody { text, wire_bytes })
}

fn inflate(wire: &[u8], limits: Limits) -> Result<Vec<u8>, BinaryBodyError> {
    if wire.is_empty() {
        return Err(BinaryBodyError::Gzip);
    }
    let mut remaining = wire;
    let mut entity = Vec::new();
    let mut members = 0;
    while !remaining.is_empty() {
        if members == limits.members {
            return Err(BinaryBodyError::Limit);
        }
        members += 1;
        // The library parses headers in new(). Bound every optional header
        // field before constructing it, without retaining names or comments.
        check_header(remaining, limits.header)?;
        let mut decoder = GzDecoder::new(remaining);
        let mut chunk = [0_u8; 16 * 1024];
        loop {
            // The extra byte distinguishes an exact bound from more output.
            // Reading to zero also verifies CRC and ISIZE at the exact bound.
            let available = (limits.entity - entity.len() + 1).min(chunk.len());
            let read = decoder
                .read(&mut chunk[..available])
                .map_err(|_| BinaryBodyError::Gzip)?;
            if read == 0 {
                break;
            }
            if read > limits.entity - entity.len() {
                return Err(BinaryBodyError::Limit);
            }
            entity.extend_from_slice(&chunk[..read]);
        }
        let next = decoder.into_inner();
        if next.len() >= remaining.len() {
            return Err(BinaryBodyError::Gzip);
        }
        remaining = next;
    }
    // UTF-8 conversion happens after all members, including member boundaries
    // that split a Unicode scalar. No partial entity escapes on any failure.
    Ok(entity)
}

fn check_header(wire: &[u8], max: usize) -> Result<(), BinaryBodyError> {
    if wire.len() < 10 || wire[..3] != [0x1f, 0x8b, 8] || wire[3] & 0xe0 != 0 {
        return Err(BinaryBodyError::Gzip);
    }
    let flags = wire[3];
    let mut offset = 10;
    if flags & 4 != 0 {
        checked_header_end(wire, offset + 2, max)?;
        let len = u16::from_le_bytes([wire[offset], wire[offset + 1]]) as usize;
        offset += 2;
        checked_header_end(wire, offset + len, max)?;
        offset += len;
    }
    for flag in [8, 16] {
        if flags & flag != 0 {
            let bytes = &wire[offset..];
            let budget = max.saturating_sub(offset);
            let end = bytes.iter().take(budget).position(|byte| *byte == 0);
            match end {
                Some(end) => offset += end + 1,
                None if bytes.len() >= budget => return Err(BinaryBodyError::Limit),
                None => return Err(BinaryBodyError::Gzip),
            }
        }
    }
    if flags & 2 != 0 {
        offset += 2;
    }
    checked_header_end(wire, offset, max)
}

fn checked_header_end(wire: &[u8], end: usize, max: usize) -> Result<(), BinaryBodyError> {
    if end > wire.len() {
        Err(BinaryBodyError::Gzip)
    } else if end > max {
        Err(BinaryBodyError::Limit)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Python 3 gzip.compress(b"0123456789", mtime=0), independently reproduced
    // by Node zlib.gzipSync. The decompressed bytes are the test oracle.
    const TEN: &str = "H4sIAAAAAAAC/zMwNDI2MTUzt7AEAMbHhKYKAAAA";

    fn small() -> Limits {
        Limits {
            wire: 128,
            entity: 10,
            members: 2,
            header: 64,
        }
    }

    #[test]
    fn exact_entity_bound_reads_and_verifies_footer() {
        assert_eq!(
            decode_with_limits(TEN, true, small()).unwrap().text,
            "0123456789"
        );
        let mut wire = STANDARD.decode(TEN).unwrap();
        let footer = wire.len() - 8;
        wire[footer] ^= 1;
        assert_eq!(
            decode_with_limits(&STANDARD.encode(wire), true, small()).err(),
            Some(BinaryBodyError::Gzip)
        );
    }

    #[test]
    fn local_bounds_remain_separate_from_malformed_data() {
        let mut limits = small();
        limits.entity = 9;
        assert_eq!(
            decode_with_limits(TEN, true, limits).err(),
            Some(BinaryBodyError::Limit)
        );
        limits = small();
        limits.wire = 5;
        assert_eq!(
            decode_with_limits(TEN, true, limits).err(),
            Some(BinaryBodyError::Limit)
        );
        assert_eq!(
            decode_with_limits("AA=A", false, small()).err(),
            Some(BinaryBodyError::Base64)
        );
        for encoded in ["AB==", "_w==", "AA==\n", "AA"] {
            assert_eq!(
                decode_with_limits(encoded, false, small()).err(),
                Some(BinaryBodyError::Base64)
            );
        }
    }

    #[test]
    fn header_bound_is_aggregate_and_includes_terminators() {
        let original = STANDARD.decode(TEN).unwrap();
        let mut wire = original[..10].to_vec();
        wire[3] = 8;
        wire.extend_from_slice(b"name\0");
        wire.extend_from_slice(&original[10..]);
        let mut limits = small();
        limits.header = 15;
        assert_eq!(
            decode_with_limits(&STANDARD.encode(&wire), true, limits)
                .unwrap()
                .text,
            "0123456789"
        );
        limits.header = 14;
        assert_eq!(
            decode_with_limits(&STANDARD.encode(&wire), true, limits).err(),
            Some(BinaryBodyError::Limit)
        );
    }

    #[test]
    fn empty_members_count_without_inventing_entity_bytes() {
        let empty = STANDARD.decode("H4sIAAAAAAAC/wMAAAAAAAAAAAA=").unwrap();
        let two = [empty.as_slice(), empty.as_slice()].concat();
        assert_eq!(
            decode_with_limits(&STANDARD.encode(&two), true, small())
                .unwrap()
                .text,
            ""
        );
        let three = [two.as_slice(), empty.as_slice()].concat();
        assert_eq!(
            decode_with_limits(&STANDARD.encode(&three), true, small()).err(),
            Some(BinaryBodyError::Limit)
        );
    }

    #[test]
    fn corrupt_later_member_never_exposes_the_valid_first_entity() {
        let ten = STANDARD.decode(TEN).unwrap();
        let mut second = ten.clone();
        let footer = second.len() - 8;
        second[footer] ^= 1;
        let wire = [ten.as_slice(), second.as_slice()].concat();
        let mut limits = small();
        limits.entity = 20;
        assert_eq!(
            decode_with_limits(&STANDARD.encode(&wire), true, limits).err(),
            Some(BinaryBodyError::Gzip)
        );
    }

    #[test]
    fn decoded_wire_bound_is_checked_after_encoded_length_preflight() {
        let mut limits = small();
        limits.wire = 4;
        // Four and five captured bytes both need eight Base64 characters.
        assert_eq!(
            decode_with_limits("AAAAAA==", false, limits)
                .unwrap()
                .wire_bytes,
            4
        );
        assert_eq!(
            decode_with_limits("AAAAAAA=", false, limits).err(),
            Some(BinaryBodyError::Limit)
        );
    }
}
