//! GPP header correspondence and published discrete-section structures.
//!
//! Sources: IAB Global-Privacy-Platform/Core/Consent String Specification.md and
//! official iabgpp-es encoder tree fd0546e7b28e7e186bc27aac6375e6c10a80e47e.
//! These checks validate encoding. Consent permissions and account policies
//! require the caller's policy and the applicable dated vendor lists.

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GppStructure {
    pub section_ids: Vec<u32>,
    pub unvalidated_sections: Vec<u32>,
    pub unknown_header_version: Option<u8>,
}

#[derive(Debug)]
pub(crate) struct DecodedUsSection {
    pub section_id: u32,
    pub version: u8,
    pub fields: std::collections::BTreeMap<String, Vec<u8>>,
    pub legacy_headerless: bool,
}

pub(crate) fn decoded_us_sections(value: &str) -> Vec<DecodedUsSection> {
    let Ok(structure) = validate_gpp_structure(value) else {
        return Vec::new();
    };
    structure
        .section_ids
        .iter()
        .zip(value.split('~').skip(1))
        .filter_map(|(&id, section)| {
            if structure.unvalidated_sections.contains(&id) {
                return None;
            }
            let layout = us_layout(id)?;
            let parts: Vec<_> = section.split('.').collect();
            let has_header = has_us_section_header(id, &parts);
            let core_value = parts[usize::from(has_header)];
            let mut core = Bits::new(core_value).ok()?;
            let version = core.read(6, "Version").ok()? as u8;
            let mut fields = std::collections::BTreeMap::new();
            fields.insert(
                if id >= 24 { "MspaVersion" } else { "Version" }.to_string(),
                vec![version],
            );
            for field in layout.fields {
                let count = if id == 7 && core.bits.len() <= 66 {
                    match field.name {
                        "SensitiveDataProcessing" => 12,
                        "KnownChildSensitiveDataConsents" => 2,
                        _ => field.count,
                    }
                } else {
                    field.count
                };
                let values = (0..count)
                    .map(|_| core.read(field.width, field.name).map(|v| v as u8))
                    .collect::<Result<Vec<_>, _>>()
                    .ok()?;
                fields.insert(field.name.to_string(), values);
            }
            Some(DecodedUsSection {
                section_id: id,
                version,
                fields,
                legacy_headerless: id >= 24 && !has_header,
            })
        })
        .collect()
}

pub(crate) fn us_field_supported(name: &str) -> bool {
    matches!(name, "Version" | "MspaVersion")
        || (7..=27).any(|id| {
            us_layout(id).is_some_and(|layout| layout.fields.iter().any(|field| field.name == name))
        })
}

pub(crate) fn validate_gpp_structure(value: &str) -> Result<GppStructure, String> {
    let pieces: Vec<_> = value.split('~').collect();
    if pieces.iter().any(|piece| piece.is_empty()) {
        return Err("GPP has an empty header or discrete section".into());
    }
    for section in &pieces[1..] {
        for part in section.split('.') {
            Bits::new(part)?;
        }
    }
    let mut header = Bits::new(pieces[0])?;
    if header.read(6, "Header.Type")? != 3 {
        return Err("GPP header type must be 3".into());
    }
    let version = header.read(6, "Header.Version")? as u8;
    if version != 1 {
        return Ok(GppStructure {
            section_ids: Vec::new(),
            unvalidated_sections: Vec::new(),
            unknown_header_version: Some(version),
        });
    }
    let ids = header.fibonacci_ranges(u32::MAX, pieces.len() - 1, "Header.SectionIds")?;
    header.finish(false, "Header")?;
    if ids.len() != pieces.len() - 1 {
        return Err(format!(
            "GPP header declares {} discrete sections but the string contains {}",
            ids.len(),
            pieces.len() - 1
        ));
    }
    let mut unvalidated = Vec::new();
    for (&id, section) in ids.iter().zip(&pieces[1..]) {
        match id {
            2 => {
                let mut core = Bits::new(section.split('.').next().unwrap_or_default())?;
                if core.read(6, "TCF.Version")? != 2 {
                    unvalidated.push(id);
                } else {
                    crate::tcf_sections::validate_tcf_v2(section)
                        .map_err(|reason| format!("GPP section 2: {reason}"))?;
                }
            }
            3 => {
                return Err(
                    "GPP header section ID 3 cannot be repeated as a discrete section".into(),
                );
            }
            5 => {
                if !validate_canada(section)? {
                    unvalidated.push(id);
                }
            }
            6 => {
                let bytes = section.as_bytes();
                if bytes.len() != 4
                    || bytes[0] != b'1'
                    || bytes[1..].iter().any(|b| !matches!(b, b'Y' | b'N' | b'-'))
                {
                    return Err(
                        "GPP US Privacy section must be 1 followed by three Y, N or - choices"
                            .into(),
                    );
                }
            }
            7..=27 => {
                if !validate_us_section(id, section)? {
                    unvalidated.push(id);
                }
            }
            _ => unvalidated.push(id),
        }
    }
    Ok(GppStructure {
        section_ids: ids,
        unvalidated_sections: unvalidated,
        unknown_header_version: None,
    })
}

