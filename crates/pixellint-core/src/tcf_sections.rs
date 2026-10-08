//! Local structural validation of the TCF v2 consent-string bit sections.
//!
//! Sources: IAB TCF v2 string-format tables and the official iabtcf-es encoder.
//! Historical v2 core-only and allowed-vendor segments remain decodable.
//! Published current policy checks emit advisories. Registry membership,
//! historical rule activation and processing permissions remain separate.

#[cfg(test)]
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
// IAB Europe's 2025-08-19 transition clarification preserves older core-only
// strings but rejects new ones created after 2026-02-28 without disclosure.
// The exact transition hour on February 28 is not specified in that source.
const DISCLOSURE_REQUIRED_FROM_DECISECONDS: u64 = 17_723_232_000;
const POLICY_FOUR_REQUIRED_FROM_DECISECONDS: u64 = 16_961_184_000;
const DECISECONDS_PER_DAY: u64 = 864_000;
// ISO-linked Library of Congress Set 1 publication, retrieved 2026-10-07.
// Advisory only: a current snapshot does not invalidate historical strings.
const ISO_639_1_CODES: &[&str] = &[
    "aa", "ab", "ae", "af", "ak", "am", "an", "ar", "as", "av", "ay", "az", "ba", "be", "bg", "bi",
    "bm", "bn", "bo", "br", "bs", "ca", "ce", "ch", "co", "cr", "cs", "cu", "cv", "cy", "da", "de",
    "dv", "dz", "ee", "el", "en", "eo", "es", "et", "eu", "fa", "ff", "fi", "fj", "fo", "fr", "fy",
    "ga", "gd", "gl", "gn", "gu", "gv", "ha", "he", "hi", "ho", "hr", "ht", "hu", "hy", "hz", "ia",
    "id", "ie", "ig", "ii", "ik", "io", "is", "it", "iu", "ja", "jv", "ka", "kg", "ki", "kj", "kk",
    "kl", "km", "kn", "ko", "kr", "ks", "ku", "kv", "kw", "ky", "la", "lb", "lg", "li", "ln", "lo",
    "lt", "lu", "lv", "mg", "mh", "mi", "mk", "ml", "mn", "mr", "ms", "mt", "my", "na", "nb", "nd",
    "ne", "ng", "nl", "nn", "no", "nr", "nv", "ny", "oc", "oj", "om", "or", "os", "pa", "pi", "pl",
    "ps", "pt", "qu", "rm", "rn", "ro", "ru", "rw", "sa", "sc", "sd", "se", "sg", "si", "sk", "sl",
    "sm", "sn", "so", "sq", "sr", "ss", "st", "su", "sv", "sw", "ta", "te", "tg", "th", "ti", "tk",
    "tl", "tn", "to", "tr", "ts", "tt", "tw", "ty", "ug", "uk", "ur", "uz", "ve", "vi", "vo", "wa",
    "wo", "xh", "yi", "yo", "za", "zh", "zu",
];

/// Check explicit destination requirements using only encoded positive signals.
/// Invalid structures return an error so callers can retain their structural
/// diagnostic and avoid a second, misleading permission failure.
pub(crate) fn tcf_consent_requirements(
    value: &str,
    vendor_id: u16,
    purpose_ids: &[u8],
) -> Result<bool, String> {
    if vendor_id == 0 || purpose_ids.iter().any(|id| !(1..=24).contains(id)) {
        return Err(
            "TCF consent requirements need positive vendor IDs and purposes 1 through 24"
                .to_string(),
        );
    }
    validate_tcf_v2(value)?;
    let mut core = Bits::new(value.split('.').next().unwrap_or_default(), "core")?;
    core.skip(152, "Core metadata and special features")?;
    let purposes = core.read(24, "PurposesConsent")?;
    if purpose_ids
        .iter()
        .any(|id| purposes & (1 << (24 - id)) == 0)
    {
        return Ok(false);
    }
    core.skip(37, "Legitimate interests and jurisdiction")?;
    core.vendor_enabled(vendor_id, "VendorConsents")
}

/// Policy diagnostics are separate because published vendor and IAB examples
/// retain metadata that conflicts with newer policy text. They are advisories,
/// not failures of the bit-section decoder or proof of processing permission.
pub(crate) fn tcf_policy_warnings(value: &str) -> Vec<String> {
    if validate_tcf_v2(value).is_err() {
        return Vec::new();
    }
    policy_fields(value).unwrap_or_default()
}

