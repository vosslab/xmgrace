//! Grace-compatible specialised axis tick labels.
//!
//! Date values are astronomical Julian days: `2440587.5` is
//! 1970-01-01 00:00:00. Calendar formatting adds the project's reference
//! date to the supplied offset. The Gregorian calendar is supported from
//! 1582-10-15.

const GREGORIAN_START_JDN: i64 = 2_299_161; // 1582-10-15
const MONTHS_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const MONTHS_LONG: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS_SHORT: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const WEEKDAYS_LONG: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// Whether `key` names a specialised Grace label format.
///
/// `general`, `decimal`, and `exponential` intentionally remain in the plot
/// renderer: they are the ordinary numeric fallback formats, not specialised
/// labels.
pub(crate) fn recognized(key: &str) -> bool {
    matches!(key, "general" | "decimal" | "exponential") || supported(key)
}

pub(crate) fn supported(key: &str) -> bool {
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "power"
            | "scientific"
            | "engineering"
            | "computing"
            | "ddmmyy"
            | "mmddyy"
            | "yymmdd"
            | "mmyy"
            | "mmdd"
            | "monthday"
            | "daymonth"
            | "months"
            | "monthsy"
            | "monthl"
            | "dayofweeks"
            | "dayofweekl"
            | "dayofyear"
            | "hms"
            | "mmddhms"
            | "mmddyyhms"
            | "yymmddhms"
            | "degreeslon"
            | "degreesmmlon"
            | "degreesmmsslon"
            | "mmsslon"
            | "degreeslat"
            | "degreesmmlat"
            | "degreesmmsslat"
            | "mmsslat"
    )
}

/// Format a specialised Grace axis label.  `None` denotes an unknown key or
/// an input outside the supported numeric/calendar domain.
pub(crate) fn format_with_reference(
    n: f64,
    precision: usize,
    key: &str,
    reference_date: f64,
) -> Option<String> {
    if !n.is_finite() {
        return None;
    }
    let precision = precision.min(12);
    let key = key.trim().to_ascii_lowercase();
    if !supported(&key) {
        return None;
    }

    match key.as_str() {
        "power" => Some(power(n, precision)),
        "scientific" => Some(scientific(n, precision)),
        "engineering" => Some(engineering(n, precision)),
        "computing" => Some(computing(n, precision)),
        "degreeslon" => Some(degrees(n, precision, "W", "E")),
        "degreesmmlon" => Some(degrees_minutes(n, precision, "W", "E")),
        "degreesmmsslon" => Some(degrees_minutes_seconds(n, precision, "W", "E")),
        "mmsslon" => Some(minutes_seconds(n, precision, "W", "E")),
        "degreeslat" => Some(degrees(n, precision, "S", "N")),
        "degreesmmlat" => Some(degrees_minutes(n, precision, "S", "N")),
        "degreesmmsslat" => Some(degrees_minutes_seconds(n, precision, "S", "N")),
        "mmsslat" => Some(minutes_seconds(n, precision, "S", "N")),
        _ => date_label(n + reference_date, precision, &key),
    }
}

#[cfg(test)]
fn format(n: f64, precision: usize, key: &str) -> Option<String> {
    format_with_reference(n, precision, key, 0.)
}

fn power(n: f64, precision: usize) -> String {
    if n < 0. {
        format!("-10({:.precision$})", (-n).log10())
    } else if n == 0. {
        format!("{n:.precision$}")
    } else {
        format!("10({:.precision$})", n.log10())
    }
}

fn scientific(n: f64, precision: usize) -> String {
    if n == 0. {
        return format!("{n:.precision$}");
    }
    let exponent = n.abs().log10().floor() as i32;
    format!("{:.precision$}x10({exponent})", n / 10_f64.powi(exponent))
}

fn engineering(n: f64, precision: usize) -> String {
    let exponent = if n == 0. {
        0
    } else {
        ((n.abs().log10().floor() as i32).div_euclid(3) * 3).clamp(-24, 24)
    };
    let prefix = match exponent {
        -24 => "y",
        -21 => "z",
        -18 => "a",
        -15 => "f",
        -12 => "p",
        -9 => "n",
        -6 => "mk", // Grace's non-typeset micro prefix
        -3 => "m",
        3 => "k",
        6 => "M",
        9 => "G",
        12 => "T",
        15 => "P",
        18 => "E",
        21 => "Z",
        24 => "Y",
        _ => "",
    };
    format!("{:.precision$} {prefix}", n / 10_f64.powi(exponent))
}