pub(crate) fn gpp_sid_mismatch(value: &str, structure: &GppStructure) -> Option<String> {
    if structure.unknown_header_version.is_some() || value == "-1" {
        return None;
    }
    for field in value.split(',') {
        let Ok(id) = field.parse::<u32>() else {
            return Some(
                "Applicable GPP section IDs must be positive integers, or -1 alone".into(),
            );
        };
        if !structure.section_ids.contains(&id) {
            return Some(format!(
                "Applicable GPP section ID {id} is absent from the encoded header"
            ));
        }
    }
    None
}

pub(crate) fn gpp_policy_warnings(value: &str) -> Vec<String> {
    let Ok(structure) = validate_gpp_structure(value) else {
        return Vec::new();
    };
    let mut warnings = Vec::new();
    for (&id, section) in structure.section_ids.iter().zip(value.split('~').skip(1)) {
        if id != 5 || structure.unvalidated_sections.contains(&id) {
            continue;
        }
        let Ok(mut core) = Bits::new(section.split('.').next().unwrap_or_default()) else {
            continue;
        };
        let _ = core.read(6, "Version");
        for field in ["Created", "LastUpdated"] {
            if let Ok(date) = core.read(36, field)
                && date % 864_000 != 0
            {
                warnings.push(format!("GPP Canada {field} is not rounded to a full UTC day as its section specification requires; the CMP API includes conflicting daytime examples"));
            }
        }
        if core
            .skip(12 + 12 + 6 + 12 + 12 + 6 + 1 + 12, "Canada.Metadata")
            .is_ok()
        {
            for field in ["PurposesExpressConsent", "PurposesImpliedConsent"] {
                if core.read(1, field) == Ok(1) {
                    warnings.push(format!("GPP Canada {field} sets purpose 1, which the published Canadian section definition says is unused; official encoder examples also set it"));
                }
                let _ = core.skip(23, field);
            }
        }
    }
    for section in decoded_us_sections(value) {
        let id = section.section_id;
        if section.legacy_headerless {
            warnings.push(format!("GPP section {id} uses the official SDK's legacy headerless form; its April 2026 specification requires a separate section header"));
        }
        let fields = section.fields;
        if id == 7
            && section.version == 2
            && fields
                .get("SensitiveDataProcessing")
                .is_some_and(|v| v.len() == 12)
        {
            warnings.push("GPP US National version2 uses the SDK's legacy 12-category layout instead of the current 16-category/3-child layout; confirm the intended technical version".into());
        }
        for (notice, choice) in [
            ("SaleOptOutNotice", "SaleOptOut"),
            ("SharingOptOutNotice", "SharingOptOut"),
            (
                "TargetedAdvertisingOptOutNotice",
                "TargetedAdvertisingOptOut",
            ),
        ] {
            let Some(notice_value) = fields.get(notice).and_then(|v| v.first()) else {
                continue;
            };
            let Some(choice_value) = fields.get(choice).and_then(|v| v.first()) else {
                continue;
            };
            if *notice_value != 1 && *choice_value != 0 {
                warnings.push(format!("GPP section {id} {choice} is nonzero although {notice} is not 1 (notice provided); the published choice definition specifies 0 for an inapplicable or unprovided notice"));
            } else if *notice_value == 1 && *choice_value == 0 {
                warnings.push(format!("GPP section {id} {choice} is 0 (inapplicable or unprovided notice) although {notice} is 1 (notice provided); confirm these published definitions describe the same transaction"));
            }
        }
        if id == 8
            && fields
                .get("SensitiveDataLimitUseNotice")
                .and_then(|v| v.first())
                != Some(&1)
            && fields
                .get("SensitiveDataProcessing")
                .is_some_and(|v| v.iter().any(|n| *n != 0))
        {
            warnings.push("GPP California SensitiveDataProcessing contains nonzero choices although SensitiveDataLimitUseNotice is not 1; the published choice definition specifies 0 for an inapplicable or unprovided notice".into());
        }
    }
    warnings
}

#[derive(Clone)]
struct Bits {
    bits: Vec<u8>,
    position: usize,
}