fn policy_fields(value: &str) -> Result<Vec<String>, String> {
    let mut core = Bits::new(value.split('.').next().unwrap_or_default(), "core")?;
    core.skip(6, "Version")?;
    let created = core.read_u64(36, "Created")?;
    let updated = core.read_u64(36, "LastUpdated")?;
    core.skip(30, "CMP metadata and consent screen")?;
    let language = format!(
        "{}{}",
        char::from(b'a' + core.read(6, "ConsentLanguage first letter")? as u8),
        char::from(b'a' + core.read(6, "ConsentLanguage second letter")? as u8)
    );
    core.skip(12, "VendorListVersion")?;
    let policy = core.read(6, "TcfPolicyVersion")?;
    let specific = core.read(1, "IsServiceSpecific")?;
    core.skip(1, "UseNonStandardTexts")?;
    let features = core.read(12, "SpecialFeatureOptIns")?;
    let consents = core.read(24, "PurposesConsent")?;
    let interests = core.read(24, "PurposesLITransparency")?;
    let mut warnings = Vec::new();
    if ISO_639_1_CODES.binary_search(&language.as_str()).is_err() {
        warnings.push(format!(
            "TCF ConsentLanguage {language} is not assigned in the ISO-linked Library of Congress ISO 639-1 publication retrieved 2026-10-07; historical or future assignments require their dated registry"
        ));
    }
    if specific == 0 {
        warnings.push("TCF global scope is invalid since 2021-09-01; IsServiceSpecific must be 1 under the current specification".to_string());
    }
    if created >= POLICY_FOUR_REQUIRED_FROM_DECISECONDS && policy < 4 {
        warnings.push(
            "TCF strings created after 2023-09-30 must use TcfPolicyVersion 4 or higher"
                .to_string(),
        );
    }
    if policy >= 4 && interests & (0b1111 << 18) != 0 {
        warnings.push("TCF policy 4 and higher requires legitimate-interest bits for purposes 3 through 6 to be zero".to_string());
    }
    // IAB Europe's November 2023 FAQ maps policy versions <=3 to GVL v2.
    // Dated primary archives assign purposes 1..10 in that family and
    // 1..11 in policies 4/5. They assign only special features 1/2.
    // Unpublished or unevidenced policy versions receive no assignment cap.
    let highest_purpose = match policy {
        1..=3 => Some(10),
        4 | 5 => Some(11),
        _ => None,
    };
    if let Some(highest) = highest_purpose {
        let unassigned = (1 << (24 - highest)) - 1;
        if consents & unassigned != 0 {
            warnings.push(format!(
                "TCF policy {policy} does not assign purpose-consent IDs {} through 24",
                highest + 1
            ));
        }
        if interests & unassigned != 0 {
            warnings.push(format!(
                "TCF policy {policy} does not assign legitimate-interest purpose IDs {} through 24",
                highest + 1
            ));
        }
        if features & ((1 << 10) - 1) != 0 {
            warnings.push(format!(
                "TCF policy {policy} does not assign special-feature IDs 3 through 12"
            ));
        }
    }
    // Policy 4 (TCF 2.2) and 5 follow the December 2021 timestamp change.
    // The source supplies no exact historical activation day for older policy
    // versions, and future policy values are not assigned unpublished rules.
    if matches!(policy, 4 | 5) {
        if created != updated {
            warnings.push(
                "TCF policy 4/5 Created and LastUpdated must contain the same day-level timestamp"
                    .to_string(),
            );
        }
        if !created.is_multiple_of(DECISECONDS_PER_DAY)
            || !updated.is_multiple_of(DECISECONDS_PER_DAY)
        {
            warnings.push(
                "TCF policy 4/5 timestamps must represent UTC midnight at day precision"
                    .to_string(),
            );
        }
    }
    if let Some(highest) = highest_purpose {
        core.skip(13, "PurposeOneTreatment and PublisherCC")?;
        core.vendors("VendorConsents")?;
        core.vendors("VendorLegitimateInterests")?;
        let count = core.read(12, "NumPubRestrictions")?;
        let mut unassigned_restrictions = std::collections::BTreeSet::new();
        for _ in 0..count {
            let purpose = core.read(6, "PublisherRestriction.PurposeId")?;
            if purpose > highest {
                unassigned_restrictions.insert(purpose);
            }
            core.skip(2, "PublisherRestriction.RestrictionType")?;
            let entries = core.read(12, "PublisherRestriction.NumEntries")?;
            core.ranges(entries, u16::MAX as usize, "PublisherRestriction")?;
        }
        if !unassigned_restrictions.is_empty() {
            warnings.push(format!(
                "TCF policy {policy} does not assign publisher-restriction purpose IDs {unassigned_restrictions:?}"
            ));
        }
        let unassigned = (1 << (24 - highest)) - 1;
        for encoded in value.split('.').skip(1) {
            let mut segment = Bits::new(encoded, "additional segment")?;
            if segment.read(3, "SegmentType")? != 3 {
                continue;
            }
            let publisher_consents = segment.read(24, "PubPurposesConsent")?;
            let publisher_interests = segment.read(24, "PubPurposesLITransparency")?;
            if publisher_consents & unassigned != 0 {
                warnings.push(format!(
                    "TCF policy {policy} does not assign publisher-consent purpose IDs {} through 24",
                    highest + 1
                ));
            }
            if publisher_interests & unassigned != 0 {
                warnings.push(format!(
                    "TCF policy {policy} does not assign publisher legitimate-interest purpose IDs {} through 24",
                    highest + 1
                ));
            }
            // Publisher purposes 3..6 may use legitimate interest. The
            // November 2023 FAQ explicitly excludes PublisherTC from the
            // vendor-policy legal-basis removal. Custom vectors are separate
            // IDs and receive no global-purpose assignment cap.
        }
    }
    Ok(warnings)
}

