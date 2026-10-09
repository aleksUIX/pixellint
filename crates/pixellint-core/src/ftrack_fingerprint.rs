//! Source-profile parity for d9core's indexed btoa and unindexed MurmurHash.
//!
//! Reviewed SDK hashes: 77c8c6a48bdfa3444ff0128161ab436693e3ae6be5409e269e873b743baf8bd6
//! and 85e48b46c5da3f5e3a0e86763880dc197b9af2a016514e837ab11c76c7702ccb.
//! The producer publishes no receiver/version contract. Unknown or ambiguous
//! layouts remain unvalidated, including collisions with embedded delimiters.

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FingerprintCheck {
    Matches,
    Mismatch,
    Unvalidated,
}

const TYPES: &[&[u8]] = &[
    b"undefined",
    b"object",
    b"boolean",
    b"number",
    b"bigint",
    b"string",
    b"symbol",
    b"function",
];

fn slot_value(slot: u8, value: &[u8]) -> bool {
    match slot {
        b'b' | b'c' => {
            let digits = value.strip_prefix(b"-").unwrap_or(value);
            !digits.is_empty() && digits.iter().all(u8::is_ascii_digit)
        }
        b'd' | b'e' | b'f' => value == b"true" || value == b"false",
        b'g' | b'h' => TYPES.contains(&value),
        _ => true,
    }
}

fn reconstruct(
    text: &[u8],
    slots: &[u8],
    offset: usize,
    values: &mut Vec<Vec<u8>>,
    candidates: &mut std::collections::BTreeSet<Vec<u8>>,
    budget: &mut usize,
) -> bool {
    if *budget == 0 {
        return false;
    }
    *budget -= 1;
    let slot = slots[values.len()];
    if !text[offset..].starts_with(&[slot, b':']) {
        return true;
    }
    let start = offset + 2;
    if values.len() + 1 == slots.len() {
        let platform = &text[start..];
        // The source omits f only for these exact navigator.platform strings.
        if slots.contains(&b'f') == (platform == b"iPhone" || platform == b"iPad") {
            return true;
        }
        values.push(platform.to_vec());
        let mut unindexed = Vec::new();
        for (index, value) in values.iter().enumerate() {
            if index > 0 {
                unindexed.extend_from_slice(b"###");
            }
            unindexed.extend_from_slice(value);
        }
        candidates.insert(unindexed);
        values.pop();
        return true;
    }
    let marker = [b'#', b'#', b'#', slots[values.len() + 1], b':'];
    for end in start..text.len().saturating_sub(marker.len() - 1) {
        if text[end..].starts_with(&marker) && slot_value(slot, &text[start..end]) {
            values.push(text[start..end].to_vec());
            if !reconstruct(text, slots, end + 3, values, candidates, budget) {
                return false;
            }
            values.pop();
            if candidates.len() > 1 {
                return true;
            }
        }
    }
    true
}

pub(crate) fn check(indexed: &str, hash: u32) -> Option<FingerprintCheck> {
    let text = crate::manifest::decode_base64_bytes(indexed).ok()?;
    // A local resource bound is not a vendor length limit or rejection claim.
    if text.len() > 65_536 {
        return Some(FingerprintCheck::Unvalidated);
    }
    let mut candidates = std::collections::BTreeSet::new();
    let mut budget = 256;
    for slots in [
        b"abcdefghij".as_slice(),
        b"abcdeghij".as_slice(),
        b"abcdfgij".as_slice(),
        b"abcdgij".as_slice(),
    ] {
        if !reconstruct(
            &text,
            slots,
            0,
            &mut Vec::new(),
            &mut candidates,
            &mut budget,
        ) || candidates.len() > 1
        {
            return Some(FingerprintCheck::Unvalidated);
        }
    }
    let Some(candidate) = candidates.into_iter().next() else {
        return Some(FingerprintCheck::Unvalidated);
    };
    Some(if murmurhash3(&candidate) == hash {
        FingerprintCheck::Matches
    } else {
        FingerprintCheck::Mismatch
    })
}

fn murmurhash3(bytes: &[u8]) -> u32 {
    let mut hash = 31_u32;
    let mix = |word: u32| {
        word.wrapping_mul(3_432_918_353)
            .rotate_left(15)
            .wrapping_mul(461_845_907)
    };
    let mut chunks = bytes.chunks_exact(4);
    for chunk in &mut chunks {
        hash ^= mix(u32::from_le_bytes(
            chunk.try_into().expect("four-byte chunk"),
        ));
        hash = hash
            .rotate_left(13)
            .wrapping_mul(5)
            .wrapping_add(0xe654_6b64);
    }
    let remainder = chunks.remainder();
    if !remainder.is_empty() {
        let word = remainder.iter().enumerate().fold(0, |word, (index, byte)| {
            word | (u32::from(*byte) << (8 * index))
        });
        hash ^= mix(word);
    }
    hash ^= bytes.len() as u32;
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(2_246_822_507);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(3_266_489_909);
    hash ^ (hash >> 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_sdk_desktop_and_ios_murmur_outputs() {
        assert_eq!(murmurhash3(b"en###24###0###true###true###true###undefined###undefined###undefined###Linux x86_64"), 2_698_912_147);
        assert_eq!(
            murmurhash3(b"en###24###0###true###true###undefined###undefined###undefined###iPhone"),
            3_542_308_655
        );
    }
}