impl Bits {
    fn new(value: &str) -> Result<Self, String> {
        if value.is_empty() {
            return Err("GPP has an empty encoded segment".into());
        }
        let mut bits = Vec::with_capacity(value.len().saturating_mul(6));
        for byte in value.bytes() {
            let n = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'-' => 62,
                b'_' => 63,
                _ => {
                    return Err(
                        "GPP segment contains a character outside its URL-safe alphabet".into(),
                    );
                }
            };
            for shift in (0..6).rev() {
                bits.push((n >> shift) & 1);
            }
        }
        Ok(Self { bits, position: 0 })
    }

    fn read(&mut self, width: usize, field: &str) -> Result<u64, String> {
        if width > 64 || self.bits.len().saturating_sub(self.position) < width {
            return Err(format!("GPP segment is truncated in {field}"));
        }
        let mut n = 0;
        for &bit in &self.bits[self.position..self.position + width] {
            n = (n << 1) | u64::from(bit);
        }
        self.position += width;
        Ok(n)
    }

    fn skip(&mut self, width: usize, field: &str) -> Result<(), String> {
        if self.bits.len().saturating_sub(self.position) < width {
            return Err(format!("GPP segment is truncated in {field}"));
        }
        self.position += width;
        Ok(())
    }

    fn fibonacci(&mut self, field: &str) -> Result<u32, String> {
        let (mut current, mut next) = (1u64, 2u64);
        let (mut sum, mut previous) = (0u64, 0u64);
        loop {
            let bit = self.read(1, field)?;
            if bit == 1 && previous == 1 {
                return u32::try_from(sum).map_err(|_| {
                    format!("GPP {field} Fibonacci value exceeds supported integer range")
                });
            }
            if bit == 1 {
                sum = sum
                    .checked_add(current)
                    .ok_or_else(|| format!("GPP {field} Fibonacci value overflows"))?;
            }
            let following = current
                .checked_add(next)
                .ok_or_else(|| format!("GPP {field} Fibonacci code has no finite terminator"))?;
            current = next;
            next = following;
            previous = bit;
        }
    }

    fn fibonacci_ranges(
        &mut self,
        max_value: u32,
        max_count: usize,
        field: &str,
    ) -> Result<Vec<u32>, String> {
        let count = self.read(12, field)? as usize;
        if count > max_count {
            return Err(format!(
                "GPP {field} range count exceeds the declared section or vendor capacity"
            ));
        }
        let mut output = Vec::new();
        let mut previous = 0u32;
        for _ in 0..count {
            let group = self.read(1, field)? == 1;
            let start = previous
                .checked_add(self.fibonacci(field)?)
                .ok_or_else(|| format!("GPP {field} range offset overflows"))?;
            let end = if group {
                start
                    .checked_add(self.fibonacci(field)?)
                    .ok_or_else(|| format!("GPP {field} range length overflows"))?
            } else {
                start
            };
            let length = (u64::from(end) - u64::from(start) + 1) as usize;
            if start == 0
                || start <= previous
                || end > max_value
                || length > max_count.saturating_sub(output.len())
            {
                return Err(format!(
                    "GPP {field} range IDs exceed the declared section or vendor capacity"
                ));
            }
            output.extend(start..=end);
            previous = end;
        }
        Ok(output)
    }

    fn fixed_ranges(&mut self, max: u32, field: &str) -> Result<(), String> {
        let count = self.read(12, field)?;
        for _ in 0..count {
            let group = self.read(1, field)? == 1;
            let start = self.read(16, field)? as u32;
            let end = if group {
                self.read(16, field)? as u32
            } else {
                start
            };
            if start == 0 || end < start || end > max {
                return Err(format!("GPP {field} contains an invalid vendor ID range"));
            }
        }
        Ok(())
    }

    fn optimized_range(&mut self, legacy: bool, field: &str) -> Result<(), String> {
        let max = self.read(16, field)? as u32;
        if self.read(1, field)? == 0 {
            self.skip(max as usize, field)
        } else if legacy {
            self.fixed_ranges(max, field)
        } else {
            self.fibonacci_ranges(max, max as usize, field).map(|_| ())
        }
    }

    fn finish(&self, legacy_padding: bool, field: &str) -> Result<(), String> {
        let minimal = self.position.div_ceil(6) * 6;
        // The official SDK pads to a byte first, then a six-bit character.
        let sdk = (self.position.div_ceil(8) * 8).div_ceil(6) * 6;
        let legacy = self.position.div_ceil(24) * 24;
        if self.bits[self.position..].iter().any(|&bit| bit != 0)
            || (self.bits.len() != minimal
                && self.bits.len() != sdk
                && (!legacy_padding || self.bits.len() != legacy))
        {
            return Err(format!(
                "GPP {field} has nonzero or excessive trailing padding"
            ));
        }
        Ok(())
    }
}

fn has_us_section_header(id: u32, parts: &[&str]) -> bool {
    if id < 24 {
        return false;
    }
    let Ok(mut first) = Bits::new(parts[0]) else {
        return false;
    };
    let Ok(section_id) = first.read(6, "SectionID") else {
        return false;
    };
    if !(24..=27).contains(&section_id) {
        return false;
    }
    let Ok(next) = first.read(6, "Version") else {
        return false;
    };
    // Headerless MSPA cores have at most 24 bits and CoveredTransaction starts
    // the next six bits with 01 or 10. A native core following a header has four
    // characters; legacy optional GPC/sensitivity segments have at most three.
    first.bits.len() > 24
        || parts.get(1).is_some_and(|part| part.len() > 3)
        || !(16..=47).contains(&next)
}

fn validate_us_section(id: u32, value: &str) -> Result<bool, String> {
    validate_us_section_inner(id, value, true)
}

