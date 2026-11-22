use crate::advanced::{self, AdvancedOperation};
use crate::dataset::{self, SetKind, SetOperation};
use crate::expression::Expression;
use crate::fitting::{self, Feature, FitOptions};
use crate::fourier::{self, FourierOptions};
use crate::regions::Region;
use crate::series::{self, SeriesOperation};
use crate::{Diagnostic, Graph, Project, Set, autoscale};
use serde::Deserialize;

const MAX_GENERATED_POINTS: usize = 100_000;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum DataCommand {
    Generate {
        graph: u32,
        start: String,
        stop: String,
        points: usize,
        x: String,
        y: String,
    },
    Evaluate {
        graph: u32,
        set: u32,
        x: String,
        y: String,
        region: Option<u32>,
        #[serde(default)]
        negate: bool,
        #[serde(default)]
        replace: bool,
    },
    Fourier {
        graph: u32,
        set: u32,
        options: FourierOptions,
    },
    Series {
        graph: u32,
        set: u32,
        operation: SeriesOperation,
    },
    Advanced {
        graph: u32,
        set: u32,
        other: Option<u32>,
        operation: AdvancedOperation,
    },
    Fit {
        graph: u32,
        set: u32,
        options: FitOptions,
    },
    FeatureExtraction {
        graph: u32,
        feature: Feature,
    },
    SetOperation {
        graph: u32,
        operation: SetOperation,
    },
    ReplaceData {
        graph: u32,
        set: Option<u32>,
        kind: SetKind,
        rows: Vec<Vec<f64>>,
    },
}

