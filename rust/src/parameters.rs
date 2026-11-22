//! Grace graph parameter-file codec.
//!
//! Desktop Grace writes parameter files as ordinary AGR declarations without
//! the leading at sign used by project files. This module accepts only graph
//! declarations that can be safely applied to an existing browser graph.

use crate::{Diagnostic, Project, agr, set_world};
use std::fmt::Write;

/// Loads one desktop Grace graph parameter file into an existing graph.
///
/// The decoded declarations are first parsed as a disposable G0 project. A
/// valid result replaces only graph properties, preserving every existing set
/// and its data. The project remains unchanged on every failure.
pub(crate) fn load(project: &mut Project, graph: u32, bytes: &[u8]) -> Result<(), Diagnostic> {
    project.require_editable()?;
    let current = project
        .graphs
        .get(&graph)
        .ok_or_else(|| Diagnostic::new("Select an existing graph."))?;
    let source = synthetic_project(bytes)?;
    let staged = agr::open(source.as_bytes()).map_err(|error| {
        Diagnostic::new(format!("Invalid Grace parameter file: {}", error.message))
    })?;
    if !staged.diagnostics.is_empty() {
        return Err(Diagnostic::new(format!(
            "Parameter file contains unsupported declaration: {}",
            staged.diagnostics[0].message
        )));
    }
    let parameters = staged
        .graphs
        .get(&0)
        .ok_or_else(|| Diagnostic::new("Parameter file contains no graph declarations."))?;
    if !current
        .sets
        .values()
        .all(|set| agr::set_kind_supported_for_graph(&parameters.kind, &set.kind))
    {
        return Err(Diagnostic::new(
            "Parameter graph type is incompatible with this graph's data sets.",
        ));
    }

    let mut replacement = current.clone();
    replacement.world = parameters.world;
    replacement.view = parameters.view;
    replacement.settings = parameters.settings.clone();
    replacement.visible = parameters.visible;
    replacement.kind = parameters.kind.clone();
    let world = replacement.world;
    set_world(&mut replacement, world)?;
    project.graphs.insert(graph, replacement);
    Ok(())
}

/// Serializes the selected graph as a classic Grace parameter file.
///
/// The file intentionally has no data rows or runtime commands. Desktop
/// Grace reads it through -param or the plot-appearance File menu.
pub(crate) fn save(project: &Project, graph: u32) -> Result<Vec<u8>, Diagnostic> {
    project.require_editable()?;
    let graph = project
        .graphs
        .get(&graph)
        .ok_or_else(|| Diagnostic::new("Select an existing graph."))?;
    let mut output =
        String::from("# Grace graph parameter file\n# Browser Grace declarations\nversion 50125\n");
    let _ = writeln!(
        output,
        "g0 on\ng0 hidden {}\ng0 type {}\nwith g0",
        !graph.visible, graph.kind
    );
    for (key, [a, b, c, d]) in [("world", graph.world), ("view", graph.view)] {
        let _ = writeln!(output, "    {key} {a}, {b}, {c}, {d}");
    }
    for (key, value) in &graph.settings {
        let key = if key == "legend position" {
            "legend"
        } else {
            key
        };
        let _ = writeln!(output, "    {key} {value}");
    }
    Ok(output.into_bytes())
}

fn synthetic_project(bytes: &[u8]) -> Result<String, Diagnostic> {
    let mut output = String::from("@version 50125\n@g0 on\n@with g0\n");
    let mut declarations = 0;
    for (index, raw) in agr::decode(bytes).lines().enumerate() {
        let line = index + 1;
        let command = raw.trim().trim_start_matches('@').trim();
        if command.is_empty() || command.starts_with('#') {
            continue;
        }
        if command
            .strip_prefix("version ")
            .is_some_and(|version| version.trim().parse::<u32>().is_ok())
        {
            continue;
        }
        if let Some(rest) = graph_declaration(command) {
            let _ = writeln!(output, "@g0 {rest}");
            declarations += 1;
            continue;
        }
        if is_graph_context(command) {
            continue;
        }
        if forbidden(command) {
            return Err(at(
                line,
                "Parameter files cannot contain data, objects, regions, or commands.",
            ));
        }
        let _ = writeln!(output, "@    {command}");
        declarations += 1;
    }
    if declarations == 0 {
        return Err(Diagnostic::new(
            "Parameter file contains no graph declarations.",
        ));
    }
    Ok(output)
}

fn graph_declaration(command: &str) -> Option<&str> {
    let rest = command.strip_prefix('g')?;
    let length = rest.bytes().take_while(u8::is_ascii_digit).count();
    (length > 0).then_some(rest[length..].trim_start())
}

fn is_graph_context(command: &str) -> bool {
    let Some(rest) = command.strip_prefix("with g") else {
        return false;
    };
    !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit())
}

fn forbidden(command: &str) -> bool {
    let head = command.split_ascii_whitespace().next().unwrap_or_default();
    matches!(
        head.to_ascii_lowercase().as_str(),
        "target" | "type" | "link" | "map" | "with" | "timestamp" | "&"
    ) || head.strip_prefix('s').is_some_and(|number| {
        !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
    }) || head.strip_prefix('r').is_some_and(|number| {
        !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
    }) || head
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.'))
}

fn at(line: usize, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        message: message.into(),
        line: Some(line),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import_ascii;

    #[test]
    fn round_trips_graph_declarations_without_replacing_data() {
        let mut project = Project::new();
        import_ascii(&mut project, 0, b"0 1\n1 2\n").expect("data");
        {
            let graph = project.graphs.get_mut(&0).expect("graph");
            graph
                .settings
                .insert("title".into(), crate::quote("Parameters"));
            graph
                .settings
                .insert("xaxis label".into(), crate::quote("Time"));
            graph.world = [-2., -1., 3., 4.];
        }
        let bytes = save(&project, 0).expect("save parameters");
        let mut target = Project::new();
        import_ascii(&mut target, 0, b"5 6\n7 8\n").expect("target data");
        load(&mut target, 0, &bytes).expect("load parameters");
        let graph = &target.graphs[&0];
        assert_eq!(graph.settings["title"], crate::quote("Parameters"));
        assert_eq!(graph.world, [-2., -1., 3., 4.]);
        assert_eq!(graph.sets[&0].rows, vec![vec![5., 6.], vec![7., 8.]]);
    }

    #[test]
    fn rejects_data_and_preserves_the_target_graph() {
        let mut project = Project::new();
        let original = project.graphs[&0].clone();
        let error =
            load(&mut project, 0, b"version 50125\ntarget G0.S0\n").expect_err("data is forbidden");
        assert_eq!(error.line, Some(2));
        assert_eq!(project.graphs[&0].world, original.world);
    }
}