fn validate_us_section_inner(id: u32, value: &str, allow_header: bool) -> Result<bool, String> {
    let layout = us_layout(id).expect("all published US section IDs have layouts");
    let parts: Vec<_> = value.split('.').collect();
    let mut core = Bits::new(parts[0])?;
    let version = core.read(6, "Version")?;
    // April 2026 MD, IN, KY and RI specifications add a separate section header.
    // The current official SDK still emits headerless MSPA cores, so both primary
    // source forms are recognized. A section header identifies itself by its ID.
    if allow_header && has_us_section_header(id, &parts) {
        if version != u64::from(id) {
            return Err(format!(
                "GPP section {id} header declares a different section ID"
            ));
        }
        if parts.len() < 2 {
            return Err(format!(
                "GPP section {id} header is missing its core subsection"
            ));
        }
        if core.read(6, "SectionHeader.Version")? != 1 {
            return Ok(false);
        }
        let subsections = core.fibonacci_ranges(1, parts.len() - 2, "SectionHeader.SubSections")?;
        core.finish(false, "SectionHeader")?;
        if subsections.len() != parts.len() - 2 {
            return Err(format!(
                "GPP section {id} subsection IDs do not match the encoded subsections"
            ));
        }
        return validate_us_section_inner(id, &parts[1..].join("."), false);
    }
    if id < 24 && version != u64::from(layout.version) && !(id == 7 && version == 1) {
        return Ok(false);
    }
    for field in layout.fields {
        let count = if id == 7 && core.bits.len() <= 66 {
            match field.name {
                "SensitiveDataProcessing" => 12,
                "KnownChildSensitiveDataConsents" => 2,
                _ => field.count,
            }
        } else {
            field.count
        };
        for index in 0..count {
            let n = core.read(field.width, field.name)?;
            if n < field.minimum || n > field.maximum {
                return Err(format!(
                    "GPP section {id} {}[{index}] must be {} through {}",
                    field.name, field.minimum, field.maximum
                ));
            }
        }
    }
    core.finish(false, "US core")?;
    if parts.len() > 1 {
        if (!layout.gpc && layout.sensitive_count == 0) || parts.len() != 2 {
            return Err(format!(
                "GPP section {id} does not define these additional segments"
            ));
        }
        let mut gpc = Bits::new(parts[1])?;
        if layout.sensitive_count > 0 {
            for _ in 0..layout.sensitive_count {
                if gpc.read(2, "SensitiveDataProcessing")? > 2 {
                    return Err(format!(
                        "GPP section {id} sensitive data choices must be 0 through 2"
                    ));
                }
            }
            gpc.finish(false, "SensitiveDataProcessing")?;
            return Ok(true);
        }
        if gpc.read(2, "GpcSegmentType")? != 1 {
            return Err(format!("GPP section {id} GPC segment type must be 1"));
        }
        gpc.read(1, "Gpc")?;
        gpc.finish(false, "GPC")?;
    }
    Ok(true)
}

fn validate_canada(value: &str) -> Result<bool, String> {
    let parts: Vec<_> = value.split('.').collect();
    let core = Bits::new(parts[0])?;
    if core.bits.len() < 6 {
        return Err("GPP Canada core is truncated in Version".into());
    }
    let version = core.clone().read(6, "Version")?;
    if version != 1 {
        return Ok(false);
    }
    let mut seen = std::collections::HashSet::new();
    for (index, part) in parts.iter().enumerate() {
        let mut bits = Bits::new(part)?;
        let kind = if index == 0 {
            0
        } else {
            bits.read(3, "SegmentType")?
        };
        if !seen.insert(kind) {
            return Err("GPP Canada repeats a segment type".into());
        }
        match kind {
            0 => {
                let validate = |legacy| canada_core(Bits::new(part)?, legacy);
                if let Err(first) = validate(false)
                    && validate(true).is_err()
                {
                    return Err(first);
                }
            }
            1 => {
                let validate = |legacy| -> Result<(), String> {
                    let mut candidate = bits.clone();
                    candidate.optimized_range(legacy, "DisclosedVendors")?;
                    candidate.finish(true, "Canada disclosed vendors")
                };
                if let Err(first) = validate(false)
                    && validate(true).is_err()
                {
                    return Err(first);
                }
            }
            3 => {
                bits.skip(48, "PublisherPurposes")?;
                let custom = bits.read(6, "NumCustomPurposes")? as usize;
                bits.skip(custom * 2, "CustomPurposeChoices")?;
                bits.finish(true, "Canada publisher purposes")?;
            }
            _ => return Err("GPP Canada has an undefined additional segment type".into()),
        }
    }
    Ok(true)
}

