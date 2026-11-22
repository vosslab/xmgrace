//! Browser-safe subset of the Grace command language.
//!
//! This interpreter deliberately only changes the in-memory project.  It has
//! no file, network, process, plug-in, or browser API access.

use crate::data_commands::{DataCommand, append_xy};
use crate::expression::Expression;
use crate::fourier::{Abscissa, Algorithm, Direction, FourierOptions, Input, Output, Window};
use crate::series::{self, RunningStatistic, SeriesOperation};
use crate::{Diagnostic, Project, autoscale, set_world};
use std::collections::BTreeMap;

const MAX_INPUT_BYTES: usize = 65_536;
const MAX_STATEMENTS: usize = 512;

pub(crate) fn execute(project: &mut Project, input: &str, graph: u32) -> Result<(), Diagnostic> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(Diagnostic::new("Script is limited to 65536 bytes."));
    }
    let mut candidate = project.clone();
    let mut state = State {
        graph,
        values: BTreeMap::new(),
    };
    for (line, statement) in statements(input)? {
        execute_statement(&mut candidate, &mut state, statement)
            .map_err(|error| Diagnostic::new(format!("Line {line}: {}", error.message)))?;
    }
    *project = candidate;
    Ok(())
}

struct State {
    graph: u32,
    values: BTreeMap<String, f64>,
}

fn statements(input: &str) -> Result<Vec<(usize, &str)>, Diagnostic> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut line = 1;
    let mut quote = false;
    for (index, character) in input.char_indices() {
        match character {
            '"' => quote = !quote,
            ';' if !quote => push_statement(&mut result, input, start, index, line)?,
            '\n' if !quote => {
                push_statement(&mut result, input, start, index, line)?;
                line += 1;
                start = index + 1;
            }
            _ => continue,
        }
        if character == ';' {
            start = index + 1;
        }
    }
    if quote {
        return Err(Diagnostic::new("Unterminated quoted string in script."));
    }
    push_statement(&mut result, input, start, input.len(), line)?;
    if result.len() > MAX_STATEMENTS {
        return Err(Diagnostic::new("Script is limited to 512 statements."));
    }
    Ok(result)
}

fn push_statement<'a>(
    result: &mut Vec<(usize, &'a str)>,
    input: &'a str,
    start: usize,
    end: usize,
    line: usize,
) -> Result<(), Diagnostic> {
    let statement = input[start..end].trim();
    if !statement.is_empty() && !statement.starts_with('#') {
        result.push((line, statement));
    }
    Ok(())
}

fn execute_statement(
    project: &mut Project,
    state: &mut State,
    statement: &str,
) -> Result<(), Diagnostic> {
    let normalized = statement.trim();
    if let Some(graph) = normalized
        .strip_prefix("with ")
        .or_else(|| normalized.strip_prefix("WITH "))
        .and_then(parse_graph)
    {
        project.graph_mut(graph)?;
        state.graph = graph;
        return Ok(());
    }
    let lower = normalized.to_ascii_lowercase();
    if lower.starts_with("world ") {
        let world = normalized[6..].trim();
        if let Some((component, expression)) = world.split_once(char::is_whitespace) {
            let index = match component.to_ascii_lowercase().as_str() {
                "xmin" => Some(0),
                "ymin" => Some(1),
                "xmax" => Some(2),
                "ymax" => Some(3),
                _ => None,
            };
            if let Some(index) = index {
                let expression = expression.trim().trim_start_matches('=').trim();
                if expression.is_empty() {
                    return Err(Diagnostic::new("WORLD component needs a value."));
                }
                let graph = project.graph_mut(state.graph)?;
                let mut bounds = graph.world;
                bounds[index] = scalar(expression, &state.values)?;
                set_world(graph, bounds)?;
                return Ok(());
            }
        }
        let fields = split_arguments(world)?;
        if fields.len() != 4 {
            return Err(Diagnostic::new("WORLD needs xmin, ymin, xmax, ymax."));
        }
        let values = fields
            .iter()
            .map(|field| scalar(field, &state.values))
            .collect::<Result<Vec<_>, _>>()?;
        set_world(
            project.graph_mut(state.graph)?,
            [values[0], values[1], values[2], values[3]],
        )?;
        return Ok(());
    }
    if lower == "autoscale" || lower.starts_with("autoscale ") {
        let axes = lower.strip_prefix("autoscale").unwrap_or("").trim();
        let axes = if axes.is_empty() { "xy" } else { axes };
        if !matches!(axes, "x" | "y" | "xy") {
            return Err(Diagnostic::new("AUTOSCALE accepts X, Y, or XY."));
        }
        autoscale(project.graph_mut(state.graph)?, axes);
        return Ok(());
    }
    if let Some(text) = quoted_value(normalized, "title")? {
        project
            .graph_mut(state.graph)?
            .settings
            .insert("title".into(), crate::quote(text));
        return Ok(());
    }
    if let Some(text) = quoted_value(normalized, "subtitle")? {
        project
            .graph_mut(state.graph)?
            .settings
            .insert("subtitle".into(), crate::quote(text));
        return Ok(());
    }
    if let Some((set, column, formula)) = data_assignment(normalized)? {
        transform_column(project, state, set, column, formula)?;
        return Ok(());
    }
    if function_call(project, state, normalized)? {
        return Ok(());
    }
    if let Some((name, formula)) = normalized.split_once('=') {
        let name = name.trim().to_ascii_lowercase();
        if !valid_name(&name) {
            return Err(Diagnostic::new("Use a simple variable name before =."));
        }
        let value = scalar(formula, &state.values)?;
        state.values.insert(name, value);
        return Ok(());
    }
    Err(Diagnostic::new("Unsupported Grace command."))
}

