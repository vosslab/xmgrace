use crate::{Graph, Project, Set, Settings, number, unquote, value};
use std::fmt::Write;

const COLORS: [&str; 16] = [
    "#ffffff", "#000000", "#ff0000", "#00ff00", "#0000ff", "#ffff00", "#bc8f8f", "#dcdcdc",
    "#9400d3", "#00ffff", "#ff00ff", "#ffa500", "#7221bc", "#670748", "#40e0d0", "#008b00",
];
const FONT_SCALE: f64 = 0.028;

fn color(project: &Project, index: u32) -> String {
    if let Some([r, g, b]) = project.colors.get(&index) {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        COLORS.get(index as usize).unwrap_or(&COLORS[1]).to_string()
    }
}

fn numeric(settings: &Settings, key: &str, default: f64) -> f64 {
    settings
        .get(key)
        .and_then(|value| number(value))
        .unwrap_or(default)
}

fn paint(project: &Project, settings: &Settings, key: &str, default: u32) -> String {
    color(project, numeric(settings, key, f64::from(default)) as u32)
}

fn enabled(settings: &Settings, key: &str, default: bool) -> bool {
    value(settings, key, if default { "on" } else { "off" }) == "on"
}

fn xml(text: &str) -> String {
    text.chars()
        .filter(|&ch| ch == '\t' || ch == '\n' || ch >= ' ')
        .fold(String::new(), |mut escaped, ch| {
            match ch {
                '&' => escaped.push_str("&amp;"),
                '<' => escaped.push_str("&lt;"),
                '>' => escaped.push_str("&gt;"),
                _ => escaped.push(ch),
            }
            escaped
        })
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

struct Axis {
    low: f64,
    high: f64,
    from: f64,
    to: f64,
    log: bool,
}

impl Axis {
    fn new(graph: &Graph, name: &str, index: usize, from: f64, to: f64) -> Self {
        let log = value(&graph.settings, &format!("{name}axes scale"), "Normal")
            .eq_ignore_ascii_case("logarithmic");
        let invert = enabled(&graph.settings, &format!("{name}axes invert"), false);
        let (from, to) = if invert { (to, from) } else { (from, to) };
        Self {
            low: graph.world[index],
            high: graph.world[index + 2],
            from,
            to,
            log,
        }
    }

    fn at(&self, n: f64) -> Option<f64> {
        let (n, low, high) = if self.log {
            (n.log10(), self.low.log10(), self.high.log10())
        } else {
            (n, self.low, self.high)
        };
        let p = self.from + (n - low) / (high - low) * (self.to - self.from);
        p.is_finite().then_some(p.clamp(-1e9, 1e9))
    }

    fn ticks(&self, settings: &Settings, key: &str) -> Vec<(f64, bool)> {
        let (low, high) = if self.log {
            (self.low.log10(), self.high.log10())
        } else {
            (self.low, self.high)
        };
        let requested = numeric(
            settings,
            &format!("{key} tick major"),
            if self.log {
                10.
            } else {
                nice_step((high - low) / 5.)
            },
        );
        let mut step = if self.log {
            requested.log10()
        } else {
            requested
        };
        if !step.is_finite() || step <= 0. || (high - low) / step > 200. {
            step = if self.log {
                ((high - low) / 5.).ceil().max(1.)
            } else {
                nice_step((high - low) / 5.)
            };
        }
        let start = (low / step + 1e-9).floor() * step;
        let minors = numeric(
            settings,
            &format!("{key} tick minor ticks"),
            if self.log { 8. } else { 1. },
        ) as u32;
        let mut output = Vec::new();
        for index in 0..=202 {
            let start = start + f64::from(index) * step;
            if start > high + step * 1e-8 {
                break;
            }
            for minor in 0..=minors.min(20) {
                let adjusted = if self.log && (step - 1.).abs() < 1e-10 {
                    start + (f64::from(minor) + 1.).log10()
                } else {
                    start + step * f64::from(minor) / (f64::from(minors.min(20)) + 1.)
                };
                if adjusted >= low - step * 1e-8 && adjusted <= high + step * 1e-8 {
                    let n = if self.log {
                        10_f64.powf(adjusted)
                    } else if adjusted.abs() < step * 1e-10 {
                        0.
                    } else {
                        adjusted
                    };
                    output.push((n, minor == 0));
                }
            }
        }
        output
    }
}

struct Page<'a> {
    project: &'a Project,
    scale: f64,
    height: f64,
    output: String,
}

