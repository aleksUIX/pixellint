//! Exact timestamp conversion for deterministic event age checks.

use serde::{Deserialize, Serialize};

/// The time representation a vendor accepts for an event timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimestampUnit {
    Seconds,
    Milliseconds,
    Microseconds,
    /// Modern Unix timestamps with magnitude at least 10^12 are milliseconds.
    SecondsOrMilliseconds,
    Datetime,
    /// A documented UTC local timestamp. An omitted zone means UTC.
    DatetimeUtc,
}

/// Capture one clock value for a validation operation. The JS wrapper supplies
/// Date.now through validate_at, since this target has no native wall clock.
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub(crate) fn current_unix_seconds() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_secs()).unwrap_or(i64::MAX),
        Err(error) => {
            let duration = error.duration();
            let seconds = i64::try_from(duration.as_secs()).unwrap_or(i64::MAX);
            seconds
                .saturating_neg()
                .saturating_sub(i64::from(duration.subsec_nanos() > 0))
        }
    }
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub(crate) fn current_unix_seconds() -> i64 {
    panic!("this target has no native wall clock; supply Unix seconds with validate_at")
}

pub(crate) fn timestamp_millis(value: &str, unit: TimestampUnit) -> Option<i64> {
    if unit == TimestampUnit::Datetime {
        return datetime_millis(value);
    }
    if unit == TimestampUnit::DatetimeUtc {
        return datetime_millis(value).or_else(|| datetime_millis(&format!("{value}Z")));
    }
    let (negative, digits, exponent) = decimal_parts(value)?;
    let scale = match unit {
        TimestampUnit::Seconds => 3,
        TimestampUnit::Milliseconds => 0,
        TimestampUnit::Microseconds => -3,
        TimestampUnit::SecondsOrMilliseconds => {
            if digits != "0" && i64::try_from(digits.len()).ok()?.checked_add(exponent)? >= 13 {
                0
            } else {
                3
            }
        }
        TimestampUnit::Datetime | TimestampUnit::DatetimeUtc => unreachable!(),
    };
    let exponent = exponent.checked_add(scale)?;
    if !(0..=19).contains(&exponent) || digits.len().checked_add(exponent as usize)? > 19 {
        return None;
    }
    let mut millis = digits.parse::<i128>().ok()?;
    millis = millis.checked_mul(10_i128.checked_pow(exponent as u32)?)?;
    if negative {
        millis = millis.checked_neg()?;
    }
    i64::try_from(millis).ok()
}

/// Normalize JSON decimal syntax without a float or a bounded significand.
fn decimal_parts(value: &str) -> Option<(bool, &str, i64)> {
    let negative = value.starts_with('-');
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let (significand, exponent) = match unsigned.split_once(['e', 'E']) {
        Some((significand, exponent)) => {
            let digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            (significand, exponent.parse::<i64>().ok()?)
        }
        None => (unsigned, 0),
    };
    // Numeric event timestamps are integers. Integral exponent notation is
    // accepted without losing the original precision.
    if significand.is_empty()
        || !significand.bytes().all(|byte| byte.is_ascii_digit())
        || (significand.len() > 1 && significand.starts_with('0'))
    {
        return None;
    }
    let digits = significand.trim_start_matches('0');
    if digits.is_empty() {
        return Some((false, "0", 0));
    }
    let stripped = digits.trim_end_matches('0');
    let exponent = exponent.checked_add(i64::try_from(digits.len() - stripped.len()).ok()?)?;
    Some((negative, stripped, exponent))
}

