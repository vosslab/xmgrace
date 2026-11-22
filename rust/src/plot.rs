pub(crate) use crate::plot_axes::Axis;
use crate::{Graph, Project, Set, Settings, number, unquote, value};
use std::fmt::Write;

const COLORS: [&str; 16] = [
    "#ffffff", "#000000", "#ff0000", "#00ff00", "#0000ff", "#ffff00", "#bc8f8f", "#dcdcdc",
    "#9400d3", "#00ffff", "#ff00ff", "#ffa500", "#7221bc", "#670748", "#40e0d0", "#008b00",
];
pub(crate) const FONT_SCALE: f64 = 0.028;

pub(crate) fn color(project: &Project, index: u32) -> String {
    if let Some([r, g, b]) = project.colors.get(&index) {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        COLORS.get(index as usize).unwrap_or(&COLORS[1]).to_string()
    }
}

pub(crate) fn numeric(settings: &Settings, key: &str, default: f64) -> f64 {
    settings
        .get(key)
        .and_then(|value| number(value))
        .unwrap_or(default)
}

pub(crate) fn paint(project: &Project, settings: &Settings, key: &str, default: u32) -> String {
    color(project, numeric(settings, key, f64::from(default)) as u32)
}

pub(crate) fn enabled(settings: &Settings, key: &str, default: bool) -> bool {
    matches!(
        value(settings, key, if default { "on" } else { "off" }),
        "on" | "true"
    )
}

pub(crate) fn nice_step(span: f64) -> f64 {
    if span <= 0. || !span.is_finite() {
        return 1.;
    }
    let magnitude = 10_f64.powf(span.log10().floor());
    let fraction = span / magnitude;
    let step = if fraction < 1.5 {
        1.
    } else if fraction < 3. {
        2.
    } else if fraction < 7. {
        5.
    } else {
        10.
    };
    step * magnitude
}

pub(crate) struct Page<'a> {
    pub(crate) project: &'a Project,
    pub(crate) scale: f64,
    pub(crate) height: f64,
    pub(crate) output: String,
}

