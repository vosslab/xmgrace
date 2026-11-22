//! Area fills, baselines, and drop lines for ordinary Grace XY sets.
//!
//! The caller supplies the baseline in page coordinates so this module stays
//! independent from graph ownership while keeping classic set settings durable.

use crate::plot::{Axis, Page, numeric, paint};
use crate::{Set, value};
use std::fmt::Write;

fn enabled(set: &Set, key: &str) -> bool {
    matches!(value(&set.settings, key, "off"), "on" | "true")
}

fn points(set: &Set, x: &Axis, y: &Axis) -> Vec<(f64, f64)> {
    set.rows
        .iter()
        .filter_map(|row| x.at(*row.first()?).zip(y.at(*row.get(1)?)))
        .collect()
}

/// Draws classic line-tab decorations before the ordinary set line and symbols.
///
/// `y_world` gives the graph's lower and upper world limits. It supplies the
/// graph-minimum and graph-maximum baseline modes without accessing graph state.
pub(crate) fn draw(page: &mut Page<'_>, set: &Set, x: &Axis, y: &Axis, y_world: [f64; 2]) {
    let settings = &set.settings;
    let fill_type = numeric(settings, "fill type", 0.).round() as u32;
    let draw_drops = enabled(set, "dropline");
    let draw_baseline = enabled(set, "baseline");
    if fill_type == 0 && !draw_drops && !draw_baseline {
        return;
    }
    let points = points(set, x, y);
    if points.is_empty() {
        return;
    }
    let needs_baseline = fill_type == 2 || draw_drops || draw_baseline;
    let baseline = needs_baseline
        .then(|| {
            let values = set.rows.iter().filter_map(|row| row.get(1).copied());
            let value = match numeric(settings, "baseline type", 0.).round() as u8 {
                1 => values.clone().fold(f64::INFINITY, f64::min),
                2 => values.clone().fold(f64::NEG_INFINITY, f64::max),
                3 => y_world[0],
                4 => y_world[1],
                5 => {
                    let (sum, count) =
                        values.fold((0., 0usize), |(sum, count), value| (sum + value, count + 1));
                    sum / count as f64
                }
                _ => 0.,
            };
            y.at(value)
        })
        .flatten();
    if fill_type != 0 && numeric(settings, "fill pattern", 1.) == 1. {
        let mut path = String::new();
        for (index, (px, py)) in points.iter().enumerate() {
            let command = if index == 0 { 'M' } else { 'L' };
            let _ = write!(path, "{command}{px:.3},{py:.3}");
        }
        if fill_type == 2 {
            if let (Some((first, _)), Some((last, _)), Some(base)) =
                (points.first(), points.last(), baseline)
            {
                let _ = write!(path, "L{last:.3},{base:.3}L{first:.3},{base:.3}");
            } else {
                return;
            }
        }
        page.path(
            &format!("{path}Z"),
            "none",
            0.,
            0,
            &paint(page.project, settings, "fill color", 1),
        );
    }

    let stroke = paint(page.project, settings, "line color", 1);
    let width = numeric(settings, "line linewidth", 1.) * page.scale * 0.0015;
    let style = numeric(settings, "line linestyle", 1.) as u32;
    if draw_drops && let Some(base) = baseline {
        let mut drops = String::new();
        for (px, py) in &points {
            let _ = write!(drops, "M{px:.3},{base:.3}L{px:.3},{py:.3}");
        }
        page.path(&drops, &stroke, width, style, "none");
    }
    if draw_baseline && let Some(base) = baseline {
        let left = points
            .iter()
            .map(|point| point.0)
            .fold(f64::INFINITY, f64::min);
        let right = points
            .iter()
            .map(|point| point.0)
            .fold(f64::NEG_INFINITY, f64::max);
        if left.is_finite() && right.is_finite() {
            page.path(
                &format!("M{left:.3},{base:.3}L{right:.3},{base:.3}"),
                &stroke,
                width,
                style,
                "none",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Graph, Project};

    #[test]
    fn decorations_use_line_settings_and_baseline_fill() {
        let project = Project::new();
        let mut set = Set::default();
        set.rows = vec![vec![0., 1.], vec![1., 2.]];
        set.settings.insert("fill type".into(), "2".into());
        set.settings.insert("fill color".into(), "2".into());
        set.settings.insert("dropline".into(), "on".into());
        set.settings.insert("baseline".into(), "on".into());
        let graph = Graph::default();
        let x = Axis::new(&graph, "x", 0, 0., 100.);
        let y = Axis::new(&graph, "y", 1, 100., 0.);
        let mut page = Page {
            project: &project,
            scale: 100.,
            height: 100.,
            output: String::new(),
        };
        draw(&mut page, &set, &x, &y, [0., 1.]);
        assert!(page.output.contains("fill=\"#ff0000\""));
        assert!(page.output.matches("M").count() >= 4);
    }
}
