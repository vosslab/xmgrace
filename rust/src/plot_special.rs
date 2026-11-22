//! Classic Cartesian set presentations whose rows carry geometry or style.

use crate::Set;
use crate::plot::{Axis, Page, color, numeric, paint, symbol_path};
use std::fmt::Write;

fn point(x: &Axis, y: &Axis, row: &[f64]) -> Option<(f64, f64)> {
    x.at(*row.first()?).zip(y.at(*row.get(1)?))
}

fn line(page: &mut Page<'_>, set: &Set, x: &Axis, y: &Axis) {
    let settings = &set.settings;
    if numeric(settings, "line type", 1.) == 0. || numeric(settings, "line pattern", 1.) == 0. {
        return;
    }
    let mut path = String::new();
    for row in &set.rows {
        if let Some((px, py)) = point(x, y, row) {
            let command = if path.is_empty() { 'M' } else { 'L' };
            let _ = write!(path, "{command}{px:.3},{py:.3}");
        }
    }
    page.path(
        &path,
        &paint(page.project, settings, "line color", 1),
        numeric(settings, "line linewidth", 1.) * page.scale * 0.0015,
        numeric(settings, "line linestyle", 1.) as u32,
        "none",
    );
}

fn symbols(
    page: &mut Page<'_>,
    set: &Set,
    x: &Axis,
    y: &Axis,
    size_column: bool,
    color_column: bool,
    pattern_column: bool,
) {
    let settings = &set.settings;
    let symbol = numeric(settings, "symbol", 0.) as u32;
    if symbol == 0 {
        return;
    }
    let stride = (numeric(settings, "symbol skip", 0.) as usize).saturating_add(1);
    let mut grouped: std::collections::BTreeMap<(u32, u32), String> = Default::default();
    for row in set.rows.iter().step_by(stride) {
        let Some((px, py)) = point(x, y, row) else {
            continue;
        };
        let radius = if size_column {
            row.get(2).copied().unwrap_or(1.).abs() * page.scale * 0.01
        } else {
            numeric(settings, "symbol size", 1.) * page.scale * 0.01
        };
        let color = if color_column {
            row.get(2).copied().unwrap_or(1.).round().max(0.) as u32
        } else {
            numeric(settings, "symbol color", 1.) as u32
        };
        let pattern = if pattern_column {
            row.get(3).copied().unwrap_or(1.).round().max(0.) as u32
        } else {
            numeric(settings, "symbol fill pattern", 0.) as u32
        };
        symbol_path(
            grouped.entry((color, pattern)).or_default(),
            symbol,
            px,
            py,
            radius,
        );
    }
    for ((ink, pattern), path) in grouped {
        let fill = if pattern == 0 {
            "none".into()
        } else {
            color(page.project, ink)
        };
        page.path(
            &path,
            &color(page.project, ink),
            numeric(settings, "symbol linewidth", 1.) * page.scale * 0.0015,
            u32::from(numeric(settings, "symbol pattern", 1.) != 0.),
            &fill,
        );
    }
}

fn hilo(page: &mut Page<'_>, set: &Set, x: &Axis, y: &Axis) {
    let settings = &set.settings;
    let mut path = String::new();
    let half = numeric(settings, "symbol size", 1.) * page.scale * 0.01;
    for row in &set.rows {
        let Some((&xv, values)) = row.split_first() else {
            continue;
        };
        if values.len() < 4 {
            continue;
        }
        let Some(px) = x.at(xv) else { continue };
        let Some((high, low, open, close)) = y
            .at(values[0])
            .zip(y.at(values[1]))
            .zip(y.at(values[2]))
            .zip(y.at(values[3]))
            .map(|(((a, b), c), d)| (a, b, c, d))
        else {
            continue;
        };
        let _ = write!(
            path,
            "M{px:.3},{high:.3}L{px:.3},{low:.3}M{:.3},{open:.3}H{px:.3}M{px:.3},{close:.3}H{:.3}",
            px - half,
            px + half
        );
    }
    page.path(
        &path,
        &paint(page.project, settings, "symbol color", 1),
        numeric(settings, "symbol linewidth", 1.) * page.scale * 0.0015,
        numeric(settings, "symbol linestyle", 1.) as u32,
        "none",
    );
}

fn circles(page: &mut Page<'_>, set: &Set, x: &Axis, y: &Axis) {
    let settings = &set.settings;
    let fill = if numeric(settings, "symbol fill pattern", 0.) == 0. {
        "none".into()
    } else {
        paint(page.project, settings, "symbol fill color", 1)
    };
    for row in &set.rows {
        if row.len() < 3 {
            continue;
        }
        let Some((cx, cy)) = point(x, y, row) else {
            continue;
        };
        let Some((left, right)) = x.at(row[0] - row[2]).zip(x.at(row[0] + row[2])) else {
            continue;
        };
        let Some((top, bottom)) = y.at(row[1] + row[2]).zip(y.at(row[1] - row[2])) else {
            continue;
        };
        let _ = write!(
            page.output,
            "<ellipse cx=\"{cx:.3}\" cy=\"{cy:.3}\" rx=\"{:.3}\" ry=\"{:.3}\" fill=\"{fill}\" stroke=\"{}\" stroke-width=\"{:.3}\"/>",
            (right - left).abs() * 0.5,
            (bottom - top).abs() * 0.5,
            paint(page.project, settings, "line color", 1),
            numeric(settings, "line linewidth", 1.) * page.scale * 0.0015
        );
    }
}