impl Page<'_> {
    pub(crate) fn path(&mut self, data: &str, stroke: &str, width: f64, style: u32, fill: &str) {
        if data.is_empty() || (style == 0 && fill == "none") {
            return;
        }
        let pattern: &[f64] = match style {
            2 => &[1., 3.],
            3 => &[3., 3.],
            4 => &[6., 3.],
            5 => &[9., 3.],
            6 => &[9., 3., 1., 3.],
            7 => &[9., 3., 1., 3., 1., 3.],
            8 => &[9., 3., 1., 3., 1., 3., 1., 3.],
            _ => &[],
        };
        let dash = if pattern.is_empty() {
            "none".into()
        } else {
            pattern
                .iter()
                .map(|n| format!("{:.3}", n * width.max(0.1)))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let stroke = if style == 0 { "none" } else { stroke };
        let _ = write!(
            self.output,
            "<path d=\"{data}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{width:.3}\" stroke-dasharray=\"{dash}\"/>"
        );
    }

    pub(crate) fn label(
        &mut self,
        (x, y): (f64, f64),
        text: &str,
        size: f64,
        ink: &str,
        anchor: &str,
        rotation: f64,
    ) {
        if text.is_empty() || size <= 0. {
            return;
        }
        let text = crate::grace_text::svg_runs(text);
        let _ = write!(
            self.output,
            "<text x=\"{x:.3}\" y=\"{y:.3}\" fill=\"{ink}\" font-size=\"{size:.3}\" text-anchor=\"{anchor}\" transform=\"rotate({rotation} {x:.3} {y:.3})\">{text}</text>"
        );
    }

    fn graph(&mut self, id: u32, graph: &Graph) {
        if !graph.visible {
            return;
        }
        let settings = &graph.settings;
        let [left, bottom, right, top] = [
            graph.view[0] * self.scale,
            self.height - graph.view[1] * self.scale,
            graph.view[2] * self.scale,
            self.height - graph.view[3] * self.scale,
        ];
        let (width, height) = (right - left, bottom - top);
        let x = Axis::new(graph, "x", 0, left, right);
        let y = Axis::new(graph, "y", 1, bottom, top);
        let unit = self.scale;
        let _ = write!(
            self.output,
            "<g data-graph=\"{id}\"><defs><clipPath id=\"graph-{id}\"><rect x=\"{left:.3}\" y=\"{top:.3}\" width=\"{width:.3}\" height=\"{height:.3}\"/></clipPath></defs>"
        );
        let frame = format!("M{left:.3},{bottom:.3}H{right:.3}V{top:.3}H{left:.3}Z");
        if numeric(settings, "frame background pattern", 0.) != 0. {
            let fill = paint(self.project, settings, "frame background color", 0);
            self.path(&frame, "none", 0., 0, &fill);
        }
        let kind = graph.kind.to_ascii_lowercase();
        if kind == "pie" {
            self.pie(graph, left, top, width, height);
            self.graph_titles(graph, left, right, top);
            self.output.push_str("</g>");
            return;
        }
        for grid in [true, false] {
            if !grid {
                let _ = write!(self.output, "<g clip-path=\"url(#graph-{id})\">");
                match kind.as_str() {
                    "chart" => self.chart(graph, &x, &y),
                    "polar" => self.polar(graph, left, bottom, right, top),
                    _ => {
                        for set in graph.sets.values() {
                            self.set(set, &x, &y, [graph.world[1], graph.world[3]]);
                        }
                    }
                }
                self.output.push_str("</g>");
                if numeric(settings, "frame pattern", 1.) != 0. {
                    self.path(
                        &frame,
                        &paint(self.project, settings, "frame color", 1),
                        numeric(settings, "frame linewidth", 1.) * unit * 0.0015,
                        numeric(settings, "frame linestyle", 1.) as u32,
                        "none",
                    );
                }
            }
            self.axis(&x, settings, "xaxis", (bottom, top), grid, false);
            self.axis(&y, settings, "yaxis", (left, right), grid, false);
            self.axis(&x, settings, "altxaxis", (top, bottom), grid, true);
            self.axis(&y, settings, "altyaxis", (right, left), grid, true);
        }
        self.graph_titles(graph, left, right, top);
        self.legend(graph, &x, &y);
        self.output.push_str("</g>");
    }

    fn graph_titles(&mut self, graph: &Graph, left: f64, right: f64, top: f64) {
        let settings = &graph.settings;
        let unit = self.scale;
        for (key, offset, size) in [("title", 0.05, 1.5), ("subtitle", 0.02, 1.)] {
            let ink = paint(self.project, settings, &format!("{key} color"), 1);
            let font = numeric(settings, &format!("{key} size"), size) * unit * FONT_SCALE;
            self.label(
                ((left + right) * 0.5, top - unit * offset),
                unquote(value(settings, key, "")),
                font,
                &ink,
                "middle",
                0.,
            );
        }
    }

    /// Grace charts render each XY point as a bar. The browser accepts both the
    /// classic `bar` set type and ordinary XY data in a Chart graph.
    fn chart(&mut self, graph: &Graph, x: &Axis, y: &Axis) {
        let stacked = enabled(&graph.settings, "stacked", false);
        let gap = numeric(&graph.settings, "bar hgap", 0.).clamp(0., 0.9);
        let sets: Vec<_> = graph
            .sets
            .values()
            .filter(|set| value(&set.settings, "hidden", "false") != "true")
            .collect();
        let mut bases = std::collections::BTreeMap::<i64, f64>::new();
        for (set_index, set) in sets.iter().enumerate() {
            let mut error_bars = String::new();
            let mut error_caps = String::new();
            let width = set
                .rows
                .windows(2)
                .filter_map(|rows| rows[1].first().zip(rows[0].first()))
                .map(|(next, previous)| (next - previous).abs())
                .fold(f64::INFINITY, f64::min)
                .min(x.span() / 10.);
            let group = (sets.len().max(1) as f64).recip();
            for row in &set.rows {
                let Some((&value_x, &value_y)) = row.first().zip(row.get(1)) else {
                    continue;
                };
                let key = (value_x * 1e9).round() as i64;
                let base = if stacked {
                    *bases.get(&key).unwrap_or(&0.)
                } else {
                    0.
                };
                if stacked {
                    bases.insert(key, base + value_y);
                }
                let center = if stacked {
                    value_x
                } else {
                    value_x + (set_index as f64 + 0.5) * width * group - width * 0.5
                };
                let half = width * group * (1. - gap) * 0.5;
                let Some((x0, x1, y0, y1)) = x
                    .at(center - half)
                    .zip(x.at(center + half))
                    .zip(y.at(base))
                    .zip(y.at(base + value_y))
                    .map(|(((x0, x1), y0), y1)| (x0, x1, y0, y1))
                else {
                    continue;
                };
                let path = format!("M{x0:.3},{y0:.3}H{x1:.3}V{y1:.3}H{x0:.3}Z");
                let fill = paint(self.project, &set.settings, "line color", 1);
                self.path(&path, &fill, self.scale * 0.0015, 1, &fill);
                if enabled(&set.settings, "errorbar", true) {
                    let (negative, positive) = match set.kind.as_str() {
                        "bardy" => (
                            row.get(2).copied().unwrap_or(0.).abs(),
                            row.get(2).copied().unwrap_or(0.).abs(),
                        ),
                        "bardydy" => (
                            row.get(3).copied().unwrap_or(0.).abs(),
                            row.get(2).copied().unwrap_or(0.).abs(),
                        ),
                        _ => continue,
                    };
                    let cap = numeric(&set.settings, "errorbar size", 1.) * self.scale * 0.01;
                    let place = value(&set.settings, "errorbar place", "both");
                    let top_value = base + value_y;
                    for (delta, side) in [(-negative, "opposite"), (positive, "normal")] {
                        if delta == 0. || (place != "both" && place != side) {
                            continue;
                        }
                        if let Some(edge) = y.at(top_value + delta) {
                            let _ = write!(error_bars, "M{:.3},{y1:.3}V{edge:.3}", (x0 + x1) * 0.5);
                            let _ = write!(
                                error_caps,
                                "M{:.3},{edge:.3}h{:.3}",
                                (x0 + x1) * 0.5 - cap,
                                2. * cap
                            );
                        }
                    }
                }
            }
            let ink = paint(self.project, &set.settings, "errorbar color", 1);
            self.path(
                &error_bars,
                &ink,
                numeric(&set.settings, "errorbar riser linewidth", 1.) * self.scale * 0.0015,
                numeric(&set.settings, "errorbar riser linestyle", 1.) as u32,
                "none",
            );
            self.path(
                &error_caps,
                &ink,
                numeric(&set.settings, "errorbar linewidth", 1.) * self.scale * 0.0015,
                numeric(&set.settings, "errorbar linestyle", 1.) as u32,
                "none",
            );
        }
    }

    /// Polar data stores angle in X radians and radius in Y. Its world limits
    /// determine the visible radius while the view rectangle supplies the circle.
    fn polar(&mut self, graph: &Graph, left: f64, bottom: f64, right: f64, top: f64) {
        let center = ((left + right) * 0.5, (top + bottom) * 0.5);
        let radius = (right - left).abs().min((bottom - top).abs()) * 0.5;
        let low = graph.world[1];
        let high = graph.world[3];
        if !low.is_finite() || !high.is_finite() || low >= high {
            return;
        }
        for set in graph.sets.values() {
            if value(&set.settings, "hidden", "false") == "true" {
                continue;
            }
            let mut path = String::new();
            for (index, row) in set.rows.iter().enumerate() {
                let Some((&angle, &r)) = row.first().zip(row.get(1)) else {
                    continue;
                };
                let r = (r - low) / (high - low) * radius;
                let px = center.0 + r * angle.cos();
                let py = center.1 - r * angle.sin();
                let _ = write!(
                    path,
                    "{}{:0.3},{:0.3}",
                    if index == 0 { "M" } else { "L" },
                    px,
                    py
                );
            }
            self.path(
                &path,
                &paint(self.project, &set.settings, "line color", 1),
                numeric(&set.settings, "line linewidth", 1.) * self.scale * 0.0015,
                numeric(&set.settings, "line linestyle", 1.) as u32,
                "none",
            );
        }
        self.path(
            &format!("M{:.3},{:.3}a{radius:.3},{radius:.3} 0 1,0 {:.3},0a{radius:.3},{radius:.3} 0 1,0 {:.3},0", center.0 - radius, center.1, 2. * radius, -2. * radius),
            &paint(self.project, &graph.settings, "frame color", 1), self.scale * 0.0015, 1, "none",
        );
    }

    fn pie(&mut self, graph: &Graph, left: f64, top: f64, width: f64, height: f64) {
        let values: Vec<_> = graph
            .sets
            .values()
            .flat_map(|set| set.rows.iter().map(move |row| (set, row)))
            .filter_map(|(set, row)| {
                row.get(1)
                    .copied()
                    .filter(|value| *value > 0.)
                    .map(|value| (set, value))
            })
            .collect();
        let total: f64 = values.iter().map(|(_, value)| value).sum();
        if !total.is_finite() || total <= 0. {
            return;
        }
        let center = (left + width * 0.5, top + height * 0.5);
        let radius = width.min(height) * 0.38;
        let mut angle = -std::f64::consts::FRAC_PI_2;
        for (index, (set, value)) in values.into_iter().enumerate() {
            let end = angle + value / total * std::f64::consts::TAU;
            let (x0, y0) = (
                center.0 + radius * angle.cos(),
                center.1 + radius * angle.sin(),
            );
            let (x1, y1) = (center.0 + radius * end.cos(), center.1 + radius * end.sin());
            let large = u32::from(end - angle > std::f64::consts::PI);
            let data = format!(
                "M{:.3},{:.3}L{x0:.3},{y0:.3}A{radius:.3},{radius:.3} 0 {large},1 {x1:.3},{y1:.3}Z",
                center.0, center.1
            );
            let fill = paint(
                self.project,
                &set.settings,
                "line color",
                (index as u32 % 15) + 1,
            );
            self.path(&data, "#ffffff", self.scale * 0.0015, 1, &fill);
            angle = end;
        }
    }

    fn objects(&mut self) {
        crate::plot_objects::draw(self);
    }

    fn set(&mut self, set: &Set, x: &Axis, y: &Axis, y_world: [f64; 2]) {
        if value(&set.settings, "hidden", "false") == "true" {
            return;
        }
        if crate::plot_special::draw(self, set, x, y) {
            return;
        }
        crate::plot_set_fill::draw(self, set, x, y, y_world);
        let settings = &set.settings;
        let unit = self.scale;
        let mut data = String::new();
        let mut previous = None;
        let connection = numeric(settings, "line type", 1.) as u32;
        if connection != 0 && numeric(settings, "line pattern", 1.) != 0. {
            for row in &set.rows {
                let point = x.at(row[0]).zip(y.at(row[1]));
                if let Some((px, py)) = point {
                    match (previous, connection) {
                        (None, _) => {
                            let _ = write!(data, "M{px:.3},{py:.3}");
                        }
                        (Some(_), 2) => {
                            let _ = write!(data, "H{px:.3}V{py:.3}");
                        }
                        (Some(_), 3) => {
                            let _ = write!(data, "V{py:.3}H{px:.3}");
                        }
                        _ => {
                            let _ = write!(data, "L{px:.3},{py:.3}");
                        }
                    }
                }
                previous = point;
            }
            self.path(
                &data,
                &paint(self.project, settings, "line color", 1),
                numeric(settings, "line linewidth", 1.) * unit * 0.0015,
                numeric(settings, "line linestyle", 1.) as u32,
                "none",
            );
        }
        if set.kind != "xy" && enabled(settings, "errorbar", true) {
            self.error_bars(set, x, y);
        }
        let symbol = numeric(settings, "symbol", 0.) as u32;
        if symbol == 0 {
            return;
        }
        data.clear();
        let radius = numeric(settings, "symbol size", 1.) * unit * 0.01;
        if radius <= 0. {
            return;
        }
        let stride = (numeric(settings, "symbol skip", 0.) as usize).saturating_add(1);
        if symbol == 11 {
            let character = char::from(numeric(settings, "symbol char", 65.) as u8);
            let mut encoded = [0; 4];
            let label = character.encode_utf8(&mut encoded);
            let ink = paint(self.project, settings, "symbol color", 1);
            let size = radius * FONT_SCALE / 0.01;
            for row in set.rows.iter().step_by(stride) {
                if let Some((px, py)) = x.at(row[0]).zip(y.at(row[1])) {
                    self.label((px, py + size * 0.32), label, size, &ink, "middle", 0.);
                }
            }
            return;
        }
        for row in set.rows.iter().step_by(stride) {
            if let Some((px, py)) = x.at(row[0]).zip(y.at(row[1])) {
                symbol_path(&mut data, symbol, px, py, radius);
            }
        }
        let fill = if numeric(settings, "symbol fill pattern", 0.) == 0. {
            "none".into()
        } else {
            paint(self.project, settings, "symbol fill color", 1)
        };
        let outline = u32::from(numeric(settings, "symbol pattern", 1.) != 0.);
        self.path(
            &data,
            &paint(self.project, settings, "symbol color", 1),
            numeric(settings, "symbol linewidth", 1.) * unit * 0.0015,
            outline,
            &fill,
        );
    }

    fn error_bars(&mut self, set: &Set, x: &Axis, y: &Axis) {
        let mut bars = String::new();
        let mut caps = String::new();
        let settings = &set.settings;
        let unit = self.scale;
        let cap = numeric(settings, "errorbar size", 1.) * unit * 0.01;
        let place = value(settings, "errorbar place", "both");
        for row in &set.rows {
            for (index, varying, fixed) in [(0, x, y), (1, y, x)] {
                let (negative, positive) = errors(row, &set.kind, index);
                let Some(other) = fixed.at(row[1 - index]) else {
                    continue;
                };
                let coord = |n: f64| if index == 0 { (n, other) } else { (other, n) };
                let coordinate = row[index];
                let Some(origin) = varying.at(coordinate) else {
                    continue;
                };
                let (ox, oy) = coord(origin);
                for (delta, side) in [(-negative, "opposite"), (positive, "normal")] {
                    if delta == 0. || (place != "both" && place != side) {
                        continue;
                    }
                    let edge = coordinate + delta;
                    if let Some(end) = varying.at(edge) {
                        let (px, py) = coord(end);
                        let _ = write!(bars, "M{ox:.3},{oy:.3}L{px:.3},{py:.3}");
                        if index == 0 {
                            let _ = write!(caps, "M{px:.3},{:.3}v{:.3}", py - cap, 2. * cap);
                        } else {
                            let _ = write!(caps, "M{:.3},{py:.3}h{:.3}", px - cap, 2. * cap);
                        }
                    }
                }
            }
        }
        let color = paint(self.project, settings, "errorbar color", 1);
        for (data, name) in [(bars, "errorbar riser"), (caps, "errorbar")] {
            self.path(
                &data,
                &color,
                numeric(settings, &format!("{name} linewidth"), 1.) * unit * 0.0015,
                numeric(settings, &format!("{name} linestyle"), 1.) as u32,
                "none",
            );
        }
    }

    fn legend(&mut self, graph: &Graph, x: &Axis, y: &Axis) {
        if !enabled(&graph.settings, "legend", true) {
            return;
        }
        let text = value(&graph.settings, "legend position", "0.85, 0.8");
        let Some((a, b)) = text
            .split_once(',')
            .and_then(|(a, b)| number(a.trim()).zip(number(b.trim())))
        else {
            return;
        };
        let unit = self.scale;
        let position = if value(&graph.settings, "legend loctype", "view") == "world" {
            x.at(a).zip(y.at(b))
        } else {
            Some((a * unit, self.height - b * unit))
        };
        let Some((px, py)) = position else {
            return;
        };
        let size = numeric(&graph.settings, "legend char size", 1.) * unit * FONT_SCALE;
        let ink = paint(self.project, &graph.settings, "legend color", 1);
        let entries = graph
            .sets
            .values()
            .filter(|set| value(&set.settings, "hidden", "false") != "true")
            .filter(|set| !unquote(value(&set.settings, "legend", "")).is_empty());
        for (i, set) in entries.enumerate() {
            let cy = py + (i as f64 + 1.) * size * 1.4;
            let line = paint(self.project, &set.settings, "line color", 1);
            self.path(
                &format!("M{px:.3},{cy:.3}h{:.3}", unit * 0.04),
                &line,
                unit * 0.0015,
                if numeric(&set.settings, "line type", 1.) == 0. {
                    0
                } else {
                    numeric(&set.settings, "line linestyle", 1.) as u32
                },
                "none",
            );
            self.label(
                (px + unit * 0.05, cy + size * 0.32),
                unquote(value(&set.settings, "legend", "")),
                size,
                &ink,
                "start",
                0.,
            );
        }
    }
}

