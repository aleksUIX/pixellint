//! ECMAScript Date.prototype.toString, used by Parse.ly browser beacons.
//!
//! Primary grammar: ECMA-262 DateString, TimeString, TimeZoneString and
//! ToDateString, sections 21.4.4.41.1 through 21.4.4.41.4.
//! https://tc39.es/ecma262/multipage/numbers-and-dates.html#sec-todatestring
//! The Parse.ly SDK retrieved on 2026-10-07 calls (new Date).toString().
//! https://cdn.parsely.com/keys/genericconfigfree/p.js

pub(crate) fn valid_javascript_date(value: &str) -> bool {
    parse(value).is_some()
}

fn parse(value: &str) -> Option<()> {
    // The time zone annotation is implementation-defined, including its name
    // and alphabet. Its surrounding space and parentheses are specified.
    let core = match value.split_once(" (") {
        Some((core, annotation)) if annotation.ends_with(')') => core,
        Some(_) => return None,
        None => value,
    };
    if !core.is_ascii() {
        return None;
    }
    let fields: Vec<_> = core.split(' ').collect();
    let [weekday, month, day, year, time, zone] = fields.as_slice() else {
        return None;
    };
    let weekday = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
        .iter()
        .position(|name| name == weekday)? as i64;
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|name| name == month)? as i64
        + 1;
    let day = two_digits(day)?;
    let negative = year.starts_with('-');
    let magnitude = year.strip_prefix('-').unwrap_or(year);
    // DateString pads to at least four digits. Unlike ISO expanded years,
    // positive years have no '+' prefix and negative years need only four.
    if magnitude.len() < 4
        || (magnitude.len() > 4 && magnitude.starts_with('0'))
        || !magnitude.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let mut year = magnitude.parse::<i64>().ok()?;
    if year > 1_000_000 || (negative && year == 0) {
        return None;
    }
    if negative {
        year = -year;
    }
    let leap = year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0);
    let month_days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=month_days).contains(&day) {
        return None;
    }
    let time: Vec<_> = time.split(':').collect();
    let [hour, minute, second] = time.as_slice() else {
        return None;
    };
    let (hour, minute, second) = (two_digits(hour)?, two_digits(minute)?, two_digits(second)?);
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let zone = zone.strip_prefix("GMT")?;
    if zone.len() != 5 || !matches!(zone.as_bytes()[0], b'+' | b'-') {
        return None;
    }
    let (offset_hour, offset_minute) = (two_digits(&zone[1..3])?, two_digits(&zone[3..])?);
    if offset_hour > 23 || offset_minute > 59 {
        return None;
    }
    // Gregorian civil day conversion also works for year zero and BCE years.
    let arithmetic_year = year - i64::from(month <= 2);
    let era = arithmetic_year.div_euclid(400);
    let era_year = arithmetic_year - era * 400;
    let march_month = month + if month > 2 { -3 } else { 9 };
    let year_day = (153 * march_month + 2) / 5 + day - 1;
    let era_day = era_year * 365 + era_year / 4 - era_year / 100 + year_day;
    let days = era * 146097 + era_day - 719468;
    if (days + 4).rem_euclid(7) != weekday {
        return None;
    }
    // TimeClip bounds an ECMAScript instant to 100 million days. The textual
    // offset omits historical sub-minute components, so retain the possible
    // last 59 seconds when validating that lossy representation.
    let offset_sign = if zone.starts_with('-') { -1 } else { 1 };
    let utc_seconds = days * 86400 + hour * 3600 + minute * 60 + second
        - offset_sign * (offset_hour * 3600 + offset_minute * 60);
    if utc_seconds.abs() > 8_640_000_000_059 {
        return None;
    }
    Some(())
}

fn two_digits(value: &str) -> Option<i64> {
    if value.len() != 2 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::valid_javascript_date;

    #[test]
    fn accepts_calendar_offsets_and_signed_years_from_ecmascript_dates() {
        for value in [
            "Wed Oct 07 2026 12:00:00 GMT-0700 (Pacific Daylight Time)",
            "Thu Feb 29 2024 23:59:59 GMT+0000",
            "Sat Jan 01 2000 00:00:00 GMT+0530 (India Standard Time)",
            "Sat Jan 01 0000 00:00:00 GMT+0000",
            "Fri Jan 01 -0001 00:00:00 GMT+0000",
            "Sat Jan 01 10000 00:00:00 GMT+0000",
            "Tue Apr 20 -271821 00:00:00 GMT+0000",
            "Sat Sep 13 275760 00:00:00 GMT+0000",
            "Wed Oct 07 2026 12:00:00 GMT+1245 (Chatham Islands)",
            "Wed Oct 07 2026 12:00:00 GMT+0000 (Coordinated Universal Time (UTC))",
            "Wed Oct 07 2026 12:00:00 GMT+0000 (世界协调时间)",
        ] {
            assert!(valid_javascript_date(value), "{value}");
        }
    }

    #[test]
    fn rejects_non_dates_bad_calendar_clock_offsets_and_weekdays() {
        for value in [
            "Invalid Date",
            "Wed Oct 07 2026",
            "Wed Oct 07 2026 12:00:00 GMT",
            "Wed Oct 07 2026 12:00:00 GMT+24:00",
            "Wed Oct 07 2026 12:00:00 GMT+2400",
            "Wed Oct 07 2026 12:00:00 GMT+0060",
            "Wed Oct 07 2026 24:00:00 GMT+0000",
            "Wed Oct 07 2026 12:60:00 GMT+0000",
            "Wed Oct 07 2026 12:00:60 GMT+0000",
            "Wed Oct 07 2026 12:00:00 GMT+0000 (UTC",
            "Wed Oct 07 2026 12:00:00 GMT+0000 junk",
            "Thu Oct 07 2026 12:00:00 GMT+0000",
            "Fri Feb 29 1900 12:00:00 GMT+0000",
            "Mon Feb 29 2100 12:00:00 GMT+0000",
            "Mon Apr 31 2023 12:00:00 GMT+0000",
            "Wed Oct 7 2026 12:00:00 GMT+0000",
            "Wed Oct 07 +2026 12:00:00 GMT+0000",
            "Sat Jan 01 -0000 00:00:00 GMT+0000",
            "Sat Jan 01 010000 00:00:00 GMT+0000",
            "Sun Jan 01 999999 00:00:00 GMT+0000",
        ] {
            assert!(!valid_javascript_date(value), "{value}");
        }
    }
}
