use crate::appearance::{self, AppearanceOperation};
use crate::data_commands::DataCommand;
use crate::graph_edit::GraphOperation;
use crate::objects::{self, ObjectOperation};
use crate::regions::{self, RegionOperation};
use crate::scale::Scale;
use crate::{Diagnostic, Graph, Project, ascii, autoscale, dataset, graph_edit, set_world};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(untagged)]
enum Command {
    Data(DataCommand),
    Plot(PlotCommand),
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum PlotCommand {
    Appearance {
        operation: AppearanceOperation,
    },
    GraphOperation {
        operation: GraphOperation,
    },
    ObjectOperation {
        operation: ObjectOperation,
    },
    RegionOperation {
        operation: RegionOperation,
    },
    AutoTicks {
        graph: u32,
    },
    AutoOn {
        graph: u32,
        x: f64,
        y: f64,
    },
    Autoscale {
        graph: u32,
        axes: String,
    },
    World {
        graph: u32,
        bounds: [f64; 4],
    },
    Axis {
        graph: u32,
        axis: String,
        min: f64,
        max: f64,
        major: f64,
        label: String,
        scale: String,
    },
    Graph {
        graph: u32,
        title: String,
        subtitle: String,
    },
    Set {
        graph: u32,
        set: u32,
        label: String,
        color: u8,
        symbol: u8,
        line: u8,
        hidden: bool,
    },
}

fn label(text: &str) -> Result<String, Diagnostic> {
    if text.contains(['\r', '\n']) {
        return Err(Diagnostic::new("Enter text on one line."));
    }
    Ok(crate::quote(text))
}

impl Project {
    pub fn load_parameters(&mut self, graph: u32, bytes: &[u8]) -> Result<(), Diagnostic> {
        crate::parameters::load(self, graph, bytes)
    }

    pub fn save_parameters(&self, graph: u32) -> Result<Vec<u8>, Diagnostic> {
        crate::parameters::save(self, graph)
    }

    /// Runs user-selected commands as a single edit.
    pub fn run_script(&mut self, graph: u32, input: &str) -> Result<(), Diagnostic> {
        crate::script::execute(self, input, graph)
    }

    /// Counts each graph set's points within the selected region.
    pub fn region_report(&self, graph: u32, id: u32) -> Result<Vec<(u32, usize)>, Diagnostic> {
        let selected = self
            .regions
            .get(&id)
            .ok_or_else(|| Diagnostic::new("Select an existing region."))?;
        let graph_data = self.graph_for_data(graph)?;
        Ok(graph_data
            .sets
            .iter()
            .map(|(&id, set)| {
                let count = set.rows.iter().filter(|row| {
                matches!(row.as_slice(), [x,y,..] if selected.contains(graph, *x, *y))
            }).count();
                (id, count)
            })
            .collect())
    }

    /// Reads a new local text file into an existing set while preserving its style.
    pub fn reload_data(&mut self, graph: u32, set: u32, bytes: &[u8]) -> Result<(), Diagnostic> {
        let graph = self.graph_mut(graph)?;
        let stored = graph
            .sets
            .get(&set)
            .ok_or_else(|| Diagnostic::new("Select an existing set."))?;
        let kind = dataset::SetKind::parse(&stored.kind)?;
        let options = ascii::ImportOptions {
            delimiter: ascii::Delimiter::Auto,
            header: false,
            kind,
            columns: (0..kind.width()).map(Some).collect(),
            extra_y: Vec::new(),
            date_x: false,
        };
        let mut sets = ascii::parse(bytes, options)?;
        if sets.len() != 1 {
            return Err(Diagnostic::new("Choose a file containing one data set."));
        }
        let rows = sets.remove(0).rows;
        dataset::replace(graph, Some(set), kind, rows)?;
        autoscale(graph, "xy");
        Ok(())
    }

    pub fn import_netcdf(
        &mut self,
        graph: u32,
        bytes: &[u8],
        x: Option<&str>,
        y: &str,
    ) -> Result<(), Diagnostic> {
        let graph = self.graph_mut(graph)?;
        let rows = crate::netcdf::read_xy(bytes, x, y)?;
        crate::data_commands::append_xy(graph, rows, "Imported NetCDF data")
    }

    /// Applies the command representation shared by the native and browser hosts.
    pub fn apply_command_json(&mut self, input: &str) -> Result<(), Diagnostic> {
        let command = serde_json::from_str(input)
            .map_err(|error| Diagnostic::new(format!("Invalid Grace command: {error}")))?;
        match command {
            Command::Data(command) => command.apply(self),
            Command::Plot(command) => self.apply_command(command),
        }
    }

    /// Reads a page of data without copying the entire project to the caller.
    pub fn data_table(
        &self,
        graph: u32,
        set: u32,
        offset: usize,
        limit: usize,
    ) -> Result<&[Vec<f64>], Diagnostic> {
        dataset::table(self.graph_for_data(graph)?, set, offset, limit)
    }