fn parse_graph(value: &str) -> Option<u32> {
    let value = value.trim();
    let value = value
        .strip_prefix('g')
        .or_else(|| value.strip_prefix('G'))?;
    value.parse().ok()
}

fn quoted_value<'a>(statement: &'a str, name: &str) -> Result<Option<&'a str>, Diagnostic> {
    let Some(rest) = statement
        .strip_prefix(name)
        .or_else(|| statement.strip_prefix(&name.to_ascii_uppercase()))
    else {
        return Ok(None);
    };
    let rest = rest.trim();
    let value = rest
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .ok_or_else(|| Diagnostic::new(format!("{name} needs one quoted string.")))?;
    Ok(Some(value))
}

fn data_assignment(statement: &str) -> Result<Option<(u32, usize, &str)>, Diagnostic> {
    let Some((left, right)) = statement.split_once('=') else {
        return Ok(None);
    };
    let left = left.trim().to_ascii_lowercase();
    let Some(set) = left.strip_prefix('s').and_then(|rest| rest.split_once('.')) else {
        return Ok(None);
    };
    let set_id = set
        .0
        .parse()
        .map_err(|_| Diagnostic::new("Invalid set identifier."))?;
    let column = match set.1.trim() {
        "x" => 0,
        "y" => 1,
        "y1" => 2,
        "y2" => 3,
        "y3" => 4,
        "y4" => 5,
        _ => {
            return Err(Diagnostic::new(
                "Set assignments support X, Y, and Y1 through Y4.",
            ));
        }
    };
    Ok(Some((set_id, column, right.trim())))
}

