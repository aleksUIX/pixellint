//! ISO 4217 currency designations from the official SIX Maintenance Agency.
//!
//! Snapshots read on 2026-10-07. Current codes include funds and precious metals.
//! Vendor account eligibility is a separate requirement from code assignment.
//! Historical codes are accepted only when a contract explicitly opts in.

// Each snapshot retains its exact source URL and publisher date alongside codes.
const CURRENT: (&str, &str, &[&str]) = (
    "https://www.six-group.com/dam/download/financial-information/data-center/iso-currrency/lists/list-one.xml",
    "2026-09-17",
    &[
        "AED", "AFN", "ALL", "AMD", "AOA", "ARS", "AUD", "AWG", "AZN", "BAM", "BBD", "BDT", "BHD",
        "BIF", "BMD", "BND", "BOB", "BOV", "BRL", "BSD", "BTN", "BWP", "BYN", "BZD", "CAD", "CDF",
        "CHE", "CHF", "CHW", "CLF", "CLP", "CNY", "COP", "COU", "CRC", "CUP", "CVE", "CZK", "DJF",
        "DKK", "DOP", "DZD", "EGP", "ERN", "ETB", "EUR", "FJD", "FKP", "GBP", "GEL", "GHS", "GIP",
        "GMD", "GNF", "GTQ", "GYD", "HKD", "HNL", "HTG", "HUF", "IDR", "ILS", "INR", "IQD", "IRR",
        "ISK", "JMD", "JOD", "JPY", "KES", "KGS", "KHR", "KMF", "KPW", "KRW", "KWD", "KYD", "KZT",
        "LAK", "LBP", "LKR", "LRD", "LSL", "LYD", "MAD", "MDL", "MGA", "MKD", "MMK", "MNT", "MOP",
        "MRU", "MUR", "MVR", "MWK", "MXN", "MXV", "MYR", "MZN", "NAD", "NGN", "NIO", "NOK", "NPR",
        "NZD", "OMR", "PAB", "PEN", "PGK", "PHP", "PKR", "PLN", "PYG", "QAR", "RON", "RSD", "RUB",
        "RWF", "SAR", "SBD", "SCR", "SDG", "SEK", "SGD", "SHP", "SLE", "SOS", "SRD", "SSP", "STN",
        "SVC", "SYP", "SZL", "THB", "TJS", "TMT", "TND", "TOP", "TRY", "TTD", "TWD", "TZS", "UAH",
        "UGX", "USD", "USN", "UYI", "UYU", "UYW", "UZS", "VED", "VES", "VND", "VUV", "WST", "XAD",
        "XAF", "XAG", "XAU", "XBA", "XBB", "XBC", "XBD", "XCD", "XCG", "XDR", "XOF", "XPD", "XPF",
        "XPT", "XSU", "XTS", "XUA", "XXX", "YER", "ZAR", "ZMW", "ZWG",
    ],
);

const HISTORICAL: (&str, &str, &[&str]) = (
    "https://www.six-group.com/dam/download/financial-information/data-center/iso-currrency/lists/list-three.xml",
    "2026-01-01",
    &[
        "ADP", "AFA", "ALK", "ANG", "AOK", "AON", "AOR", "ARA", "ARP", "ARY", "ATS", "AYM", "AZM",
        "BAD", "BEC", "BEF", "BEL", "BGJ", "BGK", "BGL", "BGN", "BOP", "BRB", "BRC", "BRE", "BRN",
        "BRR", "BUK", "BYB", "BYR", "CHC", "CSD", "CSJ", "CSK", "CUC", "CYP", "DDM", "DEM", "ECS",
        "ECV", "EEK", "ESA", "ESB", "ESP", "EUR", "FIM", "FRF", "GEK", "GHC", "GHP", "GNE", "GNS",
        "GQE", "GRD", "GWE", "GWP", "HRD", "HRK", "IDR", "IEP", "ILP", "ILR", "ISJ", "ITL", "LAJ",
        "LSM", "LTL", "LTT", "LUC", "LUF", "LUL", "LVL", "LVR", "MGF", "MLF", "MRO", "MTL", "MTP",
        "MVQ", "MWK", "MXP", "MZE", "MZM", "NIC", "NLG", "PEH", "PEI", "PEN", "PES", "PLZ", "PTE",
        "RHD", "ROK", "ROL", "RON", "RUR", "SDD", "SDG", "SDP", "SIT", "SKK", "SLL", "SRG", "STD",
        "SUR", "SZL", "TJR", "TMM", "TPE", "TRL", "TRY", "UAK", "UGS", "UGW", "USS", "UYN", "UYP",
        "VEB", "VEF", "VNC", "XEU", "XFO", "XFU", "XRE", "YDD", "YUD", "YUM", "YUN", "ZAL", "ZMK",
        "ZRN", "ZRZ", "ZWC", "ZWD", "ZWL", "ZWN", "ZWR",
    ],
);

/// Checks code assignment. The calling format owns the vendor's case policy.
pub(crate) fn known_currency(value: &str, allow_historical: bool) -> bool {
    if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return false;
    }
    let uppercase = value.to_ascii_uppercase();
    CURRENT.2.binary_search(&uppercase.as_str()).is_ok()
        || (allow_historical && HISTORICAL.2.binary_search(&uppercase.as_str()).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_assignments_include_recent_currency_and_fund_changes() {
        for code in ["USD", "EUR", "ZWG", "XCG", "XAD"] {
            assert!(known_currency(code, false), "current assignment {code}");
        }
        assert_eq!(CURRENT.1, "2026-09-17");
        assert!(CURRENT.0.ends_with("/list-one.xml"));
    }

    #[test]
    fn retired_codes_need_explicit_historical_acceptance() {
        for code in ["BGN", "FRF", "DEM", "ZWL"] {
            assert!(!known_currency(code, false), "retired assignment {code}");
            assert!(known_currency(code, true), "historical assignment {code}");
        }
        assert_eq!(HISTORICAL.1, "2026-01-01");
        assert!(HISTORICAL.0.ends_with("/list-three.xml"));
    }

    #[test]
    fn assignment_lookup_folds_ascii_case_and_rejects_unassigned_shapes() {
        for value in ["usd", "Usd", "EUR"] {
            assert!(known_currency(value, false));
        }
        for value in [
            "ZZZ",
            "",
            "US",
            "USDD",
            "840",
            " USD",
            "USD ",
            "ＵＳＤ",
            "ÜSD",
        ] {
            assert!(!known_currency(value, true), "invalid code {value:?}");
        }
    }
}