    pub fn export_ascii(&self, graph: u32, set: u32) -> Result<Vec<u8>, Diagnostic> {
        self.require_editable()?;
        dataset::export_ascii(self.graph_for_data(graph)?, set)
    }

    pub fn import_mapped(
        &mut self,
        graph: u32,
        bytes: &[u8],
        options: &str,
    ) -> Result<(), Diagnostic> {
        let options: ascii::ImportOptions = serde_json::from_str(options)
            .map_err(|error| Diagnostic::new(format!("Invalid table import options: {error}")))?;
        let reference_date = self.reference_date;
        let use_date = options.date_x;
        let graph = self.graph_mut(graph)?;
        let mut sets = ascii::parse(bytes, options)?;
        if use_date && reference_date != 0. {
            for set in &mut sets {
                for row in &mut set.rows {
                    row[0] -= reference_date;
                }
            }
        }
        dataset::append(graph, sets)?;
        autoscale(graph, "xy");
        Ok(())
    }

    fn graph_for_data(&self, graph: u32) -> Result<&Graph, Diagnostic> {
        self.graphs
            .get(&graph)
            .ok_or_else(|| Diagnostic::new("Select an existing graph."))
    }
}

impl Project {
    fn apply_command(&mut self, command: PlotCommand) -> Result<(), Diagnostic> {
        match command {
            PlotCommand::Appearance { operation } => appearance::apply(self, operation)?,
            PlotCommand::GraphOperation { operation } => graph_edit::apply(self, operation)?,
            PlotCommand::ObjectOperation { operation } => objects::apply(self, operation)?,
            PlotCommand::RegionOperation { operation } => regions::apply(self, operation)?,
            PlotCommand::AutoTicks { graph } => crate::auto_ticks(self.graph_mut(graph)?),
            PlotCommand::AutoOn { graph, x, y } => crate::auto_on(self.graph_mut(graph)?, x, y)?,
            PlotCommand::Autoscale { graph, axes } => autoscale(self.graph_mut(graph)?, &axes),
            PlotCommand::World { graph, bounds } => set_world(self.graph_mut(graph)?, bounds)?,
            PlotCommand::Axis {
                graph,
                axis,
                min,
                max,
                major,
                label: text,
                scale,
            } => {
                let index = match axis.as_str() {
                    "x" => 0,
                    "y" => 1,
                    _ => return Err(Diagnostic::new("Select X or Y axis.")),
                };
                let scale = Scale::parse(&scale)
                    .ok_or_else(|| Diagnostic::new("Select a supported axis scale."))?;
                if !major.is_finite()
                    || major <= if scale.is_log() { 1. } else { 0. }
                    || !scale.valid_range(min, max)
                {
                    return Err(Diagnostic::new(
                        "Invalid major spacing or logarithmic range.",
                    ));
                }
                let text = label(&text)?;
                let graph = self.graph_mut(graph)?;
                let mut bounds = graph.world;
                bounds[index] = min;
                bounds[index + 2] = max;
                let key = format!("{axis}axes scale");
                let previous = graph.settings.insert(key.clone(), scale.name().into());
                if let Err(error) = set_world(graph, bounds) {
                    if let Some(old) = previous {
                        graph.settings.insert(key, old);
                    } else {
                        graph.settings.remove(&key);
                    }
                    return Err(error);
                }
                graph
                    .settings
                    .insert(format!("{axis}axis tick major"), major.to_string());
                graph.settings.insert(format!("{axis}axis label"), text);
            }
            PlotCommand::Graph {
                graph,
                title,
                subtitle,
            } => {
                let (title, subtitle) = (label(&title)?, label(&subtitle)?);
                let graph = self.graph_mut(graph)?;
                graph.settings.insert("title".into(), title);
                graph.settings.insert("subtitle".into(), subtitle);
            }
            PlotCommand::Set {
                graph,
                set,
                label: text,
                color,
                symbol,
                line,
                hidden,
            } => {
                if color > 15 || symbol > 11 || line > 3 {
                    return Err(Diagnostic::new("Invalid set appearance."));
                }
                let text = label(&text)?;
                let set = self
                    .graph_mut(graph)?
                    .sets
                    .get_mut(&set)
                    .ok_or_else(|| Diagnostic::new("Select an existing set."))?;
                for (key, value) in [
                    ("legend", text),
                    ("symbol", symbol.to_string()),
                    ("line type", line.to_string()),
                    ("line color", color.to_string()),
                    ("symbol color", color.to_string()),
                    ("hidden", hidden.to_string()),
                ] {
                    set.settings.insert(key.into(), value);
                }
            }
        }
        Ok(())
    }
}
