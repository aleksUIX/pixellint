//! Partial validation of Braze's documented explicit nested `$time` values.
//!
//! Braze lists six date patterns in addition to ISO 8601, defaults malformed
//! timezone offsets to UTC, and does not publish a timezone designation grammar.
//! Calendar fields and the documented year range are checked; opaque timezone
//! tokens and weekday/date correspondence are deliberately preserved.

pub(crate) fn validate_braze_time(value: &str) -> Result<(), String> {
    if !value.is_ascii() || value.is_empty() {
        return Err("must contain a documented date representation".to_string());
    }
    if value.starts_with('-') {
        return Err("must use a year from 0 through 3000".to_string());
    }

    let words: Vec<_> = value.split_whitespace().collect();
    if words.len() == 5 && is_weekday(words[0]) {
        let year = year(words[4])?;
        let month =
            month(words[1]).ok_or_else(|| "contains an invalid calendar month".to_string())?;
        let day = number(words[2]).ok_or_else(|| "contains an invalid calendar day".to_string())?;
        calendar(year, month, day)?;
        let (time, zone) = words[3]
            .split_once('.')
            .ok_or_else(|| "must include the documented timezone designation".to_string())?;
        if zone.is_empty() {
            return Err("must include the documented timezone designation".to_string());
        }
        return clock(time);
    }

    let (date, time) = match value.split_once(['T', 't', ' ']) {
        Some((date, time)) => (date, Some(time)),
        None => (value, None),
    };
    date_fields(date)?;
    match time {
        Some(time) => clock(time),
        None => Ok(()),
    }
}

fn number(value: &str) -> Option<u32> {
    (!value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()))
        .then(|| value.parse().ok())
        .flatten()
}

fn year(value: &str) -> Result<u32, String> {
    let digits = value.strip_prefix('+').unwrap_or(value);
    let year = number(digits).filter(|year| *year <= 3000);
    if digits.len() < 4 || year.is_none() {
        return Err("must use a year from 0 through 3000".to_string());
    }
    Ok(year.unwrap())
}

fn leap(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

fn calendar(year: u32, month: u32, day: u32) -> Result<(), String> {
    let last = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap(year) => 29,
        2 => 28,
        _ => return Err("contains an invalid calendar month".to_string()),
    };
    if !(1..=last).contains(&day) {
        return Err("contains an invalid calendar day".to_string());
    }
    Ok(())
}

fn date_fields(value: &str) -> Result<(), String> {
    // Braze also documents the month/day/year representation.
    if value.len() == 10 && value.as_bytes()[2] == b'/' && value.as_bytes()[5] == b'/' {
        return calendar(
            year(&value[6..])?,
            number(&value[..2]).unwrap_or(0),
            number(&value[3..5]).unwrap_or(0),
        );
    }
    let digits = value.strip_prefix('+').unwrap_or(value);
    if digits.len() < 4 {
        return Err("must contain a documented date representation".to_string());
    }
    let year_length = if value.starts_with('+') {
        digits.find('-').unwrap_or(4)
    } else {
        4
    };
    let y = year(&digits[..year_length])?;
    let rest = &digits[year_length..];
    if rest.is_empty() {
        return Ok(());
    }
    if let Some(week) = rest.strip_prefix("-W").or_else(|| rest.strip_prefix('W')) {
        let compact = week.replace('-', "");
        if !(compact.len() == 2 || compact.len() == 3) {
            return Err("contains an invalid ISO week date".to_string());
        }
        let week = number(&compact[..2]).unwrap_or(0);
        let weekday = if compact.len() == 3 {
            number(&compact[2..]).unwrap_or(0)
        } else {
            1
        };
        // ISO weekday for January 1, using the proleptic Gregorian calendar.
        let prior = i64::from(y) - 1;
        let jan1 = (prior * 365 + prior.div_euclid(4) - prior.div_euclid(100)
            + prior.div_euclid(400))
        .rem_euclid(7)
            + 1;
        let weeks = if jan1 == 4 || (jan1 == 3 && leap(y)) {
            53
        } else {
            52
        };
        if !(1..=weeks).contains(&week) || !(1..=7).contains(&weekday) {
            return Err("contains an invalid ISO week date".to_string());
        }
        return Ok(());
    }
    let compact = rest.replace('-', "");
    let ordinal =
        (rest.starts_with('-') && rest.len() == 4) || (!rest.starts_with('-') && rest.len() == 3);
    if ordinal {
        let day = number(&compact).unwrap_or(0);
        if !(1..=if leap(y) { 366 } else { 365 }).contains(&day) {
            return Err("contains an invalid ISO ordinal date".to_string());
        }
        return Ok(());
    }
    if compact.len() == 2 {
        return calendar(y, number(&compact).unwrap_or(0), 1);
    }
    if compact.len() == 4
        && ((!rest.starts_with('-') && rest.len() == 4)
            || (rest.len() == 6 && rest.as_bytes()[3] == b'-'))
    {
        return calendar(
            y,
            number(&compact[..2]).unwrap_or(0),
            number(&compact[2..]).unwrap_or(0),
        );
    }
    Err("must contain a documented date representation".to_string())
}