impl DataCommand {
    pub(crate) fn apply(self, project: &mut Project) -> Result<(), Diagnostic> {
        match self {
            Self::Generate {
                graph,
                start,
                stop,
                points,
                x,
                y,
            } => {
                let graph = project.graph_mut(graph)?;
                let rows = generate(&start, &stop, points, &x, &y)?;
                append_xy(graph, rows, "Formula-generated data")?;
            }
            Self::Evaluate {
                graph,
                set,
                x,
                y,
                region,
                negate,
                replace,
            } => {
                project.require_editable()?;
                let restriction = region
                    .map(|id| {
                        project
                            .regions
                            .get(&id)
                            .filter(|region| {
                                region.active && region.graph.is_none_or(|linked| linked == graph)
                            })
                            .ok_or_else(|| {
                                Diagnostic::new("Select an active region for this graph.")
                            })
                    })
                    .transpose()?;
                let input = project
                    .graphs
                    .get(&graph)
                    .ok_or_else(|| Diagnostic::new("Select an existing graph."))?;
                let rows = evaluate(
                    input,
                    set,
                    &x,
                    &y,
                    restriction.map(|region| (region, graph, negate)),
                )?;
                let graph = project.graph_mut(graph)?;
                if replace {
                    let kind = SetKind::parse(&graph.sets[&set].kind)?;
                    dataset::replace(graph, Some(set), kind, rows)?;
                    autoscale(graph, "xy");
                } else {
                    let mut result = graph.sets[&set].clone();
                    result.count = rows.len();
                    result.rows = rows;
                    append_set(graph, result, &format!("Formula applied to S{set}"))?;
                }
            }
            Self::Fourier {
                graph,
                set,
                options,
            } => {
                let graph = project.graph_mut(graph)?;
                let rows = fourier::transform(source(graph, set)?, options)?;
                append_xy(graph, rows, &format!("Fourier transform of S{set}"))?;
            }
            Self::Series {
                graph,
                set,
                operation,
            } => {
                let graph = project.graph_mut(graph)?;
                let rows = series::transform(source(graph, set)?, operation)?;
                append_xy(graph, rows, &format!("Transformation of S{set}"))?;
            }
            Self::Advanced {
                graph,
                set,
                other,
                operation,
            } => {
                let graph = project.graph_mut(graph)?;
                let preserve_kind = matches!(
                    &operation,
                    AdvancedOperation::Geometric { .. }
                        | AdvancedOperation::Sampling { .. }
                        | AdvancedOperation::Pruning { .. }
                );
                let secondary = other.map(|id| source(graph, id)).transpose()?;
                let rows = advanced::transform(source(graph, set)?, secondary, operation)?;
                let result = if preserve_kind {
                    let mut result = graph.sets.get(&set).expect("validated set").clone();
                    result.count = rows.len();
                    result.rows = rows;
                    result
                } else {
                    dataset::xy(rows)?
                };
                append_set(graph, result, &format!("Transformation of S{set}"))?;
            }
            Self::Fit {
                graph,
                set,
                options,
            } => {
                let graph = project.graph_mut(graph)?;
                let result = fitting::fit(source(graph, set)?, options)?;
                append_xy(graph, result.rows, &format!("Nonlinear fit of S{set}"))?;
                let parameters = result
                    .parameters
                    .iter()
                    .enumerate()
                    .map(|(index, value)| format!("a{index} = {value:.8e}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                if project.console.len() >= 100 {
                    project.console.remove(0);
                }
                project.console.push(format!("Nonlinear fit of S{set}. {parameters}. Residual sum of squares = {:.8e}. {} iterations.",
                    result.sum_squared_residuals, result.iterations));
            }
            Self::FeatureExtraction { graph, feature } => {
                let graph = project.graph_mut(graph)?;
                let rows = fitting::extract(graph, feature)?;
                append_xy(graph, rows, "Feature extraction")?;
            }
            Self::SetOperation { graph, operation } => dataset::operate(project, graph, operation)?,
            Self::ReplaceData {
                graph,
                set,
                kind,
                rows,
            } => {
                let graph = project.graph_mut(graph)?;
                dataset::replace(graph, set, kind, rows)?;
                autoscale(graph, "xy");
            }
        }
        Ok(())
    }
}

pub(crate) fn source(graph: &Graph, set: u32) -> Result<&[Vec<f64>], Diagnostic> {
    let set = graph
        .sets
        .get(&set)
        .ok_or_else(|| Diagnostic::new("Select an existing set."))?;
    if set.rows.is_empty() {
        return Err(Diagnostic::new("The selected set contains no points."));
    }
    Ok(&set.rows)
}

pub(crate) fn append_xy(
    graph: &mut Graph,
    rows: Vec<Vec<f64>>,
    comment: &str,
) -> Result<(), Diagnostic> {
    append_set(graph, dataset::xy(rows)?, comment)
}

fn append_set(graph: &mut Graph, mut set: Set, comment: &str) -> Result<(), Diagnostic> {
    set.settings.insert("comment".into(), crate::quote(comment));
    let mut ids = dataset::append(graph, vec![set])?.into_iter();
    if let Some(id) = ids.next() {
        // Keep derived sets easy to distinguish on the same graph.
        let color = (id % 14 + 1).to_string();
        let set = graph.sets.get_mut(&id).expect("the new set is present");
        set.settings.insert("line color".into(), color.clone());
        set.settings.insert("symbol color".into(), color);
    }
    autoscale(graph, "xy");
    Ok(())
}

fn expression(source: &str, field: &str, variables: &[&str]) -> Result<Expression, Diagnostic> {
    let body = source.trim();
    let body = match body.split_once('=') {
        Some((name, value))
            if name.trim().eq_ignore_ascii_case(field) && !value.starts_with('=') =>
        {
            value
        }
        _ => body,
    };
    Expression::parse(body, variables)
        .map_err(|error| Diagnostic::new(format!("{field}: {}", error.message)))
}

fn scalar(source: &str, field: &str) -> Result<f64, Diagnostic> {
    expression(source, field, &[])?
        .evaluate(&[])
        .map_err(|error| Diagnostic::new(format!("{field}: {}", error.message)))
}

fn at_point(
    expr: &Expression,
    values: &[f64],
    field: &str,
    index: usize,
) -> Result<f64, Diagnostic> {
    expr.evaluate(values).map_err(|error| {
        Diagnostic::new(format!("{field}, point {}: {}", index + 1, error.message))
    })
}

fn generate(
    start: &str,
    stop: &str,
    points: usize,
    x: &str,
    y: &str,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    if !(2..=MAX_GENERATED_POINTS).contains(&points) {
        return Err(Diagnostic::new(format!(
            "Enter between 2 and {MAX_GENERATED_POINTS} points."
        )));
    }
    let start = scalar(start, "Start")?;
    let stop = scalar(stop, "Stop")?;
    let x = expression(x, "X", &["$t", "t"])?;
    let y = expression(y, "Y", &["$t", "t"])?;
    (0..points)
        .map(|index| {
            let fraction = index as f64 / (points - 1) as f64;
            let t = (1. - fraction) * start + fraction * stop;
            let values = [t, t];
            Ok(vec![
                at_point(&x, &values, "X", index)?,
                at_point(&y, &values, "Y", index)?,
            ])
        })
        .collect()
}

fn evaluate(
    graph: &Graph,
    set: u32,
    x: &str,
    y: &str,
    restriction: Option<(&Region, u32, bool)>,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    const COLUMNS: [&str; 6] = ["x", "y", "y1", "y2", "y3", "y4"];
    let rows = source(graph, set)?;
    let combined = format!("{x} {y}").to_ascii_lowercase();
    let mut references = std::collections::BTreeMap::new();
    for token in
        combined.split(|character: char| !(character.is_ascii_alphanumeric() || character == '.'))
    {
        let Some((id, name)) = token
            .strip_prefix('s')
            .and_then(|value| value.split_once('.'))
        else {
            continue;
        };
        let (Ok(id), Some(column)) = (
            id.parse::<u32>(),
            COLUMNS.iter().position(|&value| value == name),
        ) else {
            continue;
        };
        let other = source(graph, id)?;
        if other.len() != rows.len() {
            return Err(Diagnostic::new(format!(
                "S{set} and S{id} must have the same number of points."
            )));
        }
        references.insert(token, (other, column));
    }
    let variables = COLUMNS
        .into_iter()
        .chain(["i"])
        .chain(references.keys().copied())
        .collect::<Vec<_>>();
    let x = expression(if x.trim().is_empty() { "x" } else { x }, "X", &variables)?;
    let y = expression(if y.trim().is_empty() { "y" } else { y }, "Y", &variables)?;
    let mut values = vec![f64::NAN; variables.len()];
    rows.iter()
        .enumerate()
        .filter(|(_, row)| {
            restriction.is_none_or(|(region, graph, negate)| {
                region.contains(graph, row[0], row[1]) != negate
            })
        })
        .map(|(index, row)| {
            let width = row.len().min(6);
            values[..width].copy_from_slice(&row[..width]);
            values[6] = index as f64;
            for ((set, column), variable) in references.values().zip(&mut values[7..]) {
                *variable = set[index].get(*column).copied().unwrap_or(f64::NAN);
            }
            let mut result = row.clone();
            result[0] = at_point(&x, &values, "X", index)?;
            result[1] = at_point(&y, &values, "Y", index)?;
            Ok(result)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_inclusive_mesh_and_preserves_failed_project() {
        let mut project = Project::new();
        let command = |y: &str| DataCommand::Generate {
            graph: 0,
            start: "0".into(),
            stop: "2*pi".into(),
            points: 5,
            x: "X = $t".into(),
            y: y.into(),
        };
        command("sin($t)").apply(&mut project).unwrap();
        let points = &project.graphs[&0].sets[&0].rows;
        assert_eq!(points.len(), 5);
        assert_eq!(points[0], [0., 0.]);
        assert_eq!(points[1][1], 1.);
        assert_eq!(points[4][0], std::f64::consts::TAU);
        assert!(command("1 / $t").apply(&mut project).is_err());
        assert_eq!(project.graphs[&0].sets.len(), 1);
    }
}