pub(crate) fn validate_tcf_v2(value: &str) -> Result<(), String> {
    let mut segments = value.split('.');
    let mut core = Bits::new(segments.next().unwrap_or_default(), "core")?;
    if core.read(6, "Version")? != 2 {
        return Err("TCF core Version must be 2".to_string());
    }
    let created = core.read_u64(36, "Created")?;
    core.skip(36, "LastUpdated")?;
    core.skip(12, "CmpId")?;
    core.skip(12, "CmpVersion")?;
    core.skip(6, "ConsentScreen")?;
    core.letters("ConsentLanguage")?;
    core.skip(12, "VendorListVersion")?;
    core.skip(6, "TcfPolicyVersion")?;
    core.skip(1, "IsServiceSpecific")?;
    core.skip(1, "UseNonStandardTexts")?;
    core.skip(12, "SpecialFeatureOptIns")?;
    core.skip(24, "PurposesConsent")?;
    core.skip(24, "PurposesLITransparency")?;
    core.skip(1, "PurposeOneTreatment")?;
    core.letters("PublisherCC")?;
    core.vendors("VendorConsents")?;
    core.vendors("VendorLegitimateInterests")?;
    let restrictions = core.read(12, "NumPubRestrictions")?;
    for _ in 0..restrictions {
        if core.read(6, "PublisherRestriction.PurposeId")? == 0 {
            return Err("TCF publisher restriction PurposeId must be positive".to_string());
        }
        if core.read(2, "PublisherRestriction.RestrictionType")? == 3 {
            return Err("TCF publisher restriction RestrictionType 3 is undefined".to_string());
        }
        let entries = core.read(12, "PublisherRestriction.NumEntries")?;
        core.ranges(entries, u16::MAX as usize, "PublisherRestriction")?;
    }
    core.finish()?;

    let mut seen = [false; 4];
    for encoded in segments {
        let mut segment = Bits::new(encoded, "additional segment")?;
        let kind = segment.read(3, "SegmentType")?;
        if !(1..=3).contains(&kind) {
            return Err(format!("TCF additional SegmentType {kind} is undefined"));
        }
        if seen[kind] {
            return Err(format!(
                "TCF SegmentType {kind} must not occur more than once"
            ));
        }
        seen[kind] = true;
        match kind {
            1 => segment.vendors("DisclosedVendors")?,
            2 => segment.vendors("AllowedVendors")?,
            3 => {
                segment.skip(24, "PublisherConsents")?;
                segment.skip(24, "PublisherLegitimateInterests")?;
                let purposes = segment.read(6, "NumCustomPurposes")?;
                segment.skip(purposes, "CustomPurposesConsent")?;
                segment.skip(purposes, "CustomPurposesLegitimateInterests")?;
            }
            _ => unreachable!(),
        }
        segment.finish()?;
    }
    if created >= DISCLOSURE_REQUIRED_FROM_DECISECONDS && !seen[1] {
        return Err(
            "TCF strings created after 2026-02-28 require a DisclosedVendors segment".to_string(),
        );
    }
    Ok(())
}

struct Bits<'a> {
    encoded: &'a [u8],
    position: usize,
    length: usize,
    label: &'a str,
}

impl<'a> Bits<'a> {
    fn new(encoded: &'a str, label: &'a str) -> Result<Self, String> {
        if encoded.is_empty() || !encoded.bytes().all(|byte| sextet(byte).is_some()) {
            return Err(format!(
                "TCF {label} must contain unpadded base64url characters"
            ));
        }
        let length = encoded
            .len()
            .checked_mul(6)
            .ok_or_else(|| format!("TCF {label} is too large"))?;
        Ok(Self {
            encoded: encoded.as_bytes(),
            position: 0,
            length,
            label,
        })
    }

    fn bit(&self, position: usize) -> usize {
        let value = sextet(self.encoded[position / 6]).unwrap();
        ((value >> (5 - position % 6)) & 1) as usize
    }