fn clock(value: &str) -> Result<(), String> {
    let end = value
        .bytes()
        .position(|b| !b.is_ascii_digit() && !matches!(b, b':' | b'.' | b','))
        .unwrap_or(value.len());
    let literal = &value[..end];
    let (integer, fraction) = match literal.split_once(['.', ',']) {
        Some((integer, fraction)) => (integer, Some(fraction)),
        None => (literal, None),
    };
    let parts: Vec<_> = integer.split(':').collect();
    let (hour, minute, second, colon_fraction) = match parts.as_slice() {
        [hour] if hour.len() == 2 => (number(hour), Some(0), Some(0), None),
        [compact] if compact.len() == 4 || compact.len() == 6 => (
            number(&compact[..2]),
            number(&compact[2..4]),
            if compact.len() == 6 {
                number(&compact[4..])
            } else {
                Some(0)
            },
            None,
        ),
        [hour, minute] if hour.len() == 2 && minute.len() == 2 => {
            (number(hour), number(minute), Some(0), None)
        }
        [hour, minute, second] if hour.len() == 2 && minute.len() == 2 && second.len() == 2 => {
            (number(hour), number(minute), number(second), None)
        }
        [hour, minute, second, millis]
            if hour.len() == 2 && minute.len() == 2 && second.len() == 2 && millis.len() == 3 =>
        {
            (number(hour), number(minute), number(second), Some(*millis))
        }
        _ => return Err("contains an invalid clock time".to_string()),
    };
    let (Some(hour), Some(minute), Some(second)) = (hour, minute, second) else {
        return Err("contains an invalid clock time".to_string());
    };
    let fraction = fraction.or(colon_fraction);
    if fraction.is_some_and(|s| s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit())) {
        return Err("contains an invalid fractional time".to_string());
    }
    if hour > 24
        || minute > 59
        || second > 60
        || (hour == 24
            && (minute != 0
                || second != 0
                || fraction.is_some_and(|s| s.bytes().any(|b| b != b'0'))))
    {
        return Err("contains an invalid clock time".to_string());
    }
    // Missing, malformed and opaque timezone tokens are retained. Braze says
    // malformed offsets default to UTC; no complete TZD grammar is published.
    Ok(())
}

fn is_weekday(value: &str) -> bool {
    ["mon", "tue", "wed", "thu", "fri", "sat", "sun"].contains(&value.to_ascii_lowercase().as_str())
}

fn month(value: &str) -> Option<u32> {
    number(value).or_else(|| {
        [
            "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
        ]
        .iter()
        .position(|month| *month == value.to_ascii_lowercase())
        .map(|index| index as u32 + 1)
    })
}

#[cfg(test)]
mod tests {
    use super::validate_braze_time;

    #[test]
    fn published_date_forms_and_iso_variants_preserve_valid_values() {
        for value in [
            "1980-01-01",
            "2020-05-28",
            "2024-02-29T12:34:56:789Z",
            "2024-02-29T12:34:56",
            "2024-02-29 12:34:56",
            "02/29/2024",
            "Thu 02 29 12:34:56.UTC 2024",
            "Thu Feb 29 12:34:56.PST 2024",
            "2024-02-29T12:34:56.123+05:30",
            "2024-02-29T12:34:56.123456789123456789Z",
            "+002024-02-29T12:34:56Z",
            "20240229T123456+0530",
            "2024-060",
            "2024060",
            "2020-W53-7",
            "2020W537T12:30Z",
            "2024-02",
            "2024",
            "2024-02-29T24:00:00",
            "2024-02-29T12:30:00+bad-offset",
            "0000-02-29",
            "3000-12-31",
        ] {
            assert!(validate_braze_time(value).is_ok(), "{value}");
        }
    }

    #[test]
    fn impossible_calendars_clock_fields_and_out_of_range_years_fail() {
        for value in [
            "2023-02-29",
            "1900-02-29",
            "2024-04-31",
            "2024-13-01",
            "2024-00-01",
            "2024-01-00",
            "02/29/2023",
            "Thu 02 30 12:34:56.UTC 2024",
            "2024-02-29T25:00:00",
            "2024-02-29T24:00:00.1",
            "2024-02-29T12:60:00",
            "2024-02-29T12:30:61",
            "2023-366",
            "2021-W53-1",
            "2020-W00-1",
            "2020-W01-8",
            "3001-01-01",
            "+003001-01-01",
            "-0001-01-01",
            "not-a-date",
            "",
        ] {
            assert!(validate_braze_time(value).is_err(), "{value}");
        }
    }
}
