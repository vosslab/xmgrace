use crate::plot::{FONT_SCALE, Page, enabled, nice_step, numeric, paint};
use crate::scale::Scale;
use crate::{Graph, Settings, unquote, value};

pub(crate) struct Axis {
    low: f64,
    high: f64,
    from: f64,
    to: f64,
    scale: Scale,
}

impl Axis {
    pub(crate) fn new(graph: &Graph, name: &str, index: usize, from: f64, to: f64) -> Self {
        let invert = enabled(&graph.settings, &format!("{name}axes invert"), false);
        let (from, to) = if invert { (to, from) } else { (from, to) };
        Self {
            low: graph.world[index],
            high: graph.world[index + 2],
            from,
            to,
            scale: Scale::from_settings(&graph.settings, name),
        }
    }

    pub(crate) fn span(&self) -> f64 {
        self.high - self.low
    }

    pub(crate) fn at(&self, n: f64) -> Option<f64> {
        let n = self.scale.project(n)?;
        let low = self.scale.project(self.low)?;
        let high = self.scale.project(self.high)?;
        if high == low {
            return None;
        }
        let p = self.from + (n - low) / (high - low) * (self.to - self.from);
        p.is_finite().then_some(p.clamp(-1e9, 1e9))
    }

    fn ticks(&self, settings: &Settings, key: &str) -> Vec<(f64, bool)> {
        let (low, high) = if self.scale.is_log() {
            (self.low.log10(), self.high.log10())
        } else {
            (self.low, self.high)
        };
        let requested = numeric(
            settings,
            &format!("{key} tick major"),
            if self.scale.is_log() {
                10.
            } else {
                nice_step((high - low) / 5.)
            },
        );
        let mut step = if self.scale.is_log() {
            requested.log10()
        } else {
            requested
        };
        if !step.is_finite() || step <= 0. || (high - low) / step > 200. {
            step = if self.scale.is_log() {
                ((high - low) / 5.).ceil().max(1.)
            } else {
                nice_step((high - low) / 5.)
            };
        }
        let start = (low / step + 1e-9).floor() * step;
        let minors = numeric(
            settings,
            &format!("{key} tick minor ticks"),
            if self.scale.is_log() { 8. } else { 1. },
        ) as u32;
        let mut output = Vec::new();
        for index in 0..=202 {
            let start = start + f64::from(index) * step;
            if start > high + step * 1e-8 {
                break;
            }
            for minor in 0..=minors.min(20) {
                let adjusted = if self.scale.is_log() && (step - 1.).abs() < 1e-10 {
                    start + (f64::from(minor) + 1.).log10()
                } else {
                    start + step * f64::from(minor) / (f64::from(minors.min(20)) + 1.)
                };
                if adjusted >= low - step * 1e-8 && adjusted <= high + step * 1e-8 {
                    let n = if self.scale.is_log() {
                        let Some(value) = self.scale.unproject(adjusted) else {
                            continue;
                        };
                        value
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

impl Page<'_> {
    pub(crate) fn axis(
        &mut self,
        axis: &Axis,
        settings: &Settings,
        key: &str,
        (normal, opposite): (f64, f64),
        grid: bool,
        alternate: bool,
    ) {
        if !enabled(settings, key, !alternate) || (alternate && grid) {
            return;
        }
        let horiz = matches!(key, "xaxis" | "altxaxis");
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
        let tick_direction = value(settings, &format!("{key} tick direction"), "in");
        let ticklabel_place = value(settings, &format!("{key} ticklabel place"), "normal");
        let stagger =
            numeric(settings, &format!("{key} ticklabel stagger"), 0.).clamp(0., 10.) as usize;
        let mut major_index = 0usize;
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
                    let (start, end) = match tick_direction {
                        "out" => (position, position - length * direction),
                        "both" => (position - length * direction, position + length * direction),
                        _ => (position, position + length * direction),
                    };
                    let (x1, y1) = p(coordinate, start);
                    let (x2, y2) = p(coordinate, end);
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
                let level = major_index % (stagger + 1);
                major_index += 1;
                let prec =
                    numeric(settings, &format!("{key} ticklabel prec"), 5.).clamp(0., 12.) as usize;
                let format = value(settings, &format!("{key} ticklabel format"), "general");
                let text = tick_text(n, prec, format, self.project.reference_date);
                let text = format!(
                    "{}{}{}",
                    unquote(value(settings, &format!("{key} ticklabel prepend"), "")),
                    text,
                    unquote(value(settings, &format!("{key} ticklabel append"), ""))
                );
                for (side, position, other) in
                    [("normal", normal, opposite), ("opposite", opposite, normal)]
                {
                    if ticklabel_place != "both" && ticklabel_place != side {
                        continue;
                    }
                    let outward = (position - other).signum();
                    let offset = if horiz {
                        size + unit * 0.01
                    } else {
                        unit * 0.01
                    } + level as f64 * size * 1.25;
                    let (x, y) = p(coordinate, position + outward * offset);
                    self.label(
                        (x, if horiz { y } else { y + size * 0.32 }),
                        &text,
                        size,
                        &ink,
                        if horiz {
                            "middle"
                        } else if outward < 0. {
                            "end"
                        } else {
                            "start"
                        },
                        0.,
                    );
                }
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
        let outside = (normal - opposite).signum();
        let (x, y) = p(
            (axis.from + axis.to) * 0.5,
            normal
                + outside
                    * if horiz {
                        size + unit * 0.045
                    } else {
                        unit * 0.075
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
}

fn tick_text(n: f64, precision: usize, format: &str, reference_date: f64) -> String {
    if let Some(label) =
        crate::tick_format::format_with_reference(n, precision, format, reference_date)
    {
        return label;
    }
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
