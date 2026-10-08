//! AppMeasurement event names and values from Adobe's primary events guide.
//!
//! https://experienceleague.adobe.com/en/docs/analytics/implementation/vars/page-vars/events/events-overview
//! https://experienceleague.adobe.com/en/docs/analytics/implementation/vars/page-vars/events/event-serialization

/// Account event availability, counter versus numeric configuration, and
/// serialization history require report-suite state. Serialization IDs stay
/// opaque because the AppMeasurement subsection supplies no complete grammar.
/// Adobe's undefined "64k bytes" is interpreted as 64,000 advisory bytes.
pub(crate) fn validate_events(value: &str) -> Option<String> {
    if value.len() > 64_000 {
        return Some("The events string exceeds the advisory 64,000-byte interpretation of Adobe's 64k-byte maximum.".into());
    }
    for (index, event) in value.split(',').enumerate() {
        let event = match event.split_once(':') {
            Some((event, id)) if !id.is_empty() => event,
            Some(_) => {
                return Some(format!(
                    "Event {} has an empty serialization ID.",
                    index + 1
                ));
            }
            None => event,
        };
        let name = match event.split_once('=') {
            Some((name, amount)) if numeric(amount) => name,
            Some(_) => {
                return Some(format!(
                    "Event {} needs a numeric assigned value.",
                    index + 1
                ));
            }
            None => event,
        };
        if !custom_event(name)
            && !matches!(
                name,
                "purchase" | "prodView" | "scOpen" | "scAdd" | "scRemove" | "scView" | "scCheckout"
            )
        {
            return Some(format!(
                "Event {} is not a documented custom or commerce event name.",
                index + 1
            ));
        }
    }
    None
}

fn custom_event(value: &str) -> bool {
    let Some(number) = value.strip_prefix("event") else {
        return false;
    };
    !number.starts_with('0')
        && number.bytes().all(|byte| byte.is_ascii_digit())
        && number
            .parse::<u16>()
            .is_ok_and(|number| (1..=1000).contains(&number))
}

fn numeric(value: &str) -> bool {
    let unsigned = value.strip_prefix(['+', '-']).unwrap_or(value);
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(index) => (&unsigned[..index], Some(&unsigned[index + 1..])),
        None => (unsigned, None),
    };
    let mut digits = 0;
    let mut dots = 0;
    for byte in mantissa.bytes() {
        if byte.is_ascii_digit() {
            digits += 1;
        } else if byte == b'.' {
            dots += 1;
        } else {
            return false;
        }
    }
    digits > 0
        && dots <= 1
        && exponent.is_none_or(|exponent| {
            let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
            !exponent.is_empty() && exponent.bytes().all(|byte| byte.is_ascii_digit())
        })
}

#[cfg(test)]
mod tests {
    use super::validate_events;

    #[test]
    fn primary_examples_and_configured_numeric_values_are_valid() {
        for value in [
            "event1",
            "event1,event13,purchase",
            "event1=10",
            "event1=2,event2",
            "event1=9.99",
            "event1=4.5",
            "event1:ABC123,event2:ABC123",
            "event10=2:abcd",
            "event1000=-2.5:order-id",
            "prodView,scOpen,scAdd,scRemove,scView,scCheckout",
            "event1=.5,event2=1e2",
            "event1:ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        ] {
            assert_eq!(validate_events(value), None, "{value}");
        }
    }

    #[test]
    fn malformed_names_delimiters_and_values_are_diagnosed() {
        for value in [
            "",
            "event0",
            "event1001",
            "event01",
            "Event1",
            "Purchase",
            "prodview",
            "event1,",
            ",event1",
            "event1,,event2",
            "event1|event2",
            "event1=",
            "event1=two",
            "event1=$2",
            "event1=NaN",
            "event1=Infinity",
            "event1=2.3.4",
            "event1=1e",
            "event1:",
            "event1=2:",
        ] {
            assert!(validate_events(value).is_some(), "{value}");
        }
    }

    #[test]
    fn byte_advisory_boundary_is_explicit() {
        let base = "event1:";
        assert_eq!(
            validate_events(&format!("{base}{}", "x".repeat(64_000 - base.len()))),
            None
        );
        assert!(validate_events(&format!("{base}{}", "x".repeat(64_001 - base.len()))).is_some());
        assert!(validate_events(&format!("{base}{}", "é".repeat(32_000))).is_some());
    }
}