fn datetime_millis(value: &str) -> Option<i64> {
    if !value.is_ascii() {
        return None;
    }
    let normalized = value.replacen(' ', "T", 1);
    if !crate::manifest::valid_datetime(&normalized, false, false, true) {
        return None;
    }
    let (date, time) = normalized.split_once(['T', 't'])?;
    let (year, month, day) = if date.len() == 8 {
        (
            date[..4].parse::<i64>().ok()?,
            date[4..6].parse::<i64>().ok()?,
            date[6..].parse::<i64>().ok()?,
        )
    } else {
        (
            date[..4].parse::<i64>().ok()?,
            date[5..7].parse::<i64>().ok()?,
            date[8..].parse::<i64>().ok()?,
        )
    };
    let (hour, minute, second, mut rest) = if time.as_bytes().get(2) == Some(&b':') {
        (
            time[..2].parse::<i64>().ok()?,
            time[3..5].parse::<i64>().ok()?,
            time[6..8].parse::<i64>().ok()?,
            &time[8..],
        )
    } else {
        (
            time[..2].parse::<i64>().ok()?,
            time[2..4].parse::<i64>().ok()?,
            time[4..6].parse::<i64>().ok()?,
            &time[6..],
        )
    };
    let mut fraction_millis = 0;
    if let Some(fraction) = rest.strip_prefix('.') {
        let count = fraction.bytes().take_while(u8::is_ascii_digit).count();
        let first = &fraction[..count.min(3)];
        // Epoch milliseconds discard smaller units deterministically.
        fraction_millis = first.parse::<i64>().ok()? * 10_i64.pow((3 - first.len()) as u32);
        rest = &fraction[count..];
    }
    let offset_seconds = if rest.is_empty() || rest == "Z" || rest == "z" {
        0
    } else {
        let hours = rest[1..3].parse::<i64>().ok()?;
        let minutes = if rest.len() == 5 {
            &rest[3..5]
        } else {
            &rest[4..6]
        }
        .parse::<i64>()
        .ok()?;
        (hours * 3600 + minutes * 60) * if rest.starts_with('-') { -1 } else { 1 }
    };
    // Gregorian calendar conversion. March starts each arithmetic year so leap
    // days occur at its end; the era division also handles year zero.
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146097 + day_of_era - 719468;
    let seconds = days
        .checked_mul(86400)?
        .checked_add(hour * 3600 + minute * 60 + second)?
        .checked_sub(offset_seconds)?;
    seconds.checked_mul(1000)?.checked_add(fraction_millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_units_keep_exact_milliseconds_and_large_integer_precision() {
        assert_eq!(timestamp_millis("1", TimestampUnit::Seconds), Some(1000));
        assert_eq!(timestamp_millis("-1", TimestampUnit::Seconds), Some(-1000));
        assert_eq!(
            timestamp_millis("1700000000", TimestampUnit::SecondsOrMilliseconds),
            Some(1700000000000)
        );
        assert_eq!(
            timestamp_millis("1700000000123", TimestampUnit::SecondsOrMilliseconds),
            Some(1700000000123)
        );
        assert_eq!(
            timestamp_millis("9007199254740993", TimestampUnit::Milliseconds),
            Some(9007199254740993)
        );
        assert_eq!(
            timestamp_millis("17e8", TimestampUnit::Seconds),
            Some(1700000000000)
        );
        assert_eq!(
            timestamp_millis("1000e-3", TimestampUnit::Milliseconds),
            Some(1)
        );
        assert_eq!(
            timestamp_millis("0e999999", TimestampUnit::Milliseconds),
            Some(0)
        );
    }

    #[test]
    fn numeric_overflow_and_non_integral_milliseconds_are_rejected() {
        for value in [
            "1e999",
            "9223372036854775808",
            "-9223372036854775809",
            "1e-3",
            "01",
            "+1",
            "NaN",
            "1.5",
            "",
            "1e+",
        ] {
            assert_eq!(
                timestamp_millis(value, TimestampUnit::Milliseconds),
                None,
                "{value}"
            );
        }
        assert_eq!(
            timestamp_millis("9223372036854775807", TimestampUnit::Milliseconds),
            Some(i64::MAX)
        );
        assert_eq!(
            timestamp_millis("-9223372036854775808", TimestampUnit::Milliseconds),
            Some(i64::MIN)
        );
        assert_eq!(
            timestamp_millis("9223372036854776", TimestampUnit::Seconds),
            None
        );
        assert_eq!(timestamp_millis("1e-3", TimestampUnit::Seconds), Some(1));
    }

    #[test]
    fn datetime_epoch_offsets_basic_notation_and_utc_defaults_agree() {
        for value in [
            "1970-01-01T00:00:00Z",
            "19700101T000000Z",
            "1970-01-01 00:00:00",
            "1970-01-01T01:00:00+01:00",
            "19691231T190000-0500",
        ] {
            assert_eq!(
                timestamp_millis(value, TimestampUnit::Datetime),
                Some(0),
                "{value}"
            );
        }
        assert_eq!(
            timestamp_millis("1969-12-31T23:59:59.999Z", TimestampUnit::Datetime),
            Some(-1)
        );
        assert_eq!(
            timestamp_millis("1970-01-01T00:00:00.1Z", TimestampUnit::Datetime),
            Some(100)
        );
        assert_eq!(
            timestamp_millis("1970-01-01T00:00:00.123456Z", TimestampUnit::Datetime),
            Some(123)
        );
        assert_eq!(
            timestamp_millis("2000-02-29T00:00:00Z", TimestampUnit::Datetime),
            Some(951782400000)
        );
    }

    #[test]
    fn invalid_calendars_and_malformed_offsets_are_rejected() {
        for value in [
            "1900-02-29T00:00:00Z",
            "2025-02-29T00:00:00Z",
            "2026-04-31T00:00:00Z",
            "2026-01-01T24:00:00Z",
            "2026-01-01T00:00:00+25:00",
            "2026-01-01T00:00:00.123oops",
            "2026-01-01",
            "😀",
        ] {
            assert_eq!(
                timestamp_millis(value, TimestampUnit::Datetime),
                None,
                "{value}"
            );
        }
    }
}