fn canada_core(mut core: Bits, legacy: bool) -> Result<(), String> {
    core.read(6, "Canada.Version")?;
    core.skip(36 + 36 + 12 + 12 + 6, "Canada.Metadata")?;
    for _ in 0..2 {
        if core.read(6, "ConsentLanguage")? > 25 {
            return Err("GPP Canada consent language must encode two A through Z letters".into());
        }
    }
    core.skip(12 + 6 + 1 + 12 + 24 + 24, "Canada.ConsentFields")?;
    core.optimized_range(legacy, "VendorExpressConsent")?;
    core.optimized_range(legacy, "VendorImpliedConsent")?;
    // The official SDK permits omission of the trailing restrictions field.
    if core.bits.len().saturating_sub(core.position) >= 12 {
        let restrictions = core.read(12, "PubRestrictions.Count")?;
        for _ in 0..restrictions {
            let purpose = core.read(6, "PubRestrictions.Purpose")?;
            let kind = core.read(2, "PubRestrictions.Type")?;
            if purpose == 0 || purpose > 24 || kind > 2 {
                return Err(
                    "GPP Canada publisher restriction purpose/type is outside its defined range"
                        .into(),
                );
            }
            if legacy {
                core.fixed_ranges(u16::MAX.into(), "PubRestrictions.Vendors")?;
            } else {
                core.optimized_range(false, "PubRestrictions.Vendors")?;
            }
        }
    }
    core.finish(true, "Canada core")
}

struct FieldSpec {
    name: &'static str,
    width: usize,
    count: usize,
    minimum: u64,
    maximum: u64,
}
struct UsLayout {
    version: u8,
    gpc: bool,
    sensitive_count: usize,
    fields: &'static [FieldSpec],
}

