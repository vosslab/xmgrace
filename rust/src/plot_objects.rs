//! SVG drawing for the typed Grace annotation layer.

use crate::objects::{Arrow, CoordinateSpace, DrawingObject};
use crate::plot::{Axis, FONT_SCALE, Page, color};
use std::fmt::Write;

fn point(page: &Page<'_>, space: &CoordinateSpace, x: f64, y: f64) -> Option<(f64, f64)> {
    match space {
        CoordinateSpace::View => Some((x * page.scale, page.height - y * page.scale)),
        CoordinateSpace::World { graph } => {
            let graph = page
                .project
                .graphs
                .get(graph)
                .filter(|graph| graph.visible)?;
            let left = graph.view[0] * page.scale;
            let bottom = page.height - graph.view[1] * page.scale;
            let right = graph.view[2] * page.scale;
            let top = page.height - graph.view[3] * page.scale;
            Axis::new(graph, "x", 0, left, right)
                .at(x)
                .zip(Axis::new(graph, "y", 1, bottom, top).at(y))
        }
    }
}

/// Draw Grace's three arrow-head forms. The layout keeps the AGR order:
/// head width first, then the trailing notch length.
fn arrow(page: &mut Page<'_>, start: (f64, f64), end: (f64, f64), arrow: &Arrow, ink: &str) {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let length = (dx * dx + dy * dy).sqrt();
    if length <= f64::EPSILON || arrow.ends == 0 {
        return;
    }
    let unit = (dx / length, dy / length);
    for (point, direction, selected) in [
        (start, (-unit.0, -unit.1), arrow.ends & 1 != 0),
        (end, unit, arrow.ends & 2 != 0),
    ] {
        if !selected {
            continue;
        }
        let radius = (arrow.size * page.scale * 0.015).max(2.);
        let normal = (-direction.1, direction.0);
        let base = (
            point.0 - direction.0 * radius,
            point.1 - direction.1 * radius,
        );
        let left = (
            base.0 + normal.0 * radius * arrow.layout[0] * 0.5,
            base.1 + normal.1 * radius * arrow.layout[0] * 0.5,
        );
        let right = (
            base.0 - normal.0 * radius * arrow.layout[0] * 0.5,
            base.1 - normal.1 * radius * arrow.layout[0] * 0.5,
        );
        let notch = (
            base.0 + direction.0 * radius * arrow.layout[1],
            base.1 + direction.1 * radius * arrow.layout[1],
        );
        match arrow.kind {
            0 => page.path(
                &format!(
                    "M{:.3},{:.3}L{:.3},{:.3}L{:.3},{:.3}",
                    left.0, left.1, point.0, point.1, right.0, right.1
                ),
                ink,
                page.scale * 0.0015,
                1,
                "none",
            ),
            1 | 2 => {
                let fill = if arrow.kind == 1 {
                    ink.to_owned()
                } else {
                    color(page.project, page.project.background)
                };
                page.path(
                    &format!(
                        "M{:.3},{:.3}L{:.3},{:.3}L{:.3},{:.3}L{:.3},{:.3}Z",
                        left.0, left.1, point.0, point.1, right.0, right.1, notch.0, notch.1
                    ),
                    ink,
                    page.scale * 0.0015,
                    1,
                    &fill,
                );
            }
            _ => {}
        }
    }
}

fn draw_one(page: &mut Page<'_>, object: &DrawingObject) {
    let space = match object {
        DrawingObject::Text { space, .. }
        | DrawingObject::Line { space, .. }
        | DrawingObject::Box { space, .. }
        | DrawingObject::Ellipse { space, .. } => space,
    };
    let clipped = matches!(space, CoordinateSpace::World { .. });
    if let CoordinateSpace::World { graph } = space {
        let _ = write!(page.output, "<g clip-path=\"url(#graph-{graph})\">");
    }
    match object {
        DrawingObject::Text {
            visible,
            coordinates,
            text,
            style,
            ..
        } if *visible => {
            if let Some(point) = point(page, space, coordinates[0], coordinates[1]) {
                let anchor = match style.justification % 3 {
                    0 => "start",
                    1 => "middle",
                    _ => "end",
                };
                page.label(
                    point,
                    text,
                    style.size * page.scale * FONT_SCALE,
                    &color(page.project, style.color),
                    anchor,
                    -style.rotation,
                );
            }
        }
        DrawingObject::Line {
            visible,
            coordinates,
            stroke,
            arrow: arrows,
            ..
        } if *visible => {
            if let Some((start, end)) = point(page, space, coordinates[0], coordinates[1])
                .zip(point(page, space, coordinates[2], coordinates[3]))
            {
                let ink = color(page.project, stroke.color);
                page.path(
                    &format!("M{:.3},{:.3}L{:.3},{:.3}", start.0, start.1, end.0, end.1),
                    &ink,
                    stroke.width * page.scale * 0.0015,
                    stroke.style as u32,
                    "none",
                );
                arrow(page, start, end, arrows, &ink);
            }
        }
        DrawingObject::Box {
            visible,
            coordinates,
            stroke,
            fill,
            ..
        } if *visible => {
            if let Some((a, b)) = point(page, space, coordinates[0], coordinates[1]).zip(point(
                page,
                space,
                coordinates[2],
                coordinates[3],
            )) {
                let fill = if fill.pattern == 0 {
                    "none".into()
                } else {
                    color(page.project, fill.color)
                };
                page.path(
                    &format!("M{:.3},{:.3}H{:.3}V{:.3}H{:.3}Z", a.0, a.1, b.0, b.1, a.0),
                    &color(page.project, stroke.color),
                    stroke.width * page.scale * 0.0015,
                    stroke.style as u32,
                    &fill,
                );
            }
        }
        DrawingObject::Ellipse {
            visible,
            coordinates,
            stroke,
            fill,
            ..
        } if *visible => {
            if let Some((a, b)) = point(page, space, coordinates[0], coordinates[1]).zip(point(
                page,
                space,
                coordinates[2],
                coordinates[3],
            )) {
                let fill = if fill.pattern == 0 {
                    "none".into()
                } else {
                    color(page.project, fill.color)
                };
                let _ = write!(
                    page.output,
                    "<ellipse cx=\"{:.3}\" cy=\"{:.3}\" rx=\"{:.3}\" ry=\"{:.3}\" fill=\"{fill}\" stroke=\"{}\" stroke-width=\"{:.3}\"/>",
                    (a.0 + b.0) * 0.5,
                    (a.1 + b.1) * 0.5,
                    (a.0 - b.0).abs() * 0.5,
                    (a.1 - b.1).abs() * 0.5,
                    color(page.project, stroke.color),
                    stroke.width * page.scale * 0.0015
                );
            }
        }
        _ => {}
    }
    if clipped {
        page.output.push_str("</g>");
    }
}

pub(crate) fn draw(page: &mut Page<'_>) {
    for object in page.project.objects.values() {
        draw_one(page, object);
    }
}