fn vectors(page: &mut Page<'_>, set: &Set, x: &Axis, y: &Axis) {
    let settings = &set.settings;
    let mut path = String::new();
    let mut heads = String::new();
    for row in &set.rows {
        if row.len() < 4 {
            continue;
        }
        if let Some(((ax, ay), (bx, by))) =
            point(x, y, row).zip(x.at(row[0] + row[2]).zip(y.at(row[1] + row[3])))
        {
            let _ = write!(path, "M{ax:.3},{ay:.3}L{bx:.3},{by:.3}");
            let (dx, dy) = (bx - ax, by - ay);
            let length = (dx * dx + dy * dy).sqrt();
            if length > f64::EPSILON {
                let size = numeric(settings, "errorbar size", 1.) * page.scale * 0.015;
                let (ux, uy) = (dx / length, dy / length);
                let (nx, ny) = (-uy, ux);
                let _ = write!(
                    heads,
                    "M{bx:.3},{by:.3}L{:.3},{:.3}L{:.3},{:.3}Z",
                    bx - ux * size + nx * size * 0.45,
                    by - uy * size + ny * size * 0.45,
                    bx - ux * size - nx * size * 0.45,
                    by - uy * size - ny * size * 0.45
                );
            }
        }
    }
    page.path(
        &path,
        &paint(page.project, settings, "errorbar color", 1),
        numeric(settings, "errorbar riser linewidth", 1.) * page.scale * 0.0015,
        numeric(settings, "errorbar riser linestyle", 1.) as u32,
        "none",
    );
    let ink = paint(page.project, settings, "errorbar color", 1);
    page.path(&heads, &ink, 0., 0, &ink);
}

fn boxplot(page: &mut Page<'_>, set: &Set, x: &Axis, y: &Axis) {
    let settings = &set.settings;
    let half = numeric(settings, "symbol size", 1.) * page.scale * 0.01;
    let ink = paint(page.project, settings, "symbol color", 1);
    let fill = if numeric(settings, "symbol fill pattern", 0.) == 0. {
        "none".into()
    } else {
        paint(page.project, settings, "symbol fill color", 1)
    };
    let mut path = String::new();
    for row in &set.rows {
        if row.len() < 6 {
            continue;
        }
        let Some(px) = x.at(row[0]) else { continue };
        let Some((median, low, high, whisker_low, whisker_high)) = y
            .at(row[1])
            .zip(y.at(row[2]))
            .zip(y.at(row[3]))
            .zip(y.at(row[4]))
            .zip(y.at(row[5]))
            .map(|((((a, b), c), d), e)| (a, b, c, d, e))
        else {
            continue;
        };
        let _ = write!(
            path,
            "M{:.3},{low:.3}H{:.3}V{high:.3}H{:.3}ZM{:.3},{median:.3}H{:.3}M{px:.3},{low:.3}V{whisker_low:.3}M{px:.3},{high:.3}V{whisker_high:.3}",
            px - half,
            px + half,
            px - half,
            px - half,
            px + half
        );
    }
    page.path(
        &path,
        &ink,
        numeric(settings, "symbol linewidth", 1.) * page.scale * 0.0015,
        numeric(settings, "symbol linestyle", 1.) as u32,
        &fill,
    );
}

/// Draw a nonstandard classic set and report whether it consumed the set.
pub(crate) fn draw(page: &mut Page<'_>, set: &Set, x: &Axis, y: &Axis) -> bool {
    match set.kind.as_str() {
        "xyhilo" => hilo(page, set, x, y),
        "xyr" => {
            circles(page, set, x, y);
            symbols(page, set, x, y, false, false, false);
        }
        "xysize" => {
            line(page, set, x, y);
            symbols(page, set, x, y, true, false, false);
        }
        "xycolor" => {
            line(page, set, x, y);
            symbols(page, set, x, y, false, true, false);
        }
        "xycolpat" => {
            line(page, set, x, y);
            symbols(page, set, x, y, false, true, true);
        }
        "xyvmap" => {
            line(page, set, x, y);
            vectors(page, set, x, y);
            symbols(page, set, x, y, false, false, false);
        }
        "xyboxplot" => {
            line(page, set, x, y);
            boxplot(page, set, x, y);
        }
        "xyz" => {
            line(page, set, x, y);
            symbols(page, set, x, y, false, false, false);
        }
        _ => return false,
    }
    true
}