fn us_layout(id: u32) -> Option<UsLayout> {
    match id {
        7 => Some(UsLayout {
            version: 2,
            gpc: true,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "SharingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SharingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataLimitUseNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SharingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 16,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 3,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "PersonalDataConsents",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        8 => Some(UsLayout {
            version: 1,
            gpc: true,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SharingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataLimitUseNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SharingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 9,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 2,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "PersonalDataConsents",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        9 => Some(UsLayout {
            version: 1,
            gpc: false,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "SharingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 8,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        10 => Some(UsLayout {
            version: 1,
            gpc: true,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "SharingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 7,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        11 => Some(UsLayout {
            version: 1,
            gpc: false,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "SharingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 8,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        12 => Some(UsLayout {
            version: 1,
            gpc: true,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "SharingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 8,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 3,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        13 => Some(UsLayout {
            version: 1,
            gpc: false,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "ProcessingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 8,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 3,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "AdditionalDataProcessingConsent",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        14 => Some(UsLayout {
            version: 1,
            gpc: true,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "SharingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 8,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 3,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "AdditionalDataProcessingConsent",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        15 => Some(UsLayout {
            version: 1,
            gpc: true,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "ProcessingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 11,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 3,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "AdditionalDataProcessingConsent",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        16 => Some(UsLayout {
            version: 1,
            gpc: true,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "ProcessingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 8,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "AdditionalDataProcessingConsent",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        17 => Some(UsLayout {
            version: 1,
            gpc: true,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "ProcessingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 9,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 5,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "AdditionalDataProcessingConsent",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        18 => Some(UsLayout {
            version: 1,
            gpc: true,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "ProcessingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 8,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        19 => Some(UsLayout {
            version: 1,
            gpc: true,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "ProcessingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 8,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "AdditionalDataProcessingConsent",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        20 => Some(UsLayout {
            version: 1,
            gpc: true,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "ProcessingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 8,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 3,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "AdditionalDataProcessingConsent",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        21 => Some(UsLayout {
            version: 1,
            gpc: true,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "ProcessingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 10,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 5,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "AdditionalDataProcessingConsent",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        22 => Some(UsLayout {
            version: 1,
            gpc: true,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "ProcessingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 8,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "AdditionalDataProcessingConsent",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        23 => Some(UsLayout {
            version: 1,
            gpc: true,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "ProcessingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SensitiveDataProcessing",
                    width: 2,
                    count: 8,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "AdditionalDataProcessingConsent",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaOptOutOptionMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaServiceProviderMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        24 => Some(UsLayout {
            version: 1,
            gpc: true,
            sensitive_count: 0,
            fields: &[
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "ProcessingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "AdditionalDataProcessingConsent",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        25 => Some(UsLayout {
            version: 1,
            gpc: false,
            sensitive_count: 8,
            fields: &[
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "ProcessingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "AdditionalDataProcessingConsent",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        26 => Some(UsLayout {
            version: 1,
            gpc: false,
            sensitive_count: 8,
            fields: &[
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "ProcessingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "AdditionalDataProcessingConsent",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        27 => Some(UsLayout {
            version: 1,
            gpc: false,
            sensitive_count: 8,
            fields: &[
                FieldSpec {
                    name: "MspaCoveredTransaction",
                    width: 2,
                    count: 1,
                    minimum: 1,
                    maximum: 2,
                },
                FieldSpec {
                    name: "MspaMode",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "ProcessingNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOutNotice",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "SaleOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "TargetedAdvertisingOptOut",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "KnownChildSensitiveDataConsents",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
                FieldSpec {
                    name: "AdditionalDataProcessingConsent",
                    width: 2,
                    count: 1,
                    minimum: 0,
                    maximum: 2,
                },
            ],
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../../fixtures/privacy-gpp-source-cases.json"
        ))
        .unwrap()
    }

    fn mutate(value: &str, start: usize, width: usize, replacement: u64) -> String {
        let mut bits = Bits::new(value).unwrap().bits;
        assert!(start + width <= bits.len());
        for index in 0..width {
            bits[start + index] = ((replacement >> (width - index - 1)) & 1) as u8;
        }
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        bits.chunks(6)
            .map(|chunk| {
                let n = chunk
                    .iter()
                    .fold(0usize, |n, bit| (n << 1) | usize::from(*bit));
                ALPHABET[n] as char
            })
            .collect()
    }

    #[test]
    fn official_current_and_legacy_section_examples() {
        let fixtures = corpus();
        for case in fixtures["section_cases"].as_array().unwrap() {
            let id = case["section_id"].as_u64().unwrap() as u32;
            let value = case["value"].as_str().unwrap();
            let result = if id == 5 {
                validate_canada(value)
            } else {
                validate_us_section(id, value)
            };
            assert_eq!(result, Ok(true), "{}: {}", case["id"], case["source"]);
        }
    }

    #[test]
    fn official_full_gpp_examples_and_applicable_subsets() {
        let fixtures = corpus();
        for case in fixtures["gpp_cases"].as_array().unwrap() {
            let value = case["value"].as_str().unwrap();
            let result = validate_gpp_structure(value);
            assert!(
                result.is_ok(),
                "{}: {}: {result:?}",
                case["id"],
                case["source"]
            );
            let structure = result.unwrap();
            assert_eq!(structure.section_ids.len(), value.split('~').count() - 1);
            assert!(structure.unvalidated_sections.is_empty(), "{}", case["id"]);
            if let Some(id) = structure.section_ids.first() {
                assert_eq!(gpp_sid_mismatch(&id.to_string(), &structure), None);
            }
            assert_eq!(gpp_sid_mismatch("-1", &structure), None);
            assert!(gpp_sid_mismatch("999", &structure).is_some());
        }
        let two = validate_gpp_structure(
            "DBACNYA~CPSG_8APSG_8ANwAAAENAwCgAAAAAAAAAAAAAAAAAAAA.IAAA.YAAAAAAAAAAA~1YNN",
        )
        .unwrap();
        assert_eq!(two.section_ids, [2, 6]);
        assert_eq!(gpp_sid_mismatch("2,6", &two), None);
        assert_eq!(gpp_sid_mismatch("6", &two), None);
        assert!(gpp_sid_mismatch("-1,6", &two).is_some());
        assert!(gpp_sid_mismatch("0", &two).is_some());
    }

    #[test]
    fn header_ranges_correspond_exactly_to_sections() {
        assert!(
            validate_gpp_structure("DBAA")
                .unwrap()
                .section_ids
                .is_empty()
        );
        for value in [
            "DBABLA",
            "DBAA~1YNN",
            "DBABLA~CAAAVVVVVVRA.QA~1YNN",
            "DBABLA~",
            "DBABLA~CAAAVVVVVVRA.",
            "DBABLA~CAAAVVVVVVRA.Q=",
            "DBABLAA~CAAAVVVVVVRA.QA",
        ] {
            assert!(validate_gpp_structure(value).is_err(), "{value}");
        }
        for bad_header in [
            mutate("DBABLA", 35, 1, 1),
            mutate("DBABLA", 12, 12, 4095),
            mutate("DBABLA", 25, 5, 0b00110),
            format!("DBAB{}", "A".repeat(20)),
        ] {
            assert!(
                validate_gpp_structure(&format!("{bad_header}~CAAAVVVVVVRA.QA")).is_err(),
                "{bad_header}"
            );
        }
    }

    #[test]
    fn all_us_current_fields_reject_reserved_choices_and_truncation() {
        let fixtures = corpus();
        for id in 7..=27 {
            let layout = us_layout(id).unwrap();
            let value = fixtures["section_cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| {
                    c["section_id"] == id
                        && c["value"]
                            .as_str()
                            .unwrap()
                            .starts_with(if id == 7 { 'C' } else { 'B' })
                })
                .unwrap()["value"]
                .as_str()
                .unwrap();
            let parts: Vec<_> = value.split('.').collect();
            let mut position = 6;
            for field in layout.fields {
                for index in 0..field.count {
                    let invalid = mutate(parts[0], position, field.width, 3);
                    assert!(
                        validate_us_section(id, &invalid).is_err(),
                        "section{id} {}[{index}]",
                        field.name
                    );
                    if field.minimum == 1 {
                        let invalid = mutate(parts[0], position, field.width, 0);
                        assert!(
                            validate_us_section(id, &invalid).is_err(),
                            "section{id} {}=0",
                            field.name
                        );
                    }
                    position += field.width;
                }
            }
            let last_required_character = position.div_ceil(6);
            assert!(
                validate_us_section(id, &parts[0][..last_required_character - 1]).is_err(),
                "section{id} truncation"
            );
            assert_eq!(
                validate_us_section(id, &parts[0][..last_required_character]),
                Ok(true),
                "section{id} minimal padding"
            );
            assert!(
                validate_us_section(id, &format!("{}A", parts[0])).is_err(),
                "section{id} excess padding"
            );
            if layout.gpc {
                for suffix in ["AA", "QA.QA", "Qg"] {
                    assert!(validate_us_section(id, &format!("{}.{suffix}", parts[0])).is_err());
                }
                assert_eq!(
                    validate_us_section(id, &format!("{}.YA", parts[0])),
                    Ok(true)
                );
            } else if layout.sensitive_count > 0 {
                for suffix in ["wAA", "AA"] {
                    assert!(validate_us_section(id, &format!("{}.{suffix}", parts[0])).is_err());
                }
                assert_eq!(
                    validate_us_section(id, &format!("{}.qqo", parts[0])),
                    Ok(true)
                );
            } else {
                assert!(validate_us_section(id, &format!("{}.QA", parts[0])).is_err());
            }
        }
    }

    #[test]
    fn canada_ranges_and_additional_segments_have_boundaries() {
        const CORE: &str = "BPSG_8APSG_8AAAAAAENAACAAAAAAAAAAAAAAAAAAA";
        assert_eq!(validate_canada(CORE), Ok(true));
        for value in [
            "B".to_string(),
            CORE[..38].to_string(),
            mutate(CORE, 108, 6, 63),
            format!("{CORE}.YAAAAAAAAAA.YAAAAAAAAAA"),
            format!("{CORE}.QAAAA"),
            format!("{CORE}.IA"),
            format!("{CORE}.Y"),
            format!("{CORE}.YAAAAAAAAAB"),
            mutate(CORE, 199, 16, 65535),
        ] {
            assert!(validate_canada(&value).is_err(), "{value}");
        }
        assert_eq!(
            validate_canada("BPSG_8APSG_8AAyACAENGdCgf_gfgAfgfgBgABABAAABAB4AACACAAA.fHHHA4444ao"),
            Ok(true)
        );
    }

    #[test]
    fn unknown_versions_and_sections_remain_explicitly_unvalidated() {
        let future_header = mutate("DBABLA", 6, 6, 2);
        let result = validate_gpp_structure(&format!("{future_header}~AAAA")).unwrap();
        assert_eq!(result.unknown_header_version, Some(2));
        assert_eq!(gpp_sid_mismatch("7", &result), None);
        assert!(validate_gpp_structure(&format!("{future_header}~A@")).is_err());
        let future_us = mutate("CAAAVVVVVVRA", 0, 6, 3);
        let result = validate_gpp_structure(&format!("DBABLA~{future_us}.QA")).unwrap();
        assert_eq!(result.unvalidated_sections, [7]);
        let unknown = mutate("DBABLA", 25, 5, 0b00011);
        let result = validate_gpp_structure(&format!("{unknown}~AAAA")).unwrap();
        assert!(!result.unvalidated_sections.is_empty());
    }

    #[test]
    fn april_2026_section_headers_have_exact_subsection_correspondence() {
        for (id, prefix, subsection) in [
            (24, 'Y', "QA"),
            (25, 'Z', "AAA"),
            (26, 'a', "AAA"),
            (27, 'b', "AAA"),
        ] {
            assert_eq!(
                validate_us_section(id, &format!("{prefix}BAA.BQAA")),
                Ok(true)
            );
            assert_eq!(
                validate_us_section(id, &format!("{prefix}BABY.BQAA.{subsection}")),
                Ok(true)
            );
            for value in [
                format!("{prefix}BAA"),
                format!("{prefix}BAA.BQAA.{subsection}"),
                format!("{prefix}BABY.BQAA"),
                format!("{prefix}BABM.BQAA.{subsection}"),
                format!("{prefix}BAA.BAAA"),
                format!("{prefix}BAA.BQAAAA"),
            ] {
                assert!(
                    validate_us_section(id, &value).is_err(),
                    "section{id}: {value}"
                );
            }
            let wrong_id = if id == 24 { 'Z' } else { 'Y' };
            assert!(validate_us_section(id, &format!("{wrong_id}BAA.BQAA")).is_err());
            assert_eq!(
                validate_us_section(id, &format!("{prefix}CAA.BQAA")),
                Ok(false)
            );
        }
    }

    #[test]
    fn published_notice_definitions_produce_advisories() {
        let fixtures = corpus();
        for id in 7..=27 {
            let value = fixtures["section_cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| {
                    c["section_id"] == id
                        && c["value"]
                            .as_str()
                            .unwrap()
                            .starts_with(if id == 7 { 'C' } else { 'B' })
                })
                .unwrap()["value"]
                .as_str()
                .unwrap();
            let parts: Vec<_> = value.split('.').collect();
            let layout = us_layout(id).unwrap();
            let mut position = 6;
            let mut offsets = std::collections::HashMap::new();
            for field in layout.fields {
                offsets.insert(field.name, position);
                position += field.width * field.count;
            }
            for (notice, choice) in [
                ("SaleOptOutNotice", "SaleOptOut"),
                ("SharingOptOutNotice", "SharingOptOut"),
                (
                    "TargetedAdvertisingOptOutNotice",
                    "TargetedAdvertisingOptOut",
                ),
            ] {
                let (Some(&notice_offset), Some(&choice_offset)) =
                    (offsets.get(notice), offsets.get(choice))
                else {
                    continue;
                };
                let invalid = mutate(&mutate(parts[0], notice_offset, 2, 0), choice_offset, 2, 1);
                // Decode a single-section header with the independently tested Fibonacci encoder.
                let header = test_single_section_header(id);
                let warnings = gpp_policy_warnings(&format!("{header}~{invalid}"));
                assert!(
                    warnings.iter().any(|s| s.contains(choice)),
                    "section{id}: {warnings:?}"
                );
                let provided = mutate(&invalid, notice_offset, 2, 1);
                assert!(
                    !gpp_policy_warnings(&format!("{header}~{provided}"))
                        .iter()
                        .any(|s| s.contains(choice))
                );
            }
        }
        assert!(gpp_policy_warnings("DBABLA~CAAAVVVVVVRA.QA").is_empty());
        let header = test_single_section_header(24);
        assert!(
            gpp_policy_warnings(&format!("{header}~BQAA.QA"))
                .iter()
                .any(|s| s.contains("legacy headerless"))
        );
        assert!(gpp_policy_warnings(&format!("{header}~YBABY.BQAA.QA")).is_empty());
        let ca_header = test_single_section_header(5);
        let canada = "BPSG_8APSG_8AAAAAAENAACAAAAAAAAAAAAAAAAAAA";
        let daytime = mutate(canada, 6, 36, 16_409_952_001);
        assert!(
            gpp_policy_warnings(&format!("{ca_header}~{daytime}"))
                .iter()
                .any(|s| s.contains("full UTC day"))
        );
    }

    #[test]
    fn decoded_us_fields_preserve_exact_arrays_and_absent_fields() {
        assert!(us_field_supported("SaleOptOutNotice"));
        assert!(us_field_supported("MspaVersion"));
        assert!(!us_field_supported("SaleOptOutNotcie"));
        let decoded = decoded_us_sections("DBABLA~CAAAVVVVVVRA.QA");
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].section_id, 7);
        assert_eq!(decoded[0].version, 2);
        assert_eq!(decoded[0].fields["SensitiveDataProcessing"], vec![1; 16]);
        assert_eq!(
            decoded[0].fields["KnownChildSensitiveDataConsents"],
            vec![1; 3]
        );
        let id = test_single_section_header(24);
        let native = decoded_us_sections(&format!("{id}~YBABY.BVVU.YA"));
        assert!(!native[0].legacy_headerless);
        assert_eq!(native[0].fields["SaleOptOutNotice"], [1]);
        assert!(!native[0].fields.contains_key("SharingNotice"));
        assert!(!native[0].fields.contains_key("SensitiveDataProcessing"));
        let legacy = decoded_us_sections(&format!("{id}~BVVU.YA"));
        assert!(legacy[0].legacy_headerless);
        assert_eq!(native[0].fields, legacy[0].fields);
        // MspaVersion is agreement metadata, distinct from SectionHeader.Version.
        for agreement_version in [0, 2, 24, 25, 26, 27, 63] {
            let core = mutate("BVVU", 0, 6, agreement_version);
            for value in [format!("{id}~{core}.YA"), format!("{id}~YBABY.{core}.YA")] {
                let decoded = decoded_us_sections(&value);
                assert_eq!(decoded.len(), 1, "{value}");
                assert_eq!(decoded[0].version, agreement_version as u8);
                assert_eq!(decoded[0].fields["SaleOptOutNotice"], [1]);
            }
        }
        // LiveRamp's published example omits the IAB-required MSPA choice.
        assert!(
            validate_gpp_structure("DBABLA~CVQqAAAAAAA")
                .unwrap_err()
                .contains("MspaCoveredTransaction")
        );
        assert!(decoded_us_sections("DBABLA~CVQqAAAAAAA").is_empty());
    }

    fn test_single_section_header(id: u32) -> String {
        let mut fibonacci = vec![1u32, 2];
        while *fibonacci.last().unwrap() <= id {
            let length = fibonacci.len();
            fibonacci.push(fibonacci[length - 1] + fibonacci[length - 2]);
        }
        fibonacci.pop();
        let mut remaining = id;
        let mut code = vec![0u8; fibonacci.len()];
        for index in (0..fibonacci.len()).rev() {
            if fibonacci[index] <= remaining {
                code[index] = 1;
                remaining -= fibonacci[index];
            }
        }
        code.push(1);
        let mut bits = Bits::new("DBAB").unwrap().bits;
        bits.push(0);
        bits.extend(code);
        while !bits.len().is_multiple_of(6) {
            bits.push(0);
        }
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        bits.chunks(6)
            .map(|c| ALPHABET[c.iter().fold(0usize, |n, b| (n << 1) | usize::from(*b))] as char)
            .collect()
    }
}
