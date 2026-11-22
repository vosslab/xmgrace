//! Parsing for local text tables selected through the browser file picker.
//!
//! This module only builds complete `Set` values.  Its caller owns attaching
//! them to a project, which makes failed imports atomic.

use crate::dataset::SetKind;
use crate::{Diagnostic, Set};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Delimiter {
    Auto,
    Comma,
    Tab,
    Whitespace,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportOptions {
    pub(crate) delimiter: Delimiter,
    pub(crate) header: bool,
    pub(crate) kind: SetKind,
    /// A zero-based source column for each Grace coordinate. `None` means the
    /// one-based row index and is permitted only for the X coordinate.
    pub(crate) columns: Vec<Option<usize>>,
    /// Additional Y columns that share the selected X column. This is the
    /// classic NXY input form and is valid only for ordinary XY data.
    #[serde(default)]
    pub(crate) extra_y: Vec<usize>,
    /// Interpret the selected X column as a local ISO-8601 date or date-time
    /// and store its Grace Julian-day value.
    #[serde(default)]
    pub(crate) date_x: bool,
}

fn error(line: usize, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        message: message.into(),
        line: Some(line),
    }
}

fn number(value: &str) -> Option<f64> {
    value
        .trim()
        .replace(['d', 'D'], "e")
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn date_x(value: &str) -> Option<f64> {
    let (date, time) = value
        .trim()
        .split_once('T')
        .map_or((value.trim(), None), |(date, time)| (date, Some(time)));
    let mut fields = date.split('-');
    let year = fields.next()?;
    let month = fields.next()?;
    let day = fields.next()?;
    if year.len() != 4
        || month.len() != 2
        || day.len() != 2
        || !year.bytes().all(|byte| byte.is_ascii_digit())
        || !month.bytes().all(|byte| byte.is_ascii_digit())
        || !day.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let year = year.parse::<i32>().ok()?;
    let month = month.parse::<u32>().ok()?;
    let day = day.parse::<u32>().ok()?;
    if fields.next().is_some() || !(1582..=9999).contains(&year) || !(1..=12).contains(&month) {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = [
        31,
        28 + u32::from(leap),
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if day == 0 || day > days[(month - 1) as usize] {
        return None;
    }
    let seconds = match time {
        None => 0.,
        Some(time) => parse_time(time)?,
    };
    // Grace stores the astronomical Julian day for calendar coordinates.
    // This is its Gregorian branch, which starts on 1582-10-15.
    if year == 1582 && (month, day) < (10, 15) {
        return None;
    }
    let year = i64::from(year);
    let month = i64::from(month);
    let day = i64::from(day);
    let century = (year - 1) / 100;
    let non_leap = (year & 3) != 0 || (year % 100 == 0 && ((year / 100) & 3) != 0);
    let julian_day = (1461 * (year - 1)) / 4 + century / 4 - century + (month * 489) / 16
        - if month > 2 {
            if non_leap { 32 } else { 31 }
        } else {
            30
        }
        + day
        + 1_721_425;
    // Astronomical Julian days begin at noon. `cal_to_jul` identifies the
    // civil date and `jul_and_time_to_jul` applies this half-day offset.
    Some(julian_day as f64 + seconds / 86_400. - 0.5)
}

fn parse_time(value: &str) -> Option<f64> {
    let mut fields = value.split(':');
    let hour = fields.next()?;
    let minute = fields.next()?;
    let second = fields.next()?;
    if hour.len() != 2
        || minute.len() != 2
        || !hour.bytes().all(|byte| byte.is_ascii_digit())
        || !minute.bytes().all(|byte| byte.is_ascii_digit())
        || second.is_empty()
        || !second
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
        || second.bytes().filter(|byte| *byte == b'.').count() > 1
    {
        return None;
    }
    let hour = hour.parse::<u32>().ok()?;
    let minute = minute.parse::<u32>().ok()?;
    let second = second.parse::<f64>().ok()?;
    if fields.next().is_some() || hour > 23 || minute > 59 || !(0.0..60.0).contains(&second) {
        return None;
    }
    Some((hour * 3600 + minute * 60) as f64 + second)
}

fn fields(line: &str, delimiter: Delimiter, line_number: usize) -> Result<Vec<String>, Diagnostic> {
    match delimiter {
        Delimiter::Whitespace | Delimiter::Auto => Ok(line
            .split_ascii_whitespace()
            .map(ToOwned::to_owned)
            .collect()),
        Delimiter::Tab => Ok(line
            .split('\t')
            .map(|field| field.trim().to_owned())
            .collect()),
        Delimiter::Comma => csv_fields(line, line_number),
    }
}

fn csv_fields(line: &str, line_number: usize) -> Result<Vec<String>, Diagnostic> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut at_start = true;
    let mut quote_closed = false;
    let mut chars = line.chars().peekable();

    while let Some(character) = chars.next() {
        match character {
            '"' if at_start && !quoted => {
                quoted = true;
                at_start = false;
            }
            '"' if quoted && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' if quoted => {
                quoted = false;
                quote_closed = true;
            }
            ',' if !quoted => {
                result.push(current.trim().to_owned());
                current.clear();
                at_start = true;
                quote_closed = false;
            }
            _ if quote_closed && !character.is_whitespace() => {
                return Err(error(
                    line_number,
                    "Text after a closing CSV quote must begin a new field.",
                ));
            }
            _ if quoted || !character.is_whitespace() || !at_start => {
                current.push(character);
                at_start = false;
            }
            _ => {}
        }
    }
    if quoted {
        return Err(error(line_number, "Unterminated quoted CSV field."));
    }
    result.push(current.trim().to_owned());
    Ok(result)
}

fn resolve_delimiter(line: &str, requested: Delimiter) -> Delimiter {
    match requested {
        Delimiter::Auto if line.contains(',') => Delimiter::Comma,
        Delimiter::Auto if line.contains('\t') => Delimiter::Tab,
        Delimiter::Auto => Delimiter::Whitespace,
        selected => selected,
    }
}

fn validate_options(options: &ImportOptions) -> Result<(), Diagnostic> {
    if options.columns.len() != options.kind.width() {
        return Err(Diagnostic::new(format!(
            "Select {} columns for {} data.",
            options.kind.width(),
            options.kind.as_agr().to_ascii_uppercase()
        )));
    }
    if options.columns.iter().skip(1).any(Option::is_none) {
        return Err(Diagnostic::new(
            "Only the X coordinate can use the row index column.",
        ));
    }
    if !options.extra_y.is_empty() && options.kind != SetKind::Xy {
        return Err(Diagnostic::new(
            "Additional Y columns are available only for XY data.",
        ));
    }
    if options.date_x && options.columns.first().is_none_or(Option::is_none) {
        return Err(Diagnostic::new(
            "Select an X column before importing calendar dates.",
        ));
    }
    Ok(())
}

/// Parses browser-selected text into fully validated data sets.
///
/// Empty lines and `&` delimit sets. Lines beginning with `#` are comments.
/// A header skips the first data record in each input file, not each set.
pub(crate) fn parse(bytes: &[u8], options: ImportOptions) -> Result<Vec<Set>, Diagnostic> {
    validate_options(&options)?;
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Diagnostic::new("The data file must use UTF-8 text."))?
        .trim_start_matches('\u{feff}');
    let mut delimiter = None;
    let mut header_remaining = options.header;
    let mut sets = Vec::new();
    let mut rows = vec![Vec::new(); options.extra_y.len() + 1];

    for (index, raw) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = raw.trim();
        if line.starts_with('#') {
            continue;
        }
        if line.is_empty() || line == "&" {
            if !rows[0].is_empty() {
                for rows in &mut rows {
                    sets.push(make_set(std::mem::take(rows), options.kind));
                }
            }
            continue;
        }
        let selected = *delimiter.get_or_insert_with(|| resolve_delimiter(line, options.delimiter));
        let record = fields(line, selected, line_number)?;
        if header_remaining {
            header_remaining = false;
            continue;
        }
        let row_index = rows[0].len() + 1;
        let mut row = Vec::with_capacity(options.columns.len());
        for (coordinate, source) in options.columns.iter().enumerate() {
            let value = match source {
                None if coordinate == 0 => row_index as f64,
                None => unreachable!("validated above"),
                Some(column) => record
                    .get(*column)
                    .and_then(|field| {
                        if coordinate == 0 && options.date_x {
                            date_x(field)
                        } else {
                            number(field)
                        }
                    })
                    .ok_or_else(|| {
                        let expected = if coordinate == 0 && options.date_x {
                            "an ISO-8601 date from 1582-10-15 onward"
                        } else {
                            "a finite number"
                        };
                        error(
                            line_number,
                            format!("Column {} must contain {expected}.", column + 1),
                        )
                    })?,
            };
            row.push(value);
        }
        let x = row[0];
        rows[0].push(row);
        for (target, column) in rows.iter_mut().skip(1).zip(&options.extra_y) {
            let y = record
                .get(*column)
                .and_then(|field| number(field))
                .ok_or_else(|| {
                    error(
                        line_number,
                        format!("Column {} must contain a finite number.", column + 1),
                    )
                })?;
            target.push(vec![x, y]);
        }
    }
    if !rows[0].is_empty() {
        for rows in rows {
            sets.push(make_set(rows, options.kind));
        }
    }
    if sets.is_empty() {
        return Err(Diagnostic::new("The file contains no data rows."));
    }
    Ok(sets)
}

