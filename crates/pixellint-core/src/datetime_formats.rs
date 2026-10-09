//! Source-selected calendar checks without a timezone database or host locale.

use crate::manifest::{DateTimeRepresentation, valid_datetime};

pub(crate) fn valid_representation(value: &str, representation: DateTimeRepresentation) -> bool {
    // ASCII numeric prefixes make byte positions deterministic. Named zone
    // text may be localized, so only its numeric prefix has this restriction.
    match representation {
        DateTimeRepresentation::Us12Hour => valid_us(value, true),
        DateTimeRepresentation::Us24Hour => valid_us(value, false),
        DateTimeRepresentation::IsoLocal
        | DateTimeRepresentation::IsoOffset
        | DateTimeRepresentation::IsoNamedTimezone => {
            let Some(prefix) = value.get(..19) else {
                return false;
            };
            if !prefix.is_ascii() || !matches!(prefix.as_bytes()[10], b'T' | b' ') {
                return false;
            }
            let normalized = prefix.replacen(' ', "T", 1);
            if !valid_seconds(&normalized) {
                return false;
            }
            let suffix = &value[19..];
            match representation {
                DateTimeRepresentation::IsoLocal => suffix.is_empty(),
                DateTimeRepresentation::IsoOffset => suffix == "Z" || valid_offset(suffix, false),
                DateTimeRepresentation::IsoNamedTimezone => {
                    suffix.strip_prefix(' ').is_some_and(valid_named_zone)
                }
                _ => unreachable!(),
            }
        }
    }
}

fn valid_us(value: &str, twelve_hour: bool) -> bool {
    if !value.is_ascii()
        || value.len() < 19
        || value.as_bytes()[2] != b'/'
        || value.as_bytes()[5] != b'/'
        || value.as_bytes()[10] != b' '
    {
        return false;
    }
    let date = format!("{}-{}-{}", &value[6..10], &value[..2], &value[3..5]);
    let time = &value[11..];
    let normalized_time = if twelve_hour {
        let Some((clock, marker)) = time.rsplit_once(' ') else {
            return false;
        };
        let Some((hour, rest)) = clock.split_once(':') else {
            return false;
        };
        if !(1..=2).contains(&hour.len())
            || !hour.bytes().all(|b| b.is_ascii_digit())
            || rest.len() != 5
            || !matches!(marker, "AM" | "PM")
        {
            return false;
        }
        let Ok(hour) = hour.parse::<u8>() else {
            return false;
        };
        if !(1..=12).contains(&hour) {
            return false;
        }
        let hour = hour % 12 + if marker == "PM" { 12 } else { 0 };
        format!("{hour:02}:{rest}")
    } else {
        if time.len() != 8 {
            return false;
        }
        time.to_string()
    };
    valid_seconds(&format!("{date}T{normalized_time}"))
}

fn valid_seconds(value: &str) -> bool {
    value.len() == 19
        && valid_datetime(value, false, false, false)
        && value[17..19].parse::<u8>().is_ok_and(|second| second <= 59)
}

fn valid_offset(value: &str, colon: bool) -> bool {
    if !value.is_ascii() || !matches!(value.as_bytes().first(), Some(b'+' | b'-')) {
        return false;
    }
    let (hour, minute) = if colon {
        let Some((hour, minute)) = value[1..].split_once(':') else {
            return false;
        };
        if !(1..=2).contains(&hour.len()) || minute.len() != 2 {
            return false;
        }
        (hour, minute)
    } else {
        if value.len() != 5 {
            return false;
        }
        (&value[1..3], &value[3..5])
    };
    hour.bytes()
        .chain(minute.bytes())
        .all(|b| b.is_ascii_digit())
        && hour.parse::<u8>().is_ok_and(|hour| hour <= 23)
        && minute.parse::<u8>().is_ok_and(|minute| minute <= 59)
}

fn valid_named_zone(zone: &str) -> bool {
    if zone.is_empty() || zone.trim() != zone || zone.chars().any(char::is_control) {
        return false;
    }
    // zzzz accepts general GMT offsets as well as localized names. Check an
    // explicit offset even when written in the named-zone representation.
    if let Some(offset) = zone.strip_prefix("GMT")
        && matches!(offset.as_bytes().first(), Some(b'+' | b'-'))
    {
        return valid_offset(offset, true);
    }
    if matches!(zone.as_bytes().first(), Some(b'+' | b'-')) {
        return valid_offset(zone, false);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendars_and_clocks_do_not_normalize_impossible_dates() {
        for (form, values) in [
            (
                DateTimeRepresentation::Us12Hour,
                vec!["02/29/2024 12:00:00 AM", "02/29/2000 1:59:59 PM"],
            ),
            (
                DateTimeRepresentation::Us24Hour,
                vec!["02/29/2024 00:00:00", "12/31/2024 23:59:59"],
            ),
            (
                DateTimeRepresentation::IsoLocal,
                vec!["2024-02-29T00:00:00", "2000-02-29 23:59:59"],
            ),
        ] {
            for value in values {
                assert!(valid_representation(value, form), "{value}");
            }
        }
        for value in [
            "2023-02-29T12:00:00",
            "1900-02-29T12:00:00",
            "2024-04-31T12:00:00",
            "2024-00-01T12:00:00",
            "2024-01-00T12:00:00",
            "2024-01-01T24:00:00",
            "2024-01-01T12:60:00",
            "2024-01-01T12:00:60",
            "2024-01-01T12:00:00.1",
        ] {
            assert!(
                !valid_representation(value, DateTimeRepresentation::IsoLocal),
                "{value}"
            );
        }
        for value in [
            "02/29/2023 12:00:00 AM",
            "02/29/1900 1:00:00 PM",
            "02/29/2024 0:00:00 AM",
            "02/29/2024 13:00:00 PM",
            "02/29/2024 1:60:00 PM",
            "02/29/2024 1:00:60 PM",
        ] {
            assert!(
                !valid_representation(value, DateTimeRepresentation::Us12Hour),
                "{value}"
            );
        }
    }

    #[test]
    fn offsets_are_bounded_while_named_zone_identity_stays_opaque() {
        for suffix in ["+0000", "-0800", "+2359", "Z"] {
            assert!(valid_representation(
                &format!("2024-02-29T12:00:00{suffix}"),
                DateTimeRepresentation::IsoOffset
            ));
        }
        for suffix in ["+2400", "+1260", "+08:00", "+800", "z", ""] {
            assert!(!valid_representation(
                &format!("2024-02-29T12:00:00{suffix}"),
                DateTimeRepresentation::IsoOffset
            ));
        }
        for zone in [
            "Pacific Standard Time",
            "PST",
            "America/New_York",
            "GMT-08:00",
            "GMT+5:30",
            "GMT+23:59",
            "日本標準時",
            "Unknown Zone",
        ] {
            assert!(
                valid_representation(
                    &format!("2024-02-29 12:00:00 {zone}"),
                    DateTimeRepresentation::IsoNamedTimezone
                ),
                "{zone}"
            );
        }
        for zone in ["GMT+24:00", "GMT+08:60", "+2460", "", " PST", "PST\n"] {
            assert!(
                !valid_representation(
                    &format!("2024-02-29 12:00:00 {zone}"),
                    DateTimeRepresentation::IsoNamedTimezone
                ),
                "{zone}"
            );
        }
    }
}