fn transform_column(
    project: &mut Project,
    state: &State,
    set: u32,
    column: usize,
    formula: &str,
) -> Result<(), Diagnostic> {
    let graph = project.graph_mut(state.graph)?;
    let data = graph
        .sets
        .get(&set)
        .ok_or_else(|| Diagnostic::new("Select an existing set."))?
        .rows
        .clone();
    if data.iter().any(|row| row.len() <= column) {
        return Err(Diagnostic::new(
            "The selected set has no requested data column.",
        ));
    }
    let names = expression_names(state);
    let references: Vec<&str> = names.iter().map(String::as_str).collect();
    let expression = Expression::parse(formula, &references)?;
    let rows = data
        .into_iter()
        .enumerate()
        .map(|(index, mut row)| {
            let values = expression_values(&row, index, state, &names);
            row[column] = expression.evaluate(&values)?;
            Ok(row)
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let target = graph.sets.get_mut(&set).expect("set was checked");
    target.rows = rows;
    target.count = target.rows.len();
    autoscale(graph, "xy");
    Ok(())
}

fn function_call(
    project: &mut Project,
    state: &State,
    statement: &str,
) -> Result<bool, Diagnostic> {
    let Some((name, args)) = statement.split_once('(') else {
        return Ok(false);
    };
    let Some(args) = args.strip_suffix(')') else {
        return Ok(false);
    };
    let name = name.trim().to_ascii_uppercase();
    let args = split_arguments(args)?;
    if let Some((algorithm, direction)) = fourier_kind(&name) {
        if args.len() != 5 {
            return Err(Diagnostic::new(
                "FFT and DFT need set, REAL/COMPLEX, window, INDEX/FREQUENCY/PERIOD, and output.",
            ));
        }
        DataCommand::Fourier {
            graph: state.graph,
            set: set_argument(args[0])?,
            options: FourierOptions {
                algorithm,
                input: fourier_input(args[1])?,
                direction,
                window: fourier_window(args[2])?,
                abscissa: fourier_abscissa(args[3])?,
                output: fourier_output(args[4])?,
            },
        }
        .apply(project)?;
        return Ok(true);
    }
    let running = match name.as_str() {
        "RUNAVG" => Some(RunningStatistic::Mean),
        "RUNMED" => Some(RunningStatistic::Median),
        "RUNMIN" => Some(RunningStatistic::Minimum),
        "RUNMAX" => Some(RunningStatistic::Maximum),
        "RUNSTD" => Some(RunningStatistic::Stddev),
        _ => None,
    };
    if let Some(statistic) = running {
        if args.len() != 2 {
            return Err(Diagnostic::new(
                "Running statistics need a set and point count.",
            ));
        }
        let set = set_argument(args[0])?;
        let length = scalar(args[1], &state.values)? as usize;
        derive(
            project,
            state.graph,
            set,
            SeriesOperation::Running { length, statistic },
        )?;
        return Ok(true);
    }
    let operation = match name.as_str() {
        "INTEGRATE" if args.len() == 1 => Some(SeriesOperation::Integrate),
        "DIFFERENCE" if (1..=2).contains(&args.len()) => Some(SeriesOperation::Difference {
            scheme: difference_scheme(args.get(1).copied())?,
        }),
        _ => None,
    };
    if let Some(operation) = operation {
        derive(project, state.graph, set_argument(args[0])?, operation)?;
        return Ok(true);
    }
    Ok(false)
}

fn fourier_kind(name: &str) -> Option<(Algorithm, Direction)> {
    match name {
        "FFT" => Some((Algorithm::Fft, Direction::Forward)),
        "INVFFT" => Some((Algorithm::Fft, Direction::Inverse)),
        "DFT" => Some((Algorithm::Dft, Direction::Forward)),
        "INVDFT" => Some((Algorithm::Dft, Direction::Inverse)),
        _ => None,
    }
}

fn fourier_input(value: &str) -> Result<Input, Diagnostic> {
    match value.trim().to_ascii_uppercase().as_str() {
        "REAL" => Ok(Input::Real),
        "COMPLEX" => Ok(Input::Complex),
        _ => Err(Diagnostic::new("Fourier input is REAL or COMPLEX.")),
    }
}

fn fourier_window(value: &str) -> Result<Window, Diagnostic> {
    match value.trim().to_ascii_uppercase().as_str() {
        "NONE" => Ok(Window::None),
        "TRIANGULAR" => Ok(Window::Triangular),
        "HANN" | "HANNING" => Ok(Window::Hann),
        "WELCH" => Ok(Window::Welch),
        "HAMMING" => Ok(Window::Hamming),
        "BLACKMAN" => Ok(Window::Blackman),
        "PARZEN" => Ok(Window::Parzen),
        _ => Err(Diagnostic::new(
            "Fourier window is NONE, TRIANGULAR, HANNING, WELCH, HAMMING, BLACKMAN, or PARZEN.",
        )),
    }
}

fn fourier_abscissa(value: &str) -> Result<Abscissa, Diagnostic> {
    match value.trim().to_ascii_uppercase().as_str() {
        "INDEX" => Ok(Abscissa::Index),
        "FREQUENCY" => Ok(Abscissa::Frequency),
        "PERIOD" => Ok(Abscissa::Period),
        _ => Err(Diagnostic::new(
            "Fourier X output is INDEX, FREQUENCY, or PERIOD.",
        )),
    }
}

fn fourier_output(value: &str) -> Result<Output, Diagnostic> {
    match value.trim().to_ascii_uppercase().as_str() {
        "MAGNITUDE" => Ok(Output::Magnitude),
        "PHASE" => Ok(Output::Phase),
        "COEFFICIENTS" => Ok(Output::Coefficients),
        _ => Err(Diagnostic::new(
            "Fourier output is MAGNITUDE, PHASE, or COEFFICIENTS.",
        )),
    }
}

fn difference_scheme(
    argument: Option<&str>,
) -> Result<crate::series::DifferenceScheme, Diagnostic> {
    match argument
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        None | Some("forward") => Ok(crate::series::DifferenceScheme::Forward),
        Some("backward") => Ok(crate::series::DifferenceScheme::Backward),
        Some("centered") | Some("centred") => Ok(crate::series::DifferenceScheme::Centered),
        _ => Err(Diagnostic::new(
            "DIFFERENCE mode is FORWARD, BACKWARD, or CENTERED.",
        )),
    }
}

fn derive(
    project: &mut Project,
    graph_id: u32,
    set: u32,
    operation: SeriesOperation,
) -> Result<(), Diagnostic> {
    let graph = project.graph_mut(graph_id)?;
    let rows = graph
        .sets
        .get(&set)
        .ok_or_else(|| Diagnostic::new("Select an existing set."))?
        .rows
        .clone();
    let result = series::transform(&rows, operation)?;
    append_xy(graph, result, &format!("Script transformation of S{set}"))
}

fn set_argument(input: &str) -> Result<u32, Diagnostic> {
    input
        .trim()
        .strip_prefix('s')
        .or_else(|| input.trim().strip_prefix('S'))
        .ok_or_else(|| Diagnostic::new("Expected a set such as S0."))?
        .parse()
        .map_err(|_| Diagnostic::new("Invalid set identifier."))
}

fn scalar(input: &str, values: &BTreeMap<String, f64>) -> Result<f64, Diagnostic> {
    let names: Vec<String> = values.keys().cloned().collect();
    let references: Vec<&str> = names.iter().map(String::as_str).collect();
    let expression = Expression::parse(input.trim(), &references)?;
    let inputs: Vec<f64> = names.iter().map(|name| values[name]).collect();
    expression.evaluate(&inputs)
}

fn expression_names(state: &State) -> Vec<String> {
    ["x", "y", "y1", "y2", "y3", "y4", "i"]
        .into_iter()
        .map(str::to_string)
        .chain(state.values.keys().cloned())
        .collect()
}

fn expression_values(row: &[f64], index: usize, state: &State, names: &[String]) -> Vec<f64> {
    names
        .iter()
        .map(|name| match name.as_str() {
            "x" => row[0],
            "y" => row[1],
            "y1" => *row.get(2).unwrap_or(&0.),
            "y2" => *row.get(3).unwrap_or(&0.),
            "y3" => *row.get(4).unwrap_or(&0.),
            "y4" => *row.get(5).unwrap_or(&0.),
            "i" => index as f64,
            _ => state.values[name],
        })
        .collect()
}

fn split_arguments(input: &str) -> Result<Vec<&str>, Diagnostic> {
    if input.contains('"') {
        return Err(Diagnostic::new(
            "Quoted function arguments are unsupported.",
        ));
    }
    Ok(input
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect())
}

fn valid_name(name: &str) -> bool {
    !matches!(
        name,
        "x" | "y" | "y1" | "y2" | "y3" | "y4" | "i" | "pi" | "e"
    ) && name
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset;

    #[test]
    fn applies_commands_atomically() {
        let mut project = Project::new();
        dataset::append(
            project.graph_mut(0).unwrap(),
            vec![dataset::xy(vec![vec![0., 1.], vec![1., 3.]]).unwrap()],
        )
        .unwrap();
        execute(
            &mut project,
            "with g0; title \"Scripted\"; S0.y = y * 2; INTEGRATE(S0)",
            0,
        )
        .unwrap();
        assert_eq!(project.graphs[&0].sets[&0].rows[1][1], 6.);
        assert_eq!(project.graphs[&0].sets.len(), 2);
        assert_eq!(
            crate::unquote(&project.graphs[&0].settings["title"]),
            "Scripted"
        );
        assert!(execute(&mut project, "S0.y = 1 / 0", 0).is_err());
        assert_eq!(project.graphs[&0].sets[&0].rows[1][1], 6.);
    }

    #[test]
    fn executes_classic_fft_and_dft_forms() {
        let mut project = Project::new();
        dataset::append(
            project.graph_mut(0).unwrap(),
            vec![
                dataset::xy(vec![
                    vec![0., 0.],
                    vec![1., 1.],
                    vec![2., 0.],
                    vec![3., -1.],
                ])
                .unwrap(),
            ],
        )
        .unwrap();
        execute(
            &mut project,
            "FFT(S0,REAL,NONE,INDEX,MAGNITUDE); DFT(S0,REAL,NONE,INDEX,MAGNITUDE)",
            0,
        )
        .unwrap();
        assert_eq!(project.graphs[&0].sets[&1].rows[1], vec![1., 4.]);
        assert_eq!(project.graphs[&0].sets[&2].rows[1], vec![1., 1.]);
    }
}