fn computing(n: f64, precision: usize) -> String {
    let mut exponent = if n == 0. {
        0
    } else {
        ((n.abs().log2().floor() as i32).div_euclid(10) * 10).clamp(0, 80)
    };
    let mut scaled = n / 2_f64.powi(exponent);
    if exponent < 80 && significant(scaled, precision) == "1024" {
        exponent += 10;
        scaled = n / 2_f64.powi(exponent);
    }
    let prefix = match exponent {
        10 => "K",
        20 => "M",
        30 => "G",
        40 => "T",
        50 => "P",
        60 => "E",
        70 => "Z",
        80 => "Y",
        _ => "",
    };
    let mut text = significant(scaled, precision);
    if text == "-0" || text == "-0.0" {
        text = "0".into();
    }
    format!("{text}{prefix}")
}

fn significant(value: f64, precision: usize) -> String {
    if value == 0. {
        return "0".into();
    }
    let digits_before = value.abs().log10().floor() as i32 + 1;
    let decimals = (precision as i32 - digits_before).max(0) as usize;
    let text = format!("{value:.decimals$}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn hemisphere<'a>(n: f64, negative: &'a str, positive: &'a str) -> (&'a str, f64) {
    if n < 0. {
        (negative, -n)
    } else {
        (positive, n)
    }
}

fn degrees(n: f64, precision: usize, negative: &str, positive: &str) -> String {
    if n == 0. {
        "0".into()
    } else {
        let (direction, value) = hemisphere(n, negative, positive);
        format!("{value:.precision$}{direction}")
    }
}

fn degrees_minutes(n: f64, precision: usize, negative: &str, positive: &str) -> String {
    if n == 0. {
        return "0 0'".into();
    }
    let (direction, value) = hemisphere(n, negative, positive);
    let whole = value.floor() as i64;
    format!(
        "{whole} {:.precision$}' {direction}",
        (value - whole as f64) * 60.
    )
}

fn degrees_minutes_seconds(n: f64, precision: usize, negative: &str, positive: &str) -> String {
    if n == 0. {
        return "0 0' 0\"".into();
    }
    let (direction, value) = hemisphere(n, negative, positive);
    let whole = value.floor() as i64;
    let minutes_total = (value - whole as f64) * 60.;
    let minutes = minutes_total.floor() as i64;
    format!(
        "{whole} {minutes}' {:.precision$}\" {direction}",
        (minutes_total - minutes as f64) * 60.
    )
}

fn minutes_seconds(n: f64, precision: usize, negative: &str, positive: &str) -> String {
    if n == 0. {
        return "0 0' 0\"".into();
    }
    let (direction, value) = hemisphere(n, negative, positive);
    let minutes_total = value * 60.;
    let minutes = minutes_total.floor() as i64;
    format!(
        "{minutes}' {:.precision$}\" {direction}",
        (minutes_total - minutes as f64) * 60.
    )
}

#[derive(Clone, Copy)]
struct Calendar {
    jdn: i64,
    year: i64,
    month: usize,
    day: i64,
    second_of_day: i64,
}

fn date_label(n: f64, precision: usize, key: &str) -> Option<String> {
    let calendar = calendar(n)?;
    let year_width = if precision <= 2 { 2 } else { 4 };
    let date = || round_day(calendar);
    let month = || round_month(calendar);
    let time = || calendar;
    let label = match key {
        "ddmmyy" => {
            let c = date();
            format!(
                "{:02}-{:02}-{:0width$}",
                c.day,
                c.month,
                c.year,
                width = year_width
            )
        }
        "mmddyy" => {
            let c = date();
            format!(
                "{:02}-{:02}-{:0width$}",
                c.month,
                c.day,
                c.year,
                width = year_width
            )
        }
        "yymmdd" => {
            let c = date();
            format!(
                "{:0width$}-{:02}-{:02}",
                c.year,
                c.month,
                c.day,
                width = year_width
            )
        }
        "mmyy" => {
            let c = month();
            format!("{:02}-{:0width$}", c.month, c.year, width = year_width)
        }
        "mmdd" => {
            let c = date();
            format!("{:02}-{:02}", c.month, c.day)
        }
        "monthday" => {
            let c = date();
            format!("{}-{:02}", MONTHS_SHORT[c.month - 1], c.day)
        }
        "daymonth" => {
            let c = date();
            format!("{:02}-{}", c.day, MONTHS_SHORT[c.month - 1])
        }
        "months" => MONTHS_SHORT[month().month - 1].into(),
        "monthsy" => {
            let c = month();
            format!(
                "{}-{:0width$}",
                MONTHS_SHORT[c.month - 1],
                c.year,
                width = year_width
            )
        }
        "monthl" => MONTHS_LONG[month().month - 1].into(),
        "dayofweeks" => WEEKDAYS_SHORT[weekday(calendar.jdn)].into(),
        "dayofweekl" => WEEKDAYS_LONG[weekday(calendar.jdn)].into(),
        "dayofyear" => {
            let c = date();
            ordinal(c.year, c.month, c.day).to_string()
        }
        "hms" => hms(time()),
        "mmddhms" => {
            let c = time();
            format!("{:02}-{:02} {}", c.month, c.day, hms(c))
        }
        "mmddyyhms" => {
            let c = time();
            format!("{:02}-{:02}-{} {}", c.month, c.day, c.year, hms(c))
        }
        "yymmddhms" => {
            let c = time();
            format!(
                "{:0width$}-{:02}-{:02} {}",
                c.year,
                c.month,
                c.day,
                hms(c),
                width = year_width
            )
        }
        _ => return None,
    };
    Some(label)
}

fn calendar(value: f64) -> Option<Calendar> {
    let mut jdn = (value + 0.5).floor() as i64;
    if jdn < GREGORIAN_START_JDN {
        return None;
    }
    let mut seconds = (((value + 0.5 - jdn as f64) * 86_400.).floor() + 0.5) as i64;
    if seconds >= 86_400 {
        seconds = 0;
        jdn += 1;
    }
    let (year, month, day) = gregorian(jdn);
    Some(Calendar {
        jdn,
        year,
        month,
        day,
        second_of_day: seconds,
    })
}

fn gregorian(jdn: i64) -> (i64, usize, i64) {
    let a = jdn + 32_044;
    let b = (4 * a + 3) / 146_097;
    let c = a - (146_097 * b) / 4;
    let d = (4 * c + 3) / 1_461;
    let e = c - (1_461 * d) / 4;
    let m = (5 * e + 2) / 153;
    (
        100 * b + d - 4_800 + m / 10,
        (m + 3 - 12 * (m / 10)) as usize,
        e - (153 * m + 2) / 5 + 1,
    )
}

fn round_day(mut c: Calendar) -> Calendar {
    if c.second_of_day >= 43_200 {
        return calendar(c.jdn as f64 + 0.5).expect("Gregorian day remains valid");
    }
    c.second_of_day = 0;
    c
}

fn round_month(c: Calendar) -> Calendar {
    let c = round_day(c);
    let next = if c.month == 12 {
        (c.year + 1, 1)
    } else {
        (c.year, c.month + 1)
    };
    let next_jdn = calendar_to_jdn(next.0, next.1, 1);
    let current_jdn = calendar_to_jdn(c.year, c.month, 1);
    if next_jdn - c.jdn <= c.jdn - current_jdn {
        calendar(next_jdn as f64 - 0.5).expect("Gregorian month remains valid")
    } else {
        Calendar {
            day: 1,
            second_of_day: 0,
            ..c
        }
    }
}

fn calendar_to_jdn(year: i64, month: usize, day: i64) -> i64 {
    let a = (14 - month as i64) / 12;
    let y = year + 4_800 - a;
    let m = month as i64 + 12 * a - 3;
    day + (153 * m + 2) / 5 + 365 * y + y / 4 - y / 100 + y / 400 - 32_045
}

fn weekday(jdn: i64) -> usize {
    ((jdn + 1).rem_euclid(7)) as usize
}

fn ordinal(year: i64, month: usize, day: i64) -> i64 {
    calendar_to_jdn(year, month, day) - calendar_to_jdn(year, 1, 1) + 1
}

fn hms(c: Calendar) -> String {
    let hour = c.second_of_day / 3_600;
    let minute = (c.second_of_day % 3_600) / 60;
    let second = c.second_of_day % 60;
    format!("{hour:02}:{minute:02}:{second:02}")
}

#[cfg(test)]
mod tests {
    use super::{format, supported};

    #[test]
    fn numeric_formats_match_grace_label_conventions() {
        assert_eq!(format(1_234., 2, "scientific"), Some("1.23x10(3)".into()));
        assert_eq!(format(1_234., 2, "engineering"), Some("1.23 k".into()));
        assert_eq!(format(1_024., 4, "computing"), Some("1K".into()));
        assert_eq!(format(-100., 1, "power"), Some("-10(2.0)".into()));
    }

    #[test]
    fn calendar_formats_use_astronomical_julian_days() {
        assert_eq!(format(2_440_587.5, 4, "yymmdd"), Some("1970-01-01".into()));
        assert_eq!(format(2_440_587.5, 4, "dayofweeks"), Some("Thu".into()));
        assert_eq!(
            format(2_440_587.5 + 47_109. / 86_400., 4, "hms"),
            Some("13:05:09".into())
        );
        assert_eq!(format(2_451_544.5, 4, "dayofyear"), Some("1".into()));
        assert_eq!(format(2_299_160.4, 4, "yymmdd"), None);
    }

    #[test]
    fn geographic_and_legacy_keys_are_available() {
        assert_eq!(format(-73.5, 1, "degreesmmlon"), Some("73 30.0' W".into()));
        assert_eq!(
            format(40.25, 1, "degreesmmsslat"),
            Some("40 15' 0.0\" N".into())
        );
        assert!(supported("mmsslat"));
        assert!(!supported("general"));
        assert!(!supported("unknown"));
    }
}
