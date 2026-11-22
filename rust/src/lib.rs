//! Grace project data and host-independent plot operations.
mod advanced;
mod agr;
mod appearance;
mod ascii;
mod commands;
mod data_commands;
mod dataset;
mod expression;
mod fitting;
mod fourier;
mod grace_text;
mod graph_edit;
mod netcdf;
mod objects;
mod parameters;
mod plot;
mod plot_axes;
mod plot_objects;
mod plot_set_fill;
mod plot_special;
mod regions;
mod scale;
mod script;
mod series;
mod tick_format;

use serde::Serialize;
use std::collections::BTreeMap;

pub use agr::{open, save};
pub use plot::svg;

pub fn netcdf_variables(bytes: &[u8]) -> Result<Vec<(String, usize)>, Diagnostic> {
    Ok(netcdf::variables(bytes)?
        .into_iter()
        .map(|item| (item.name, item.length))
        .collect())
}

pub type Settings = BTreeMap<String, String>;

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

#[derive(Debug, Clone, Default, Serialize)]
pub struct Set {
    pub settings: Settings,
    #[serde(skip)]
    pub rows: Vec<Vec<f64>>,
    pub count: usize,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub page: [f64; 2],
    pub graphs: BTreeMap<u32, Graph>,
    pub diagnostics: Vec<Diagnostic>,
    pub console: Vec<String>,
    pub(crate) objects: BTreeMap<u32, objects::DrawingObject>,
    pub(crate) regions: BTreeMap<u32, regions::Region>,
    #[serde(skip)]
    pub colors: BTreeMap<u32, [u8; 3]>,
    #[serde(skip)]
    pub color_commands: Vec<String>,
    #[serde(skip)]
    pub timestamp: Option<String>,
    #[serde(skip)]
    pub(crate) reference_date: f64,
    pub background: u32,
    pub fill_page: bool,
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}

impl Project {
    /// Creates the editable project shown for a new browser document.
    pub fn new() -> Self {
        let mut project = Self::empty();
        project.graphs.insert(0, Graph::default());
        project
    }

    /// Starts an AGR import without assuming the input declares graph zero.
    pub(crate) fn for_agr_import() -> Self {
        Self::empty()
    }

    fn empty() -> Self {
        Self {
            page: [792., 612.],
            graphs: BTreeMap::new(),
            diagnostics: Vec::new(),
            console: Vec::new(),
            objects: BTreeMap::new(),
            regions: BTreeMap::new(),
            colors: BTreeMap::new(),
            color_commands: Vec::new(),
            timestamp: None,
            reference_date: 0.,
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
    let mut quoted = String::from("\"");
    let mut slash = false;
    for character in text.chars() {
        if character == '"' && !slash {
            quoted.push('\\');
        }
        quoted.push(if matches!(character, '\r' | '\n') {
            ' '
        } else {
            character
        });
        slash = !slash && character == '\\';
    }
    if slash {
        quoted.push('\\');
    }
    quoted.push('"');
    quoted
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
    let sets = sets
        .into_iter()
        .map(dataset::xy)
        .collect::<Result<_, _>>()?;
    dataset::append(graph, sets)?;
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
    if ["x", "y"].into_iter().enumerate().any(|(index, axis)| {
        let scale = scale::Scale::from_settings(&graph.settings, axis);
        let (low, high) = (bounds[index], bounds[index + 2]);
        !scale.valid_range(low, high)
            || scale
                .project(low)
                .zip(scale.project(high))
                .is_none_or(|(a, b)| a == b || !(b - a).is_finite())
    }) {
        return Err(Diagnostic::new(
            "World limits must increase and be valid for the axis scale.",
        ));
    }
    graph.world = bounds;
    Ok(())
}

pub fn autoscale(graph: &mut Graph, axes: &str) {
    autoscale_sets(graph, axes, None);
}

fn auto_ticks(graph: &mut Graph) {
    for (axis, index) in [("x", 0), ("y", 1)] {
        let scale = scale::Scale::from_settings(&graph.settings, axis);
        let major = if scale.is_log() {
            10.
        } else {
            plot::nice_step((graph.world[index + 2] - graph.world[index]) / 5.)
        };
        graph
            .settings
            .insert(format!("{axis}axis tick major"), major.to_string());
    }
}

fn auto_on(graph: &mut Graph, x: f64, y: f64) -> Result<(), Diagnostic> {
    let axis_distance = |axis: &str, index: usize, target: f64, value: f64| {
        let scale = scale::Scale::from_settings(&graph.settings, axis);
        let normal = |point| scale.project(point).unwrap_or(f64::NAN);
        (normal(value) - normal(target))
            / (normal(graph.world[index + 2]) - normal(graph.world[index]))
    };
    let mut closest = None;
    let mut distance = f64::INFINITY;
    for (&id, set) in &graph.sets {
        if value(&set.settings, "hidden", "false") == "true" {
            continue;
        }
        for row in &set.rows {
            if let [px, py, ..] = row.as_slice() {
                let next = axis_distance("x", 0, x, *px).hypot(axis_distance("y", 1, y, *py));
                if next < distance {
                    closest = Some(id);
                    distance = next;
                }
            }
        }
    }
    let set = closest.ok_or_else(|| Diagnostic::new("No visible set to autoscale."))?;
    autoscale_sets(graph, "xy", Some(set));
    Ok(())
}

fn autoscale_sets(graph: &mut Graph, axes: &str, selected: Option<u32>) {
    for (index, axis) in ["x", "y"].iter().enumerate() {
        if !axes.contains(axis) {
            continue;
        }
        let scale = scale::Scale::from_settings(&graph.settings, axis);
        let log = scale.is_log();
        let compatible = |point: f64| {
            scale.project(point).is_some()
                && (scale != scale::Scale::Reciprocal
                    || point.signum() == graph.world[index].signum())
        };
        let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
        for set in graph
            .sets
            .iter()
            .filter(|(id, set)| {
                selected.is_none_or(|selected| selected == **id)
                    && value(&set.settings, "hidden", "false") != "true"
            })
            .map(|(_, set)| set)
        {
            for row in &set.rows {
                let Some(&coordinate) = row.get(index) else {
                    continue;
                };
                let (lower, upper) = plot::errors(row, &set.kind, index);
                let min = coordinate - lower;
                let max = coordinate + upper;
                if compatible(min) {
                    low = low.min(min);
                }
                if compatible(max) {
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
        } else if scale == scale::Scale::Normal {
            (
                (low / step).floor() * step,
                (high / step).ceil() * step,
                step,
            )
        } else {
            (low, high, step)
        };
        if scale.valid_range(min, max) {
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