impl Page<'_> {
    fn path(&mut self, data: &str, stroke: &str, width: f64, style: u32, fill: &str) {
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

    fn label(
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
        let text = xml(text);
        let _ = write!(
            self.output,
            "<text x=\"{x:.3}\" y=\"{y:.3}\" fill=\"{ink}\" font-size=\"{size:.3}\" text-anchor=\"{anchor}\" transform=\"rotate({rotation} {x:.3} {y:.3})\">{text}</text>"
        );
    }

    fn graph(&mut self, id: u32, graph: &Graph) {
        if !graph.visible || !graph.kind.eq_ignore_ascii_case("xy") {
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
        for grid in [true, false] {
            if !grid {
                let _ = write!(self.output, "<g clip-path=\"url(#graph-{id})\">");
                for set in graph.sets.values() {
                    self.set(set, &x, &y);
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
            self.axis(&x, settings, "xaxis", bottom, top, grid);
            self.axis(&y, settings, "yaxis", left, right, grid);
        }
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
        self.legend(graph, &x, &y);
        self.output.push_str("</g>");
    }

    fn axis(
        &mut self,
        axis: &Axis,
        settings: &Settings,
        key: &str,
        normal: f64,
        opposite: f64,
        grid: bool,
    ) {
        if !enabled(settings, key, true) {
            return;
        }
        let horiz = key == "xaxis";
        let p = |coordinate: f64, crossed: f64| {
            if horiz {
                (coordinate, crossed)
            } else {
                (crossed, coordinate)
            }
        };
        let tick_place = value(settings, &format!("{key} tick place"), "both");
        let ink = paint(self.project, settings, &format!("{key} ticklabel color"), 1);
        let unit = self.scale;
        let size = unit * FONT_SCALE * numeric(settings, &format!("{key} ticklabel char size"), 1.);
        for (n, major) in axis.ticks(settings, key) {
            let Some(coordinate) = axis.at(n) else {
                continue;
            };
            let kind = if major { "major" } else { "minor" };
            let color = paint(
                self.project,
                settings,
                &format!("{key} tick {kind} color"),
                1,
            );
            if grid {
                if enabled(settings, &format!("{key} tick {kind} grid"), false) {
                    let (x1, y1) = p(coordinate, normal);
                    let (x2, y2) = p(coordinate, opposite);
                    self.path(
                        &format!("M{x1:.3},{y1:.3}L{x2:.3},{y2:.3}"),
                        &color,
                        unit * 0.0015,
                        1,
                        "none",
                    );
                }
                continue;
            }
            if enabled(settings, &format!("{key} tick"), true) {
                let length = unit
                    * 0.02
                    * numeric(
                        settings,
                        &format!("{key} tick {kind} size"),
                        if major { 1. } else { 0.5 },
                    );
                let sign = (opposite - normal).signum();
                for (side, position, direction) in
                    [("normal", normal, sign), ("opposite", opposite, -sign)]
                {
                    if tick_place != "both" && tick_place != side {
                        continue;
                    }
                    let (x1, y1) = p(coordinate, position);
                    let (x2, y2) = p(coordinate, position + length * direction);
                    self.path(
                        &format!("M{x1:.3},{y1:.3}L{x2:.3},{y2:.3}"),
                        &color,
                        unit * 0.0015,
                        1,
                        "none",
                    );
                }
            }
            if major && enabled(settings, &format!("{key} ticklabel"), true) {
                let prec =
                    numeric(settings, &format!("{key} ticklabel prec"), 5.).clamp(0., 12.) as usize;
                let format = value(settings, &format!("{key} ticklabel format"), "general");
                let text = tick_text(n, prec, format);
                let text = format!(
                    "{}{}{}",
                    unquote(value(settings, &format!("{key} ticklabel prepend"), "")),
                    text,
                    unquote(value(settings, &format!("{key} ticklabel append"), ""))
                );
                let (x, y) = p(
                    coordinate,
                    normal
                        + if horiz {
                            size + unit * 0.01
                        } else {
                            -unit * 0.01
                        },
                );
                self.label(
                    (x, if horiz { y } else { y + size * 0.32 }),
                    &text,
                    size,
                    &ink,
                    if horiz { "middle" } else { "end" },
                    0.,
                );
            }
        }
        if grid {
            return;
        }
        if enabled(settings, &format!("{key} bar"), true) {
            let (x1, y1) = p(axis.from, normal);
            let (x2, y2) = p(axis.to, normal);
            let color = paint(self.project, settings, &format!("{key} bar color"), 1);
            let width = numeric(settings, &format!("{key} bar linewidth"), 1.) * unit * 0.0015;
            self.path(
                &format!("M{x1:.3},{y1:.3}L{x2:.3},{y2:.3}"),
                &color,
                width,
                1,
                "none",
            );
        }
        let (x, y) = p(
            (axis.from + axis.to) * 0.5,
            normal
                + if horiz {
                    size + unit * 0.045
                } else {
                    -unit * 0.075
                },
        );
        let ink = paint(self.project, settings, &format!("{key} label color"), 1);
        let label_size =
            numeric(settings, &format!("{key} label char size"), 1.) * unit * FONT_SCALE;
        self.label(
            (x, y),
            unquote(value(settings, &format!("{key} label"), "")),
            label_size,
            &ink,
            "middle",
            if horiz { 0. } else { -90. },
        );
    }

    fn set(&mut self, set: &Set, x: &Axis, y: &Axis) {
        if value(&set.settings, "hidden", "false") == "true" {
            return;
        }
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

fn tick_text(n: f64, precision: usize, format: &str) -> String {
    match format {
        "decimal" => format!("{n:.precision$}"),
        "exponential" => format!("{n:.precision$e}"),
        _ if n != 0. && !(0.0001..1_000_000.).contains(&n.abs()) => format!("{n:.precision$e}"),
        _ => {
            let decimals = if n == 0. {
                0
            } else {
                (precision as i32 - n.abs().log10().floor() as i32 - 1).clamp(0, 12) as usize
            };
            let text = format!("{n:.decimals$}");
            if decimals == 0 {
                text
            } else {
                text.trim_end_matches('0').trim_end_matches('.').into()
            }
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

fn symbol_path(path: &mut String, symbol: u32, x: f64, y: f64, radius: f64) {
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
    page.output.push_str("</svg>");
    page.output
}
