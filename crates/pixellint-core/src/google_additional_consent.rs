//! Google Additional Consent v1/v2 grammar and source-scoped policy advisories.
//!
//! Primary specification: https://support.google.com/admanager/answer/9681920
//! Current provider IDs are a dated advisory snapshot, not a historical registry.

use std::collections::BTreeSet;

struct AdditionalConsent<'a> {
    consented: Vec<&'a str>,
    disclosed: Vec<&'a str>,
}

pub(crate) fn validate_additional_consent(value: &str) -> Option<String> {
    parse(value).err()
}

pub(crate) fn additional_consent_warnings(value: &str) -> Vec<String> {
    let Ok(decoded) = parse(value) else {
        return Vec::new();
    };
    let consented: BTreeSet<&str> = decoded
        .consented
        .iter()
        .map(|id| canonical_id(id))
        .collect();
    let disclosed: BTreeSet<&str> = decoded
        .disclosed
        .iter()
        .map(|id| canonical_id(id))
        .collect();
    let overlap: Vec<&str> = consented.intersection(&disclosed).copied().collect();
    let mut warnings = Vec::new();
    if !overlap.is_empty() {
        warnings.push(format!("Google recommends excluding consented IDs from the ACv2 disclosed list. IDs occur in both lists: {}.", limited_ids(&overlap)));
    }
    let unknown: Vec<&str> = consented
        .union(&disclosed)
        .copied()
        .filter(|id| PROVIDER_IDS.binary_search(id).is_err())
        .collect();
    if !unknown.is_empty() {
        warnings.push(format!("Additional Consent provider IDs are absent from Google's public ATP registry snapshot read on 2026-10-07: {}. Confirm the matching registry before relying on this string.", limited_ids(&unknown)));
    }
    warnings
}

fn canonical_id(value: &str) -> &str {
    let trimmed = value.trim_start_matches('0');
    if trimmed.is_empty() { "0" } else { trimmed }
}

fn limited_ids(ids: &[&str]) -> String {
    let shown: Vec<String> = ids
        .iter()
        .take(10)
        .map(|id| {
            if id.len() > 80 {
                format!("{}...", &id[..80])
            } else {
                (*id).to_owned()
            }
        })
        .collect();
    let mut value = shown.join(", ");
    if ids.len() > 10 {
        value.push_str(&format!(" (and {} more)", ids.len() - 10));
    }
    value
}

fn parse(value: &str) -> Result<AdditionalConsent<'_>, String> {
    let parts: Vec<&str> = value.split('~').collect();
    match parts.as_slice() {
        ["1", consented] => Ok(AdditionalConsent {
            consented: parse_ids(consented, "consented")?,
            disclosed: Vec::new(),
        }),
        ["2", consented, disclosed] => {
            let tail = if *disclosed == "dv" {
                ""
            } else {
                disclosed.strip_prefix("dv.").ok_or_else(|| {
                    "ACv2 disclosure part must begin with dv., or be exactly dv when empty."
                        .to_owned()
                })?
            };
            Ok(AdditionalConsent {
                consented: parse_ids(consented, "consented")?,
                disclosed: parse_ids(tail, "disclosed")?,
            })
        }
        ["1", ..] => Err(
            "ACv1 must contain exactly a version and consented-ID part separated by ~.".to_owned(),
        ),
        ["2", ..] => Err(
            "ACv2 must contain version, consented-ID and disclosed-ID parts separated by ~."
                .to_owned(),
        ),
        _ => Err("Google currently supports Additional Consent versions 1 and 2.".to_owned()),
    }
}

fn parse_ids<'a>(value: &'a str, label: &str) -> Result<Vec<&'a str>, String> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<&str> = value.split('.').collect();
    if ids
        .iter()
        .any(|id| id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(format!(
            "The {label} list must contain decimal ATP IDs separated by single dots."
        ));
    }
    Ok(ids)
}

