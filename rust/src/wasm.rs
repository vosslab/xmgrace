use crate::{Diagnostic, Project, autoscale, import_ascii, open, save, set_world, svg};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Command {
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
        log: bool,
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

fn reject(error: Diagnostic) -> JsValue {
    JsValue::from_str(&serde_json::to_string(&error).expect("diagnostics are serializable"))
}

fn label(text: &str) -> Result<String, Diagnostic> {
    if text.contains(['"', '\\', '\r', '\n']) {
        return Err(Diagnostic::new(
            "Enter plain text without quotes or Grace escapes.",
        ));
    }
    Ok(crate::quote(text))
}

#[wasm_bindgen]
#[derive(Default)]
pub struct Grace {
    project: Project,
}

#[wasm_bindgen]
impl Grace {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_project(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        self.project = open(bytes).map_err(reject)?;
        Ok(())
    }

    pub fn new_project(&mut self) {
        self.project = Project::new();
    }

    pub fn import_data(&mut self, graph: u32, bytes: &[u8]) -> Result<(), JsValue> {
        import_ascii(&mut self.project, graph, bytes).map_err(reject)
    }

    pub fn snapshot(&self) -> String {
        serde_json::to_string(&self.project).expect("project summaries are serializable")
    }

    pub fn render_svg(&self) -> String {
        svg(&self.project)
    }

    pub fn save_project(&self) -> Result<Vec<u8>, JsValue> {
        save(&self.project).map_err(reject)
    }

    pub fn apply(&mut self, command: &str) -> Result<(), JsValue> {
        let command = serde_json::from_str(command)
            .map_err(|_| reject(Diagnostic::new("Invalid Grace command.")))?;
        self.apply_command(command).map_err(reject)
    }
}

impl Grace {
    fn apply_command(&mut self, command: Command) -> Result<(), Diagnostic> {
        match command {
            Command::Autoscale { graph, axes } => autoscale(self.project.graph_mut(graph)?, &axes),
            Command::World { graph, bounds } => set_world(self.project.graph_mut(graph)?, bounds)?,
            Command::Axis {
                graph,
                axis,
                min,
                max,
                major,
                label: text,
                log,
            } => {
                let index = match axis.as_str() {
                    "x" => 0,
                    "y" => 1,
                    _ => return Err(Diagnostic::new("Select X or Y axis.")),
                };
                if !major.is_finite() || major <= if log { 1. } else { 0. } || (log && min <= 0.) {
                    return Err(Diagnostic::new(
                        "Invalid major spacing or logarithmic range.",
                    ));
                }
                let text = label(&text)?;
                let graph = self.project.graph_mut(graph)?;
                let mut bounds = graph.world;
                bounds[index] = min;
                bounds[index + 2] = max;
                let key = format!("{axis}axes scale");
                let previous = graph.settings.insert(
                    key.clone(),
                    if log { "Logarithmic" } else { "Normal" }.into(),
                );
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
            Command::Graph {
                graph,
                title,
                subtitle,
            } => {
                let (title, subtitle) = (label(&title)?, label(&subtitle)?);
                let graph = self.project.graph_mut(graph)?;
                graph.settings.insert("title".into(), title);
                graph.settings.insert("subtitle".into(), subtitle);
            }
            Command::Set {
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
                    .project
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