fn make_set(rows: Vec<Vec<f64>>, kind: SetKind) -> Set {
    Set {
        count: rows.len(),
        rows,
        kind: kind.as_agr().into(),
        ..Set::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{Delimiter, ImportOptions, SetKind, parse};

    fn xy(delimiter: Delimiter) -> ImportOptions {
        ImportOptions {
            delimiter,
            header: false,
            kind: SetKind::Xy,
            columns: vec![Some(0), Some(1)],
            extra_y: Vec::new(),
            date_x: false,
        }
    }

    #[test]
    fn parses_comments_fortran_exponents_and_sets() {
        let sets = parse(b"# source\n0d0,1\n1,2\n&\n2,3\n", xy(Delimiter::Auto)).unwrap();
        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0].rows, vec![vec![0., 1.], vec![1., 2.]]);
        assert_eq!(sets[1].rows, vec![vec![2., 3.]]);
    }

    #[test]
    fn uses_row_index_for_x() {
        let mut options = xy(Delimiter::Whitespace);
        options.columns[0] = None;
        options.columns[1] = Some(0);
        let sets = parse(b"5\n6\n", options).unwrap();
        assert_eq!(sets[0].rows, vec![vec![1., 5.], vec![2., 6.]]);
    }

    #[test]
    fn reports_the_bad_source_line() {
        let error = parse(b"0 1\n1 no\n", xy(Delimiter::Whitespace)).unwrap_err();
        assert_eq!(error.line, Some(2));
    }

    #[test]
    fn reads_quoted_csv_headers() {
        let mut options = xy(Delimiter::Comma);
        options.header = true;
        let sets = parse(b"x,y\n\"0\", \"1\"\n", options).unwrap();
        assert_eq!(sets[0].rows, vec![vec![0., 1.]]);
    }

    #[test]
    fn creates_nxy_sets_with_a_shared_x_column() {
        let mut options = xy(Delimiter::Whitespace);
        options.extra_y = vec![2, 3];
        let sets = parse(b"0 10 20 30\n1 11 21 31\n", options).unwrap();
        assert_eq!(sets.len(), 3);
        assert_eq!(sets[0].rows, vec![vec![0., 10.], vec![1., 11.]]);
        assert_eq!(sets[1].rows, vec![vec![0., 20.], vec![1., 21.]]);
        assert_eq!(sets[2].rows, vec![vec![0., 30.], vec![1., 31.]]);
    }

    #[test]
    fn converts_iso_dates_to_grace_julian_days() {
        let mut options = xy(Delimiter::Comma);
        options.date_x = true;
        let sets = parse(b"2000-01-01,1\n2000-01-01T12:00:00,2\n", options).unwrap();
        assert_eq!(
            sets[0].rows,
            vec![vec![2_451_544.5, 1.], vec![2_451_545., 2.]]
        );
    }

    #[test]
    fn rejects_invalid_nxy_and_dates_before_creating_sets() {
        let mut non_xy = xy(Delimiter::Whitespace);
        non_xy.kind = SetKind::Xydy;
        non_xy.columns.push(Some(2));
        non_xy.extra_y = vec![3];
        assert!(parse(b"0 1 2 3\n", non_xy).is_err());

        let mut dates = xy(Delimiter::Comma);
        dates.date_x = true;
        let error = parse(b"2000-1-1,1\n", dates).unwrap_err();
        assert_eq!(error.line, Some(1));
    }
}