pub(crate) fn errors(row: &[f64], kind: &str, axis: usize) -> (f64, f64) {
    let (low, high) = match (kind, axis) {
        ("xydx", 0) | ("xydy", 1) | ("xydxdy", 0) => (2, 2),
        ("xydxdy", 1) => (3, 3),
        ("xydxdx", 0) | ("xydydy", 1) | ("xydxdxdydy", 0) => (3, 2),
        ("xydxdxdydy", 1) => (5, 4),
        _ => return (0., 0.),
    };
    (
        row.get(low).copied().unwrap_or_default().abs(),
        row.get(high).copied().unwrap_or_default().abs(),
    )
}

pub(crate) fn symbol_path(path: &mut String, symbol: u32, x: f64, y: f64, radius: f64) {
    let vertices: &[(f64, f64)] = match symbol {
        2 => &[(-0.85, -0.85), (0.85, -0.85), (0.85, 0.85), (-0.85, 0.85)],
        3 => &[(0., -1.), (1., 0.), (0., 1.), (-1., 0.)],
        4 => &[(0., -1.), (1., 1.), (-1., 1.)],
        5 => &[(-1., 0.), (1., -1.), (1., 1.)],
        6 => &[(0., 1.), (1., -1.), (-1., -1.)],
        7 => &[(1., 0.), (-1., -1.), (-1., 1.)],
        _ => &[],
    };
    if symbol == 1 {
        let _ = write!(
            path,
            "M{:.3},{y:.3}a{radius:.3},{radius:.3} 0 1,0 {:.3},0a{radius:.3},{radius:.3} 0 1,0 {:.3},0Z",
            x - radius,
            2. * radius,
            -2. * radius
        );
    } else if !vertices.is_empty() {
        for (i, (dx, dy)) in vertices.iter().enumerate() {
            let op = if i == 0 { 'M' } else { 'L' };
            let _ = write!(path, "{op}{:.3},{:.3}", x + dx * radius, y + dy * radius);
        }
        path.push('Z');
    } else {
        if matches!(symbol, 8 | 10) {
            let _ = write!(
                path,
                "M{:.3},{y:.3}h{:.3}M{x:.3},{:.3}v{:.3}",
                x - radius,
                2. * radius,
                y - radius,
                2. * radius
            );
        }
        if matches!(symbol, 9 | 10) {
            let _ = write!(
                path,
                "M{:.3},{:.3}L{:.3},{:.3}M{:.3},{:.3}L{:.3},{:.3}",
                x - radius,
                y - radius,
                x + radius,
                y + radius,
                x - radius,
                y + radius,
                x + radius,
                y - radius
            );
        }
    }
}

