//! Grace project data and host-independent plot operations.
mod agr;
mod plot;

use serde::Serialize;
use std::collections::BTreeMap;

pub use agr::{open, save};
pub use plot::svg;

pub type Settings = BTreeMap<String, String>;
pub const TEMPLATE: &[u8] = include_bytes!("../../templates/Default.agr");

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub message: String,
    pub line: Option<usize>,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            line: None,
        }
    }

    fn at(line: usize, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            line: Some(line),
        }
    }
}

#[derive(Debug, Default, Serialize)]
pub struct Set {
    pub settings: Settings,
    #[serde(skip)]
    pub rows: Vec<Vec<f64>>,
    pub count: usize,
    pub kind: String,
}

#[derive(Debug, Serialize)]
pub struct Graph {
    pub world: [f64; 4],
    pub view: [f64; 4],
    pub settings: Settings,
    pub sets: BTreeMap<u32, Set>,
    pub visible: bool,
    pub kind: String,
}

impl Default for Graph {
    fn default() -> Self {
        Self {
            world: [0., 0., 1., 1.],
            view: [0.15, 0.15, 1.15, 0.85],
            settings: Settings::new(),
            sets: BTreeMap::new(),
            visible: true,
            kind: "XY".into(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Project {
    pub page: [f64; 2],
    pub graphs: BTreeMap<u32, Graph>,
    pub diagnostics: Vec<Diagnostic>,
    #[serde(skip)]
    pub colors: BTreeMap<u32, [u8; 3]>,
    #[serde(skip)]
    pub color_commands: Vec<String>,
    #[serde(skip)]
    pub timestamp: Option<String>,
    pub background: u32,
    pub fill_page: bool,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            page: [792., 612.],
            graphs: BTreeMap::new(),
            diagnostics: Vec::new(),
            colors: BTreeMap::new(),
            color_commands: Vec::new(),
            timestamp: None,
            background: 0,
            fill_page: true,
        }
    }
}

fn number(value: &str) -> Option<f64> {
    let value = value.replace(['d', 'D'], "e").parse::<f64>().ok()?;
    value.is_finite().then_some(value)
}

fn value<'a>(settings: &'a Settings, key: &str, default: &'a str) -> &'a str {
    settings.get(key).map_or(default, String::as_str)
}

pub fn quote(text: &str) -> String {
    format!(
        "\"{}\"",
        text.replace('"', "\\\"").replace(['\r', '\n'], " ")
    )
}

pub fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value)
}

pub fn import_ascii(project: &mut Project, graph: u32, bytes: &[u8]) -> Result<(), Diagnostic> {
    project.require_editable()?;
    let text = agr::decode(bytes);
    let mut sets = vec![Vec::new()];
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if line.is_empty() || line == "&" {
            if sets.last().is_some_and(|set| !set.is_empty()) {
                sets.push(Vec::new());
            }
            continue;
        }
        let columns: Option<Vec<_>> = line
            .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
            .filter(|field| !field.is_empty())
            .map(number)
            .collect();
        let Some(row) = columns.filter(|row| row.len() == 2) else {
            return Err(Diagnostic::at(
                index + 1,
                "Expected two finite X and Y columns",
            ));
        };
        sets.last_mut().expect("one dataset exists").push(row);
    }
    sets.retain(|set| !set.is_empty());
    if sets.is_empty() {
        return Err(Diagnostic::new("The file contains no XY data."));
    }
    let graph = project.graph_mut(graph)?;
    for rows in sets {
        let id = graph.sets.last_key_value().map_or(0, |(last, _)| last + 1);
        graph.sets.insert(
            id,
            Set {
                count: rows.len(),
                rows,
                kind: "xy".into(),
                ..Set::default()
            },
        );
    }
    autoscale(graph, "xy");
    Ok(())
}

impl Project {
    pub fn require_editable(&self) -> Result<(), Diagnostic> {
        if self.diagnostics.is_empty() {
            Ok(())
        } else {
            Err(Diagnostic::new(
                "This project is open as a read-only preview.",
            ))
        }
    }

    pub fn graph_mut(&mut self, graph: u32) -> Result<&mut Graph, Diagnostic> {
        self.require_editable()?;
        self.graphs
            .get_mut(&graph)
            .ok_or_else(|| Diagnostic::new("Select an existing graph."))
    }
}

pub fn set_world(graph: &mut Graph, bounds: [f64; 4]) -> Result<(), Diagnostic> {
    if !bounds.iter().all(|v| v.is_finite())
        || bounds[0] >= bounds[2]
        || bounds[1] >= bounds[3]
        || !(bounds[2] - bounds[0]).is_finite()
        || !(bounds[3] - bounds[1]).is_finite()
        || (value(&graph.settings, "xaxes scale", "Normal").eq_ignore_ascii_case("logarithmic")
            && bounds[0] <= 0.)
        || (value(&graph.settings, "yaxes scale", "Normal").eq_ignore_ascii_case("logarithmic")
            && bounds[1] <= 0.)
    {
        return Err(Diagnostic::new(
            "World limits must increase and be valid for the axis scale.",
        ));
    }
    graph.world = bounds;
    Ok(())
}

pub fn autoscale(graph: &mut Graph, axes: &str) {
    for (index, axis) in ["x", "y"].iter().enumerate() {
        if !axes.contains(axis) {
            continue;
        }
        let log = value(&graph.settings, &format!("{axis}axes scale"), "Normal")
            .eq_ignore_ascii_case("logarithmic");
        let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
        for set in graph
            .sets
            .values()
            .filter(|set| value(&set.settings, "hidden", "false") != "true")
        {
            for row in &set.rows {
                let Some(&coordinate) = row.get(index) else {
                    continue;
                };
                let (lower, upper) = plot::errors(row, &set.kind, index);
                let min = coordinate - lower;
                let max = coordinate + upper;
                if min.is_finite() && (!log || min > 0.) {
                    low = low.min(min);
                }
                if max.is_finite() && (!log || max > 0.) {
                    high = high.max(max);
                }
            }
        }
        if !low.is_finite() || !high.is_finite() {
            continue;
        }
        if low == high {
            let margin = if low == 0. { 1. } else { low.abs() * 0.5 };
            low -= margin;
            high += margin;
        }
        let step = plot::nice_step((high - low) / 5.);
        let (min, max, major) = if log {
            (
                10_f64.powf(low.log10().floor()),
                10_f64.powf(high.log10().ceil()),
                10.,
            )
        } else {
            (
                (low / step).floor() * step,
                (high / step).ceil() * step,
                step,
            )
        };
        if min.is_finite() && max.is_finite() && min < max {
            graph.world[index] = min;
            graph.world[index + 2] = max;
            graph
                .settings
                .insert(format!("{axis}axis tick major"), major.to_string());
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm;