    fn read(&mut self, count: usize, field: &str) -> Result<usize, String> {
        self.ensure(count, field)?;
        let mut value = 0;
        for position in self.position..self.position + count {
            value = (value << 1) | self.bit(position);
        }
        self.position += count;
        Ok(value)
    }

    fn read_u64(&mut self, count: usize, field: &str) -> Result<u64, String> {
        self.ensure(count, field)?;
        let mut value = 0_u64;
        for position in self.position..self.position + count {
            value = (value << 1) | self.bit(position) as u64;
        }
        self.position += count;
        Ok(value)
    }

    fn skip(&mut self, count: usize, field: &str) -> Result<(), String> {
        self.ensure(count, field)?;
        self.position += count;
        Ok(())
    }

    fn ensure(&self, count: usize, field: &str) -> Result<(), String> {
        if count > self.length - self.position {
            return Err(format!("TCF {} is truncated in {field}", self.label));
        }
        Ok(())
    }

    fn letters(&mut self, field: &str) -> Result<(), String> {
        for _ in 0..2 {
            if self.read(6, field)? > 25 {
                return Err(format!(
                    "TCF {field} must encode two letters from A through Z"
                ));
            }
        }
        Ok(())
    }

    fn vendors(&mut self, field: &str) -> Result<(), String> {
        let max = self.read(16, &format!("{field}.MaxVendorId"))?;
        let range = self.read(1, &format!("{field}.IsRangeEncoding"))?;
        if range == 0 {
            self.skip(max, &format!("{field}.BitField"))
        } else {
            let entries = self.read(12, &format!("{field}.NumEntries"))?;
            self.ranges(entries, max, field)
        }
    }

    fn vendor_enabled(&mut self, vendor_id: u16, field: &str) -> Result<bool, String> {
        let vendor_id = vendor_id as usize;
        let max = self.read(16, &format!("{field}.MaxVendorId"))?;
        let range = self.read(1, &format!("{field}.IsRangeEncoding"))?;
        if vendor_id > max {
            return Ok(false);
        }
        if range == 0 {
            self.skip(vendor_id - 1, &format!("{field}.BitField"))?;
            return Ok(self.read(1, &format!("{field}.Consent"))? == 1);
        }
        let entries = self.read(12, &format!("{field}.NumEntries"))?;
        let mut enabled = false;
        for _ in 0..entries {
            let range = self.read(1, &format!("{field}.IsARange"))?;
            let start = self.read(16, &format!("{field}.StartOrOnlyVendorId"))?;
            let end = if range == 1 {
                self.read(16, &format!("{field}.EndVendorId"))?
            } else {
                start
            };
            enabled |= (start..=end).contains(&vendor_id);
        }
        Ok(enabled)
    }

    fn ranges(&mut self, entries: usize, max: usize, field: &str) -> Result<(), String> {
        for _ in 0..entries {
            let range = self.read(1, &format!("{field}.IsARange"))?;
            let start = self.read(16, &format!("{field}.StartOrOnlyVendorId"))?;
            if start == 0 || start > max {
                return Err(format!(
                    "TCF {field} vendor ID must be from 1 through {max}"
                ));
            }
            if range == 1 {
                let end = self.read(16, &format!("{field}.EndVendorId"))?;
                if end <= start || end > max {
                    return Err(format!(
                        "TCF {field} range end must exceed its start and not exceed {max}"
                    ));
                }
            }
        }
        Ok(())
    }

    fn finish(&self) -> Result<(), String> {
        // The official SDK pads bit strings to the next multiple of 24 bits.
        // Shorter byte/sextet padding is also used by existing vendor examples.
        let padding = self.length - self.position;
        if padding >= 24 || (self.position..self.length).any(|position| self.bit(position) != 0) {
            return Err(format!(
                "TCF {} has unexpected trailing section data",
                self.label
            ));
        }
        Ok(())
    }
}