pub fn svg(project: &Project) -> String {
    let [width, height] = project.page;
    let mut page = Page {
        project,
        scale: width.min(height),
        height,
        output: format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}pt\" height=\"{height}pt\" viewBox=\"0 0 {width} {height}\" font-family=\"Times New Roman, Liberation Serif, serif\">"
        ),
    };
    if project.fill_page {
        let background = color(project, project.background);
        let _ = write!(
            page.output,
            "<rect width=\"{width}\" height=\"{height}\" fill=\"{background}\"/>"
        );
    }
    for (&id, graph) in &project.graphs {
        page.graph(id, graph);
    }
    page.objects();
    page.output.push_str("</svg>");
    page.output
}

#[cfg(test)]
mod tests {
    use super::svg;
    use crate::Project;

    #[test]
    fn alternate_axes_are_opt_in_and_use_the_opposite_graph_edges() {
        let mut project = Project::new();
        let baseline = svg(&project);
        assert!(!baseline.contains("M91.800,91.800L703.800,91.800"));
        assert!(!baseline.contains("M703.800,520.200L703.800,91.800"));

        let settings = &mut project.graphs.get_mut(&0).expect("default graph").settings;
        settings.insert("altxaxis".into(), "on".into());
        settings.insert("altyaxis".into(), "on".into());
        settings.insert("altxaxis label".into(), "Upper axis".into());
        let alternate = svg(&project);
        assert!(alternate.contains("M91.800,91.800L703.800,91.800"));
        assert!(alternate.contains("M703.800,520.200L703.800,91.800"));
        assert!(alternate.contains("y=\"47.124\""));
        assert!(alternate.contains("Upper axis"));
    }
}