// https://storage.googleapis.com/tcfac/additional-consent-providers.csv
// Read 2026-10-07, SHA256 f3f30febabf35a3f0950ddf04772de378493f3a7208437dc9839f574d1c3d103.
// Provider names, domains and policy URLs are not copied into the validator.
const PROVIDER_IDS: &[&str] = &[
    "1003", "10231", "1027", "1031", "1033", "1040", "1046", "1047", "1048", "1051", "1053",
    "10631", "1067", "108", "10831", "1092", "1095", "1097", "1099", "11031", "1107", "1109",
    "1126", "1135", "1143", "1149", "1152", "11531", "1162", "11631", "1166", "117", "1171",
    "1186", "1188", "1192", "1205", "1215", "122", "1220", "1226", "1227", "1230", "124", "1252",
    "1268", "1270", "1276", "1284", "1290", "1301", "1307", "1312", "1329", "1342", "13431",
    "1345", "135", "1356", "1362", "13632", "1365", "13731", "1375", "1403", "14034", "14133",
    "1415", "1416", "1419", "1421", "1423", "14237", "143", "144", "1440", "1449", "1455", "147",
    "149", "1495", "1512", "1514", "1516", "1525", "1540", "1548", "1555", "1558", "1567", "1570",
    "15731", "1577", "1579", "1583", "1584", "159", "1598", "1603", "161", "1616", "1638", "1651",
    "1653", "1659", "1661", "1667", "1677", "1678", "1682", "16831", "16931", "1697", "1699",
    "1712", "1716", "1720", "1721", "1725", "1732", "1735", "1745", "1750", "1753", "1782", "1786",
    "1794", "1800", "1808", "1810", "1825", "1827", "1832", "1838", "184", "1840", "1843", "1845",
    "1859", "1870", "1878", "1880", "1882", "1889", "1898", "1911", "1917", "192", "1928", "1929",
    "1942", "1944", "1958", "196", "1962", "1963", "1964", "1967", "1968", "1969", "1978", "1985",
    "1987", "20", "2003", "2008", "2016", "2027", "2035", "2038", "2039", "2044", "2047", "2052",
    "2056", "2064", "2068", "2069", "2072", "2074", "2084", "2088", "2090", "2107", "2109", "211",
    "2115", "21233", "2124", "2130", "2133", "2135", "2137", "2140", "2141", "2147", "2156",
    "2166", "21731", "2177", "2186", "2205", "2213", "2216", "2219", "2220", "2222", "2223",
    "2224", "2225", "2234", "2251", "2253", "2262", "2271", "2275", "2279", "228", "2282", "2295",
    "2299", "230", "23031", "2309", "2312", "2316", "2322", "2325", "2328", "2331", "2335", "2336",
    "2354", "2358", "2359", "236", "2370", "2373", "2376", "2377", "239", "2400", "2403", "2405",
    "2406", "2411", "2414", "2415", "2416", "2418", "2425", "2427", "2440", "2447", "2453", "2461",
    "2465", "2468", "2472", "2477", "2484", "2486", "2488", "2493", "2498", "2501", "2506", "2510",
    "25131", "2517", "2526", "2527", "2531", "2534", "2535", "2542", "255", "2552", "2559", "2564",
    "2567", "2568", "2569", "2571", "2572", "2575", "2577", "2579", "2583", "2584", "2589", "259",
    "25931", "2595", "2596", "26031", "2604", "2605", "2609", "2610", "2612", "2614", "2621",
    "2624", "2627", "2628", "2629", "2633", "2636", "2639", "2642", "2643", "2645", "2646", "2650",
    "2651", "2652", "2656", "2657", "2658", "266", "2660", "2661", "26631", "2669", "2670", "2677",
    "2681", "2684", "2687", "2689", "2690", "2695", "2698", "2699", "2713", "2714", "272", "2729",
    "2739", "2767", "2768", "2770", "2772", "27731", "2778", "27831", "2784", "2787", "2791",
    "2792", "2798", "2801", "28031", "2805", "2812", "2813", "2814", "2816", "2817", "2821",
    "2822", "2824", "2826", "2827", "2830", "2831", "2832", "2833", "28332", "2834", "2838",
    "2839", "2844", "2846", "2849", "2850", "2852", "2854", "286", "2860", "2862", "2863", "2865",
    "2867", "2869", "2872", "28731", "2874", "2875", "2878", "2880", "2881", "2882", "2883",
    "2884", "2886", "2887", "2888", "2889", "2891", "2893", "2894", "2895", "2897", "2898", "2900",
    "2901", "2908", "2909", "291", "2916", "2917", "2918", "2920", "2922", "2923", "2927", "2929",
    "2930", "2931", "2940", "2941", "2947", "2949", "2950", "2956", "2958", "2961", "2963",
    "29631", "2964", "2965", "2966", "2968", "2972", "2973", "2974", "2975", "2979", "2980",
    "2981", "2983", "2985", "2986", "2987", "2994", "2995", "2997", "2999", "3000", "3001", "3002",
    "3003", "3005", "3008", "3009", "3010", "3012", "3016", "3017", "3018", "3019", "3023", "3028",
    "3031", "30331", "3034", "3038", "3043", "3051", "3052", "3053", "30532", "3055", "3058",
    "3059", "3063", "3066", "3073", "30732", "3074", "3075", "3076", "3077", "3089", "3090",
    "3093", "3094", "3095", "3097", "3099", "3100", "3106", "3107", "3109", "311", "3112", "3117",
    "3119", "3120", "3126", "3127", "3128", "313", "3130", "3133", "3135", "3136", "3137", "314",
    "3145", "3149", "3151", "3153", "3155", "3167", "3169", "3172", "3173", "3177", "3182", "3184",
    "3185", "3186", "3187", "3188", "3189", "3190", "3194", "3196", "320", "3200", "3201", "3209",
    "3210", "3213", "3214", "3215", "3217", "3218", "322", "3222", "3223", "3225", "3226", "3227",
    "3228", "323", "3230", "3231", "3233", "3234", "3235", "3236", "3237", "3238", "3240", "3244",
    "3250", "3251", "3253", "32531", "3254", "3257", "3260", "3266", "327", "3270", "3272", "3286",
    "3288", "3289", "3290", "3292", "3293", "3296", "3299", "3300", "3303", "3306", "3307", "3309",
    "3314", "3315", "3316", "3318", "3323", "3324", "3328", "3330", "3331", "33931", "340",
    "34231", "34631", "34731", "3531", "358", "367", "36831", "370", "371", "3731", "3831", "385",
    "39131", "39531", "40632", "407", "41131", "4131", "415", "41531", "424", "429", "43", "430",
    "436", "43631", "43731", "43831", "445", "4531", "45931", "46", "4631", "469", "47232", "4731",
    "47531", "48131", "4831", "486", "491", "49231", "49332", "494", "49431", "495", "50831",
    "522", "523", "5231", "52831", "54231", "55", "550", "55631", "560", "56131", "568", "56831",
    "56931", "57", "57131", "57231", "574", "57531", "576", "57931", "58131", "58631", "587",
    "591", "59831", "59832", "60731", "60831", "60931", "61", "61531", "61931", "621", "63831",
    "63931", "64031", "64431", "64631", "66531", "6731", "6931", "70", "7131", "723", "7235",
    "737", "7831", "7931", "797", "798", "802", "803", "817", "820", "827", "83", "839", "864",
    "89", "8931", "899", "904", "922", "93", "931", "938", "955", "959", "979", "981", "985",
    "986",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_primary_acv2_examples_and_supported_v1_alternatives() {
        for value in [
            "2~~dv.1.2.3.4.10",
            "2~1.2.3.4.10~dv.",
            "2~1.2.3.4.10~dv",
            "2~1.10~dv.2.3.4",
            "2~1.35.41.101~dv.",
            "1~1.35.41.101",
            "1~",
            "2~~dv.",
            "2~~dv",
        ] {
            assert_eq!(validate_additional_consent(value), None, "{value}");
        }
    }

    #[test]
    fn rejects_independent_delimiter_prefix_digit_and_version_cases() {
        for value in [
            "3~1~dv.",
            "0~1",
            "2~1",
            "2~1~disclosed.2",
            "2~1~dv2",
            "2~1..2~dv.",
            "2~.1~dv.",
            "2~1.~dv.",
            "2~1~dv.2.",
            "2~1~dv.2~3",
            "1~1~dv.",
            "2~١~dv.",
            "2~-1~dv.",
            "2~1 2~dv.",
        ] {
            assert!(validate_additional_consent(value).is_some(), "{value}");
        }
    }

    #[test]
    fn overlap_is_advisory_and_ids_are_compared_as_decimal_values() {
        assert!(
            additional_consent_warnings("2~20~dv.020")
                .iter()
                .any(|m| m.contains("both lists"))
        );
        assert!(
            !additional_consent_warnings("2~20.20~dv")
                .iter()
                .any(|m| m.contains("both lists"))
        );
        assert!(additional_consent_warnings("2~20~dv.46").is_empty());
    }

    #[test]
    fn current_registry_is_dated_advisory_and_future_id_does_not_break_grammar() {
        assert!(PROVIDER_IDS.windows(2).all(|p| p[0] < p[1]));
        assert_eq!(
            validate_additional_consent("2~99999999999999999999999999999999999999~dv."),
            None
        );
        let warnings = additional_consent_warnings("2~99999999999999999999999999999999999999~dv.");
        assert!(warnings.iter().any(|m| m.contains("2026-10-07")));
        assert!(additional_consent_warnings("2~0020~dv").is_empty());
        assert!(additional_consent_warnings("2~20~dv.").is_empty());
        assert!(additional_consent_warnings("3~20").is_empty());
    }
}