fn sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Default)]
    struct Builder(Vec<u8>);

    impl Builder {
        fn push(&mut self, value: usize, count: usize) {
            self.0
                .extend((0..count).rev().map(|i| ((value >> i) & 1) as u8));
        }

        fn zeroes(&mut self, count: usize) {
            self.0.resize(self.0.len() + count, 0);
        }

        fn set_created(&mut self, deciseconds: u64) {
            for index in 0..36 {
                self.0[6 + index] = ((deciseconds >> (35 - index)) & 1) as u8;
            }
        }

        fn encoded(&self, multiple: usize) -> String {
            let mut bits = self.0.clone();
            bits.resize(bits.len().next_multiple_of(multiple), 0);
            bits.chunks(6)
                .map(|chunk| ALPHABET[chunk.iter().fold(0, |v, b| (v << 1) | b) as usize] as char)
                .collect()
        }

        fn vector(&mut self, max: usize, ranges: Option<&[(usize, Option<usize>)]>) {
            self.push(max, 16);
            self.push(usize::from(ranges.is_some()), 1);
            if let Some(ranges) = ranges {
                self.entries(ranges);
            } else {
                self.zeroes(max);
            }
        }

        fn entries(&mut self, ranges: &[(usize, Option<usize>)]) {
            self.push(ranges.len(), 12);
            for &(start, end) in ranges {
                self.push(usize::from(end.is_some()), 1);
                self.push(start, 16);
                if let Some(end) = end {
                    self.push(end, 16);
                }
            }
        }
    }

    fn prefix() -> Builder {
        let mut bits = Builder::default();
        bits.push(2, 6);
        bits.zeroes(207);
        bits
    }

    fn core() -> Builder {
        let mut bits = prefix();
        bits.vector(0, None);
        bits.vector(0, None);
        bits.push(0, 12);
        bits
    }

    fn extra_vector(kind: usize, max: usize, ranges: Option<&[(usize, Option<usize>)]>) -> String {
        let mut bits = Builder::default();
        bits.push(kind, 3);
        bits.vector(max, ranges);
        bits.encoded(24)
    }

    fn publisher(custom: usize) -> Builder {
        let mut bits = Builder::default();
        bits.push(3, 3);
        bits.zeroes(48);
        bits.push(custom, 6);
        bits.zeroes(custom * 2);
        bits
    }

    #[test]
    fn published_iab_and_vendor_examples_are_structurally_valid() {
        for value in [
            "CQSbk4AQSbk4ANwAAAENAwCgAAAAAAAAAAYgACPAAAAA.IDKQA4AAgAKAGQAygAAA.YAAAAAAAAAAA",
            "CO052l-O052l-DGAMBFRACBgAIBAAAAABIYgEawAQEagAAAA",
            "CQd924AQd924AASACCENCNFsAP_gAEIAACiQL6QBAAGAAOANmAcAF9IAIADgAA.IL6AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA.YIAAAAAAAAAA",
        ] {
            assert_eq!(validate_tcf_v2(value), Ok(()), "{value}");
        }
    }

    #[test]
    fn valid_encoding_boundaries_preserve_historical_segments() {
        for multiple in [6, 24] {
            assert_eq!(validate_tcf_v2(&core().encoded(multiple)), Ok(()));
        }
        let mut bits = prefix();
        bits.vector(65535, Some(&[(65535, None), (1, Some(65534))]));
        bits.vector(0, Some(&[]));
        bits.push(1, 12);
        bits.push(63, 6);
        bits.push(2, 2);
        bits.entries(&[(12, Some(24)), (65535, None)]);
        let encoded = bits.encoded(24);
        for custom in [0, 1, 63] {
            let value = format!(
                "{encoded}.{}.{}.{}",
                publisher(custom).encoded(24),
                extra_vector(2, 65535, Some(&[(1, Some(65535))])),
                extra_vector(1, 3, None)
            );
            assert_eq!(validate_tcf_v2(&value), Ok(()));
        }
        let mut max = prefix();
        max.vector(65535, None);
        max.vector(65535, None);
        max.push(0, 12);
        assert_eq!(validate_tcf_v2(&max.encoded(24)), Ok(()));
    }

    #[test]
    fn malformed_ranges_and_restrictions_are_rejected() {
        for ranges in [
            &[(0, None)][..],
            &[(11, None)],
            &[(4, Some(3))],
            &[(4, Some(4))],
            &[(4, Some(11))],
        ] {
            let mut bits = prefix();
            bits.vector(10, Some(ranges));
            bits.vector(0, None);
            bits.push(0, 12);
            assert!(validate_tcf_v2(&bits.encoded(24)).is_err());
            for kind in [1, 2] {
                let value = format!(
                    "{}.{}",
                    core().encoded(24),
                    extra_vector(kind, 10, Some(ranges))
                );
                assert!(validate_tcf_v2(&value).is_err());
            }
        }
        for (purpose, restriction) in [(0, 0), (1, 3)] {
            let mut bits = prefix();
            bits.vector(0, None);
            bits.vector(0, None);
            bits.push(1, 12);
            bits.push(purpose, 6);
            bits.push(restriction, 2);
            bits.entries(&[(1, None)]);
            assert!(validate_tcf_v2(&bits.encoded(24)).is_err());
        }
    }

    #[test]
    fn truncated_fields_cannot_consume_missing_sections() {
        let full = core().encoded(6);
        for end in 1..full.len() {
            assert!(validate_tcf_v2(&full[..end]).is_err(), "core prefix {end}");
        }
        for kind in [1, 2] {
            let full = extra_vector(kind, 400, Some(&[(1, Some(400))]));
            let value = format!("{}.{}", core().encoded(24), &full[..full.len() - 5]);
            assert!(validate_tcf_v2(&value).unwrap_err().contains("truncated"));
            let full = extra_vector(kind, 100, None);
            let value = format!("{}.{}", core().encoded(24), &full[..full.len() - 5]);
            assert!(validate_tcf_v2(&value).unwrap_err().contains("truncated"));
        }
        let full = publisher(63).encoded(6);
        let value = format!("{}.{}", core().encoded(24), &full[..full.len() - 1]);
        assert!(validate_tcf_v2(&value).unwrap_err().contains("truncated"));
        let mut restrictions = prefix();
        restrictions.vector(0, None);
        restrictions.vector(0, None);
        restrictions.push(1, 12);
        restrictions.push(1, 6);
        restrictions.push(1, 2);
        restrictions.push(1, 12);
        restrictions.push(1, 1);
        restrictions.push(12, 16);
        assert!(
            validate_tcf_v2(&restrictions.encoded(6))
                .unwrap_err()
                .contains("EndVendorId")
        );
    }

    #[test]
    fn disclosure_transition_uses_created_deciseconds_without_policy_assumptions() {
        for created in [0, DISCLOSURE_REQUIRED_FROM_DECISECONDS - 1] {
            let mut value = core();
            value.set_created(created);
            // Policy version 5 also existed before the technical transition.
            value.0[132..138].copy_from_slice(&[0, 0, 0, 1, 0, 1]);
            assert_eq!(validate_tcf_v2(&value.encoded(24)), Ok(()));
        }
        for created in [DISCLOSURE_REQUIRED_FROM_DECISECONDS, (1_u64 << 36) - 1] {
            let mut value = core();
            value.set_created(created);
            let encoded = value.encoded(24);
            assert!(
                validate_tcf_v2(&encoded)
                    .unwrap_err()
                    .contains("require a DisclosedVendors")
            );
            assert_eq!(
                validate_tcf_v2(&format!("{encoded}.{}", extra_vector(1, 0, None))),
                Ok(())
            );
            assert!(validate_tcf_v2(&format!("{encoded}.{}", extra_vector(2, 0, None))).is_err());
        }
        let mut created = prefix();
        created.0.truncate(35);
        assert!(
            validate_tcf_v2(&created.encoded(6))
                .unwrap_err()
                .contains("Created")
        );
    }

    #[test]
    fn policy_advisories_map_published_requirements_without_changing_structure() {
        let mut value = core();
        value.0[138] = 1;
        value.0[132..138].copy_from_slice(&[0, 0, 0, 1, 0, 0]);
        assert!(tcf_policy_warnings(&value.encoded(24)).is_empty());
        value.0[138] = 0;
        assert!(tcf_policy_warnings(&value.encoded(24))[0].contains("global scope"));
        value.0[138] = 1;
        for index in 178..182 {
            value.0[index] = 1;
            let warnings = tcf_policy_warnings(&value.encoded(24));
            assert_eq!(warnings.len(), 1);
            assert!(warnings[0].contains("purposes 3 through 6"));
            value.0[index] = 0;
        }
        // The adjacent purpose 2/7 bits are not newly reserved by TCF 2.2.
        value.0[177] = 1;
        value.0[182] = 1;
        assert!(tcf_policy_warnings(&value.encoded(24)).is_empty());
        value.set_created(10);
        let warnings = tcf_policy_warnings(&value.encoded(24));
        assert!(warnings.iter().any(|v| v.contains("same day-level")));
        assert!(warnings.iter().any(|v| v.contains("UTC midnight")));
        assert_eq!(validate_tcf_v2(&value.encoded(24)), Ok(()));
        assert!(tcf_policy_warnings("CA").is_empty());
    }

    #[test]
    fn policy_floor_preserves_legacy_boundary_and_reports_current_source_conflict() {
        let mut value = core();
        value.0[138] = 1;
        value.0[132..138].copy_from_slice(&[0, 0, 0, 0, 1, 1]);
        value.set_created(POLICY_FOUR_REQUIRED_FROM_DECISECONDS - 1);
        assert!(tcf_policy_warnings(&value.encoded(24)).is_empty());
        value.set_created(POLICY_FOUR_REQUIRED_FROM_DECISECONDS);
        assert_eq!(tcf_policy_warnings(&value.encoded(24)).len(), 1);
        // A current primary IAB specification example has a 2025 creation date
        // and policy 2. Its valid encoding and policy advisory stay distinct.
        let current =
            "CQSbk4AQSbk4ANwAAAENAwCgAAAAAAAAAAYgACPAAAAA.IDKQA4AAgAKAGQAygAAA.YAAAAAAAAAAA";
        assert_eq!(validate_tcf_v2(current), Ok(()));
        assert_eq!(tcf_policy_warnings(current).len(), 1);
        let historical =
            "CPUCbs9PUDXghADABCENCBCoAP_AAEJAAAAADGwBAAGABPADCAY0BjYAgADAAngBhAMaAAA.YAAAAAAAA4AA";
        assert!(tcf_policy_warnings(historical).is_empty());
        // Nonstandard texts are expressly allowed, not a reserved zero bit.
        value.0[139] = 1;
        value.set_created(0);
        assert!(tcf_policy_warnings(&value.encoded(24)).is_empty());
    }

    #[test]
    fn dated_policy_assignments_preserve_purpose_eleven_and_unknown_versions() {
        for (policy, highest) in [(1, 10), (2, 10), (3, 10), (4, 11), (5, 11)] {
            let mut value = core();
            value.0[138] = 1;
            for (offset, bit) in value.0[132..138].iter_mut().enumerate() {
                *bit = ((policy >> (5 - offset)) & 1) as u8;
            }
            value.0[152 + highest - 1] = 1;
            value.0[176 + highest - 1] = 1;
            value.0[141] = 1;
            assert!(tcf_policy_warnings(&value.encoded(24)).is_empty());
            for purpose in [highest + 1, 24] {
                value.0[152 + purpose - 1] = 1;
                let warnings = tcf_policy_warnings(&value.encoded(24));
                assert_eq!(warnings.len(), 1);
                assert!(warnings[0].contains("purpose-consent IDs"));
                value.0[152 + purpose - 1] = 0;
                value.0[176 + purpose - 1] = 1;
                let warnings = tcf_policy_warnings(&value.encoded(24));
                assert_eq!(warnings.len(), 1);
                assert!(warnings[0].contains("legitimate-interest purpose IDs"));
                value.0[176 + purpose - 1] = 0;
            }
            for feature in [3, 12] {
                value.0[140 + feature - 1] = 1;
                let warnings = tcf_policy_warnings(&value.encoded(24));
                assert_eq!(warnings.len(), 1);
                assert!(warnings[0].contains("special-feature IDs"));
                value.0[140 + feature - 1] = 0;
            }
        }
        for policy in [0, 63] {
            let mut value = core();
            value.0[138] = 1;
            for (offset, bit) in value.0[132..138].iter_mut().enumerate() {
                *bit = ((policy >> (5 - offset)) & 1) as u8;
            }
            value.0[151] = 1;
            value.0[175] = 1;
            value.0[199] = 1;
            assert!(tcf_policy_warnings(&value.encoded(24)).is_empty());
        }
    }

    #[test]
    fn language_assignment_snapshot_is_advisory_and_accepts_every_published_code() {
        let set_language = |value: &mut Builder, code: &str| {
            for (index, letter) in code.bytes().enumerate() {
                for offset in 0..6 {
                    value.0[108 + index * 6 + offset] =
                        ((letter.to_ascii_lowercase() - b'a') >> (5 - offset)) & 1;
                }
            }
        };
        for code in ISO_639_1_CODES {
            let mut value = core();
            value.0[138] = 1;
            set_language(&mut value, code);
            assert!(tcf_policy_warnings(&value.encoded(24)).is_empty(), "{code}");
        }
        for code in ["zz", "qa", "bh"] {
            let mut value = core();
            value.0[138] = 1;
            set_language(&mut value, code);
            let literal = value.encoded(24);
            assert_eq!(validate_tcf_v2(&literal), Ok(()));
            let warnings = tcf_policy_warnings(&literal);
            assert_eq!(warnings.len(), 1);
            assert!(warnings[0].contains("ConsentLanguage"));
            assert!(warnings[0].contains("dated registry"));
        }
    }

    #[test]
    fn publisher_assignments_keep_custom_purposes_and_publisher_li_distinct() {
        for (policy, highest) in [(1, 10), (2, 10), (3, 10), (4, 11), (5, 11)] {
            let mut value = core();
            value.0[138] = 1;
            for (offset, bit) in value.0[132..138].iter_mut().enumerate() {
                *bit = ((policy >> (5 - offset)) & 1) as u8;
            }
            for purpose in [highest, highest + 1, 24] {
                for start in [3, 27] {
                    let mut published = publisher(0);
                    published.0[start + purpose - 1] = 1;
                    let literal = format!("{}.{}", value.encoded(24), published.encoded(24));
                    let warnings = tcf_policy_warnings(&literal);
                    assert_eq!(warnings.len(), usize::from(purpose > highest));
                    if purpose > highest {
                        assert!(warnings[0].contains("publisher"));
                    }
                }
            }
            for purpose in [highest, highest + 1, 63] {
                let mut restricted = value.clone();
                restricted.0.truncate(247);
                restricted.push(1, 12);
                restricted.push(purpose, 6);
                restricted.push(0, 2);
                restricted.entries(&[(97, None)]);
                let warnings = tcf_policy_warnings(&restricted.encoded(24));
                assert_eq!(warnings.len(), usize::from(purpose > highest));
                if purpose > highest {
                    assert!(warnings[0].contains("publisher-restriction"));
                }
            }
            let mut published = publisher(63);
            for bit in &mut published.0[54..] {
                *bit = 1;
            }
            // Standard PublisherLI purposes 3..6 are expressly permitted.
            for purpose in 3..=6 {
                published.0[27 + purpose - 1] = 1;
            }
            assert!(
                tcf_policy_warnings(&format!("{}.{}", value.encoded(24), published.encoded(24)))
                    .is_empty()
            );
        }
        let mut value = core();
        value.0[138] = 1;
        value.0[132..138].fill(1);
        let mut published = publisher(0);
        published.0[26] = 1;
        published.0[50] = 1;
        assert!(
            tcf_policy_warnings(&format!("{}.{}", value.encoded(24), published.encoded(24)))
                .is_empty()
        );
    }

    #[test]
    fn explicit_destination_positive_signals_handle_bitfields_and_ranges() {
        let liveramp =
            "CPSzMzJPTeGtxADABCENB_CoAP_AAEJAAAAADGwBAAGABPADCAY0BjYAgADAAngBhAMaAAA.YAAAAAAAA4AA";
        assert_eq!(tcf_consent_requirements(liveramp, 97, &[1]), Ok(true));
        let generic =
            "CQSbk4AQSbk4ANwAAAENAwCgAAAAAAAAAAYgACPAAAAA.IDKQA4AAgAKAGQAygAAA.YAAAAAAAAAAA";
        assert_eq!(tcf_consent_requirements(generic, 97, &[1]), Ok(false));
        for ranges in [None, Some(&[(97, None), (100, Some(110))][..])] {
            let mut bits = prefix();
            bits.0[152] = 1;
            bits.0[175] = 1;
            bits.vector(110, ranges);
            if ranges.is_none() {
                bits.0[230 + 96] = 1;
                bits.0[230 + 99] = 1;
                bits.0[230 + 109] = 1;
            }
            bits.vector(0, None);
            bits.push(0, 12);
            let encoded = bits.encoded(24);
            assert_eq!(tcf_consent_requirements(&encoded, 97, &[1, 24]), Ok(true));
            assert_eq!(tcf_consent_requirements(&encoded, 97, &[2]), Ok(false));
            assert_eq!(tcf_consent_requirements(&encoded, 98, &[1]), Ok(false));
            assert_eq!(tcf_consent_requirements(&encoded, 100, &[1]), Ok(true));
            assert_eq!(tcf_consent_requirements(&encoded, 110, &[]), Ok(true));
            assert_eq!(tcf_consent_requirements(&encoded, 111, &[1]), Ok(false));
        }
        assert!(tcf_consent_requirements("CA", 97, &[1]).is_err());
        assert!(tcf_consent_requirements(liveramp, 0, &[1]).is_err());
        assert!(tcf_consent_requirements(liveramp, 97, &[0]).is_err());
        assert!(tcf_consent_requirements(liveramp, 97, &[25]).is_err());
    }

    #[test]
    fn invalid_segment_ids_duplicates_alphabet_and_trailing_data_are_rejected() {
        let encoded_core = core().encoded(24);
        let disclosed = extra_vector(1, 0, None);
        assert!(validate_tcf_v2(&format!("{encoded_core}.{disclosed}.{disclosed}")).is_err());
        for kind in [0, 4, 5, 6, 7] {
            assert!(
                validate_tcf_v2(&format!("{encoded_core}.{}", extra_vector(kind, 0, None)))
                    .is_err()
            );
        }
        for bad in ["", "A=", "A+", "A/", "é", ".", "C"] {
            assert!(validate_tcf_v2(bad).is_err());
        }
        for tail in ["B", "AAAA"] {
            assert!(validate_tcf_v2(&format!("{encoded_core}{tail}")).is_err());
        }
        let mut bad = core().0;
        bad[108] = 1;
        bad[109] = 1;
        assert!(validate_tcf_v2(&Builder(bad).encoded(24)).is_err());
        let mut bad = core().0;
        bad[201] = 1;
        bad[202] = 1;
        assert!(validate_tcf_v2(&Builder(bad).encoded(24)).is_err());
    }
}
