//! Typed operations on Grace data sets.
//!
//! The project model still stores the historical AGR type name as a string. This
//! module is the single validation boundary for new and edited data, so future
//! set kinds can be added without spreading row-width checks across the worker.

use crate::{Diagnostic, Graph, Project, Set, agr};
use serde::Deserialize;

/// Classic Grace set kinds, stored with their canonical AGR names.
///
/// Rendering support is deliberately a separate graph capability decision.  This
/// type owns the durable tabular shape of each data format so import, editing,
/// export, and later renderers agree on the same contract.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SetKind {
    Xy,
    Bar,
    BarDy,
    BarDyDy,
    Xydx,
    Xydy,
    Xydxdx,
    Xydydy,
    Xydxdy,
    Xydxdxdydy,
    XyHilo,
    Xyr,
    XySize,
    XyColor,
    XyColPat,
    XyVMap,
    XyBoxPlot,
    Xyz,
}

impl SetKind {
    pub(crate) fn width(self) -> usize {
        match self {
            Self::Xy | Self::Bar => 2,
            Self::BarDy
            | Self::Xydx
            | Self::Xydy
            | Self::Xyr
            | Self::XySize
            | Self::XyColor
            | Self::Xyz => 3,
            Self::BarDyDy
            | Self::Xydxdx
            | Self::Xydydy
            | Self::Xydxdy
            | Self::XyColPat
            | Self::XyVMap => 4,
            Self::XyHilo => 5,
            Self::Xydxdxdydy | Self::XyBoxPlot => 6,
        }
    }

    pub(crate) fn as_agr(self) -> &'static str {
        match self {
            Self::Xy => "xy",
            Self::Bar => "bar",
            Self::BarDy => "bardy",
            Self::BarDyDy => "bardydy",
            Self::Xydx => "xydx",
            Self::Xydy => "xydy",
            Self::Xydxdx => "xydxdx",
            Self::Xydydy => "xydydy",
            Self::Xydxdy => "xydxdy",
            Self::Xydxdxdydy => "xydxdxdydy",
            Self::XyHilo => "xyhilo",
            Self::Xyr => "xyr",
            Self::XySize => "xysize",
            Self::XyColor => "xycolor",
            Self::XyColPat => "xycolpat",
            Self::XyVMap => "xyvmap",
            Self::XyBoxPlot => "xyboxplot",
            Self::Xyz => "xyz",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, Diagnostic> {
        match value.to_ascii_lowercase().as_str() {
            "xy" => Ok(Self::Xy),
            "bar" => Ok(Self::Bar),
            "bardy" => Ok(Self::BarDy),
            "bardydy" => Ok(Self::BarDyDy),
            "xydx" => Ok(Self::Xydx),
            "xydy" => Ok(Self::Xydy),
            "xydxdx" => Ok(Self::Xydxdx),
            "xydydy" => Ok(Self::Xydydy),
            "xydxdy" => Ok(Self::Xydxdy),
            "xydxdxdydy" => Ok(Self::Xydxdxdydy),
            "xyhilo" => Ok(Self::XyHilo),
            "xyr" => Ok(Self::Xyr),
            "xysize" => Ok(Self::XySize),
            "xycolor" => Ok(Self::XyColor),
            "xycolpat" => Ok(Self::XyColPat),
            "xyvmap" => Ok(Self::XyVMap),
            "xyboxplot" => Ok(Self::XyBoxPlot),
            "xyz" => Ok(Self::Xyz),
            _ => Err(Diagnostic::new("Select a supported data set type.")),
        }
    }
}

/// Operations exposed by the browser data-set manager.
#[derive(Debug, Deserialize)]
#[serde(
    tag = "method",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum SetOperation {
    Sort {
        set: u32,
        axis: String,
        descending: bool,
    },
    Reverse {
        set: u32,
    },
    Join {
        set: u32,
        other: u32,
    },
    Split {
        set: u32,
        length: usize,
    },
    Drop {
        set: u32,
        first: usize,
        last: usize,
    },
    Delete {
        set: u32,
    },
    Duplicate {
        set: u32,
    },
    Clear {
        set: u32,
    },
    Copy {
        set: u32,
        target_graph: u32,
        target_set: Option<u32>,
    },
    Move {
        set: u32,
        target_graph: u32,
        target_set: Option<u32>,
    },
    Swap {
        set: u32,
        target_graph: u32,
        target_set: Option<u32>,
    },
}

fn validate_rows(kind: SetKind, rows: &[Vec<f64>], allow_empty: bool) -> Result<(), Diagnostic> {
    if rows.is_empty() && !allow_empty {
        return Err(Diagnostic::new("A new data set needs at least one row."));
    }
    for (index, row) in rows.iter().enumerate() {
        if row.len() != kind.width() || !row.iter().all(|value| value.is_finite()) {
            return Err(Diagnostic::new(format!(
                "Row {} must contain {} finite values for {} data.",
                index + 1,
                kind.width(),
                kind.as_agr().to_ascii_uppercase()
            )));
        }
    }
    Ok(())
}

fn validate_set(set: &Set, allow_empty: bool) -> Result<SetKind, Diagnostic> {
    let kind = SetKind::parse(&set.kind)?;
    validate_rows(kind, &set.rows, allow_empty)?;
    Ok(kind)
}

fn validate_graph_set(graph: &Graph, set: &Set, allow_empty: bool) -> Result<SetKind, Diagnostic> {
    let kind = validate_set(set, allow_empty)?;
    if !agr::set_kind_supported_for_graph(&graph.kind, kind.as_agr()) {
        return Err(Diagnostic::new(format!(
            "{} data cannot be added to a {} graph.",
            kind.as_agr().to_ascii_uppercase(),
            graph.kind
        )));
    }
    Ok(kind)
}

fn set_id(graph: &Graph) -> Result<u32, Diagnostic> {
    let mut id = 0_u32;
    while graph.sets.contains_key(&id) {
        id = id
            .checked_add(1)
            .ok_or_else(|| Diagnostic::new("No available data set identifier."))?;
    }
    Ok(id)
}

fn set_ids(graph: &Graph, count: usize) -> Result<Vec<u32>, Diagnostic> {
    let mut ids = Vec::with_capacity(count);
    let mut candidate = 0_u32;
    while ids.len() < count {
        if !graph.sets.contains_key(&candidate) {
            ids.push(candidate);
        }
        candidate = candidate
            .checked_add(1)
            .ok_or_else(|| Diagnostic::new("No available data set identifier."))?;
    }
    Ok(ids)
}

fn clone_set(set: &Set) -> Set {
    Set {
        settings: set.settings.clone(),
        rows: set.rows.clone(),
        count: set.count,
        kind: set.kind.clone(),
    }
}

fn selected(graph: &Graph, set: u32) -> Result<&Set, Diagnostic> {
    graph
        .sets
        .get(&set)
        .ok_or_else(|| Diagnostic::new("Select an existing data set."))
}

fn update_count(set: &mut Set) {
    set.count = set.rows.len();
}

/// Appends fully validated data sets and returns their lowest-free identifiers.
pub(crate) fn append(graph: &mut Graph, sets: Vec<Set>) -> Result<Vec<u32>, Diagnostic> {
    for set in &sets {
        validate_graph_set(graph, set, false)?;
    }
    let ids = set_ids(graph, sets.len())?;
    for (id, mut set) in ids.iter().copied().zip(sets) {
        update_count(&mut set);
        graph.sets.insert(id, set);
    }
    Ok(ids)
}

/// Creates one validated XY set.
pub(crate) fn xy(rows: Vec<Vec<f64>>) -> Result<Set, Diagnostic> {
    validate_rows(SetKind::Xy, &rows, false)?;
    Ok(Set {
        count: rows.len(),
        rows,
        kind: SetKind::Xy.as_agr().into(),
        ..Set::default()
    })
}

/// Replaces an existing set or creates a new one.
///
/// Empty rows are valid only while clearing an existing data set.
pub(crate) fn replace(
    graph: &mut Graph,
    set: Option<u32>,
    kind: SetKind,
    rows: Vec<Vec<f64>>,
) -> Result<u32, Diagnostic> {
    let existing = set.is_some();
    validate_rows(kind, &rows, existing)?;
    if !agr::set_kind_supported_for_graph(&graph.kind, kind.as_agr()) {
        return Err(Diagnostic::new(format!(
            "{} data cannot be added to a {} graph.",
            kind.as_agr().to_ascii_uppercase(),
            graph.kind
        )));
    }
    let id = match set {
        Some(id) => {
            selected(graph, id)?;
            id
        }
        None => set_id(graph)?,
    };
    let settings = graph
        .sets
        .get(&id)
        .map(|old| old.settings.clone())
        .unwrap_or_default();
    let mut replacement = Set {
        settings,
        count: rows.len(),
        rows,
        kind: kind.as_agr().into(),
    };
    update_count(&mut replacement);
    graph.sets.insert(id, replacement);
    Ok(id)
}

fn graph_mut(project: &mut Project, graph: u32) -> Result<&mut Graph, Diagnostic> {
    project
        .graphs
        .get_mut(&graph)
        .ok_or_else(|| Diagnostic::new("Select an existing graph."))
}

fn destination_id(graph: &Graph, requested: Option<u32>) -> Result<u32, Diagnostic> {
    match requested {
        Some(id) if graph.sets.contains_key(&id) => Err(Diagnostic::new(
            "Choose an unused destination data set identifier.",
        )),
        Some(id) => Ok(id),
        None => set_id(graph),
    }
}

/// Applies one data-set operation after validating all selected identifiers.
pub(crate) fn operate(
    project: &mut Project,
    graph: u32,
    operation: SetOperation,
) -> Result<(), Diagnostic> {
    project.require_editable()?;
    match operation {
        SetOperation::Sort {
            set,
            axis,
            descending,
        } => {
            let axis = match axis.as_str() {
                "x" => 0,
                "y" => 1,
                "y1" => 2,
                "y2" => 3,
                "y3" => 4,
                "y4" => 5,
                _ => return Err(Diagnostic::new("Sort by X, Y, or Y1 through Y4.")),
            };
            let selected = selected(graph_mut(project, graph)?, set)?;
            let kind = validate_set(selected, true)?;
            if axis >= kind.width() {
                return Err(Diagnostic::new(format!(
                    "{} data has no {} column.",
                    kind.as_agr().to_ascii_uppercase(),
                    axis_name(axis)
                )));
            }
            let mut rows = selected.rows.clone();
            rows.sort_by(|left, right| left[axis].total_cmp(&right[axis]));
            if descending {
                rows.reverse();
            }
            graph_mut(project, graph)?
                .sets
                .get_mut(&set)
                .expect("validated set")
                .rows = rows;
        }
        SetOperation::Reverse { set } => {
            let selected = graph_mut(project, graph)?.sets.get_mut(&set);
            let Some(selected) = selected else {
                return Err(Diagnostic::new("Select an existing data set."));
            };
            selected.rows.reverse();
        }
        SetOperation::Join { set, other } => {
            if set == other {
                return Err(Diagnostic::new("Choose two different data sets to join."));
            }
            let graph = graph_mut(project, graph)?;
            let first = clone_set(selected(graph, set)?);
            let second = clone_set(selected(graph, other)?);
            if first.kind != second.kind {
                return Err(Diagnostic::new("Joined data sets must have the same type."));
            }
            let mut joined = first.rows;
            joined.extend(second.rows);
            let target = graph.sets.get_mut(&set).expect("validated set");
            target.rows = joined;
            update_count(target);
            graph.sets.remove(&other);
        }
        SetOperation::Split { set, length } => {
            if length == 0 {
                return Err(Diagnostic::new("Split length must be at least one point."));
            }
            let graph = graph_mut(project, graph)?;
            let source = clone_set(selected(graph, set)?);
            if source.rows.is_empty() {
                return Err(Diagnostic::new("Cannot split an empty data set."));
            }
            let chunks: Vec<_> = source
                .rows
                .chunks(length)
                .map(|rows| rows.to_vec())
                .collect();
            let mut replacements = Vec::new();
            for rows in chunks.iter().skip(1) {
                let mut part = clone_set(&source);
                part.rows = rows.clone();
                update_count(&mut part);
                replacements.push(part);
            }
            let first = graph.sets.get_mut(&set).expect("validated set");
            first.rows = chunks[0].clone();
            update_count(first);
            append(graph, replacements)?;
        }
        SetOperation::Drop { set, first, last } => {
            let graph = graph_mut(project, graph)?;
            let count = selected(graph, set)?.rows.len();
            if first > last || last >= count {
                return Err(Diagnostic::new(
                    "Select an inclusive point range within the data set.",
                ));
            }
            let selected = graph.sets.get_mut(&set).expect("validated set");
            selected.rows.drain(first..=last);
            update_count(selected);
        }
        SetOperation::Delete { set } => {
            let graph = graph_mut(project, graph)?;
            selected(graph, set)?;
            graph.sets.remove(&set);
        }
        SetOperation::Duplicate { set } => {
            let graph = graph_mut(project, graph)?;
            let copy = clone_set(selected(graph, set)?);
            append(graph, vec![copy])?;
        }
        SetOperation::Clear { set } => {
            let selected = graph_mut(project, graph)?.sets.get_mut(&set);
            let Some(selected) = selected else {
                return Err(Diagnostic::new("Select an existing data set."));
            };
            selected.rows.clear();
            update_count(selected);
        }
        SetOperation::Copy {
            set,
            target_graph,
            target_set,
        } => {
            let copy = {
                let source = graph_mut(project, graph)?;
                clone_set(selected(source, set)?)
            };
            let target = graph_mut(project, target_graph)?;
            validate_graph_set(target, &copy, false)?;
            let id = destination_id(target, target_set)?;
            target.sets.insert(id, copy);
        }
        SetOperation::Move {
            set,
            target_graph,
            target_set,
        } => {
            if graph == target_graph && target_set == Some(set) {
                return Ok(());
            }
            let destination = {
                let target = project
                    .graphs
                    .get(&target_graph)
                    .ok_or_else(|| Diagnostic::new("Select an existing graph."))?;
                destination_id(target, target_set)?
            };
            let moved = {
                let source = graph_mut(project, graph)?;
                clone_set(selected(source, set)?)
            };
            let target = graph_mut(project, target_graph)?;
            validate_graph_set(target, &moved, false)?;
            target.sets.insert(destination, moved);
            graph_mut(project, graph)?.sets.remove(&set);
        }
        SetOperation::Swap {
            set,
            target_graph,
            target_set,
        } => {
            let Some(target_set) = target_set else {
                return Err(Diagnostic::new("Select the destination data set to swap."));
            };
            if graph == target_graph && set == target_set {
                return Ok(());
            }
            let left = {
                let source = graph_mut(project, graph)?;
                clone_set(selected(source, set)?)
            };
            let right = {
                let target = graph_mut(project, target_graph)?;
                clone_set(selected(target, target_set)?)
            };
            let source_graph = project
                .graphs
                .get(&graph)
                .ok_or_else(|| Diagnostic::new("Select an existing graph."))?;
            let destination_graph = project
                .graphs
                .get(&target_graph)
                .ok_or_else(|| Diagnostic::new("Select an existing graph."))?;
            validate_graph_set(source_graph, &right, false)?;
            validate_graph_set(destination_graph, &left, false)?;
            graph_mut(project, graph)?.sets.insert(set, right);
            graph_mut(project, target_graph)?
                .sets
                .insert(target_set, left);
        }
    }
    Ok(())
}

fn axis_name(axis: usize) -> &'static str {
    ["X", "Y", "Y1", "Y2", "Y3", "Y4"][axis]
}

/// Exports one selected set as portable whitespace-delimited ASCII data.
pub(crate) fn export_ascii(graph: &Graph, set: u32) -> Result<Vec<u8>, Diagnostic> {
    let set = selected(graph, set)?;
    validate_set(set, true)?;
    let mut output = String::new();
    for row in &set.rows {
        for (index, value) in row.iter().enumerate() {
            if index > 0 {
                output.push(' ');
            }
            output.push_str(&value.to_string());
        }
        output.push('\n');
    }
    Ok(output.into_bytes())
}

/// Returns a bounded, zero-copy slice of a set for browser spreadsheet pages.
pub(crate) fn table(
    graph: &Graph,
    set: u32,
    offset: usize,
    limit: usize,
) -> Result<&[Vec<f64>], Diagnostic> {
    let set = selected(graph, set)?;
    if offset > set.rows.len() {
        return Err(Diagnostic::new("Table offset is outside the data set."));
    }
    let end = offset.saturating_add(limit).min(set.rows.len());
    Ok(&set.rows[offset..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> Graph {
        Graph::default()
    }

    #[test]
    fn append_uses_lowest_available_identifiers() {
        let mut graph = graph();
        graph
            .sets
            .insert(0, xy(vec![vec![0., 0.]]).expect("valid set"));
        graph
            .sets
            .insert(2, xy(vec![vec![2., 2.]]).expect("valid set"));
        assert_eq!(
            append(
                &mut graph,
                vec![
                    xy(vec![vec![1., 1.]]).expect("valid set"),
                    xy(vec![vec![3., 3.]]).expect("valid set"),
                ],
            )
            .expect("append"),
            vec![1, 3]
        );
    }

    #[test]
    fn invalid_batch_does_not_append_any_sets() {
        let mut graph = graph();
        let invalid = Set {
            rows: vec![vec![1.]],
            kind: "xy".into(),
            ..Set::default()
        };
        assert!(
            append(
                &mut graph,
                vec![xy(vec![vec![0., 0.]]).expect("valid"), invalid]
            )
            .is_err()
        );
        assert!(graph.sets.is_empty());
    }

    #[test]
    fn export_preserves_full_precision_rows() {
        let mut graph = graph();
        append(&mut graph, vec![xy(vec![vec![0.1, -2.5]]).expect("valid")]).expect("append");
        assert_eq!(export_ascii(&graph, 0).expect("export"), b"0.1 -2.5\n");
    }

    #[test]
    fn joins_and_splits_without_losing_set_style() {
        let mut project = Project::new();
        let graph = project.graphs.get_mut(&0).expect("graph zero");
        let mut first = xy(vec![vec![2., 2.], vec![1., 1.]]).expect("valid");
        first.settings.insert("line color".into(), "4".into());
        append(
            graph,
            vec![first, xy(vec![vec![3., 3.], vec![4., 4.]]).expect("valid")],
        )
        .expect("append");
        operate(&mut project, 0, SetOperation::Join { set: 0, other: 1 }).expect("join");
        operate(&mut project, 0, SetOperation::Split { set: 0, length: 2 }).expect("split");
        let graph = &project.graphs[&0];
        assert_eq!(graph.sets[&0].rows, [[2., 2.], [1., 1.]]);
        assert_eq!(graph.sets[&1].rows, [[3., 3.], [4., 4.]]);
        assert_eq!(graph.sets[&1].settings["line color"], "4");
    }

    #[test]
    fn invalid_drop_leaves_the_data_unchanged() {
        let mut project = Project::new();
        append(
            project.graphs.get_mut(&0).expect("graph zero"),
            vec![xy(vec![vec![0., 0.], vec![1., 1.]]).expect("valid")],
        )
        .expect("append");
        assert!(
            operate(
                &mut project,
                0,
                SetOperation::Drop {
                    set: 0,
                    first: 1,
                    last: 2,
                },
            )
            .is_err()
        );
        assert_eq!(project.graphs[&0].sets[&0].rows, [[0., 0.], [1., 1.]]);
    }

    #[test]
    fn bar_sets_have_the_two_column_chart_shape() {
        let mut graph = graph();
        graph.kind = "Chart".into();
        replace(
            &mut graph,
            None,
            SetKind::Bar,
            vec![vec![0., 2.], vec![1., 3.]],
        )
        .expect("valid bar set");
        assert_eq!(graph.sets[&0].kind, "bar");
        assert!(replace(&mut graph, None, SetKind::Bar, vec![vec![0., 2., 3.]]).is_err());
    }

    #[test]
    fn bar_data_is_rejected_by_an_xy_graph() {
        let mut graph = graph();
        let bar = Set {
            rows: vec![vec![0., 2.]],
            count: 1,
            kind: SetKind::Bar.as_agr().into(),
            ..Set::default()
        };
        assert!(append(&mut graph, vec![bar]).is_err());
        assert!(graph.sets.is_empty());
    }

    #[test]
    fn classic_set_kinds_have_their_agr_names_and_column_counts() {
        let cases = [
            (SetKind::BarDy, "bardy", 3),
            (SetKind::BarDyDy, "bardydy", 4),
            (SetKind::XyHilo, "xyhilo", 5),
            (SetKind::Xyr, "xyr", 3),
            (SetKind::XySize, "xysize", 3),
            (SetKind::XyColor, "xycolor", 3),
            (SetKind::XyColPat, "xycolpat", 4),
            (SetKind::XyVMap, "xyvmap", 4),
            (SetKind::XyBoxPlot, "xyboxplot", 6),
            (SetKind::Xyz, "xyz", 3),
        ];
        for (kind, agr, width) in cases {
            assert_eq!(kind.as_agr(), agr);
            assert_eq!(SetKind::parse(agr).expect("parse"), kind);
            assert_eq!(kind.width(), width);
        }
    }

    #[test]
    fn sort_accepts_classic_extra_columns_only_when_present() {
        let mut project = Project::new();
        replace(
            project.graphs.get_mut(&0).expect("graph zero"),
            None,
            SetKind::Xydxdxdydy,
            vec![vec![0., 0., 0., 0., 0., 4.], vec![1., 0., 0., 0., 0., 2.]],
        )
        .expect("six column set");
        operate(
            &mut project,
            0,
            SetOperation::Sort {
                set: 0,
                axis: "y4".into(),
                descending: false,
            },
        )
        .expect("sort y4");
        assert_eq!(project.graphs[&0].sets[&0].rows[0][0], 1.);
        assert!(
            operate(
                &mut project,
                0,
                SetOperation::Sort {
                    set: 0,
                    axis: "y4".into(),
                    descending: false,
                },
            )
            .is_ok()
        );

        let xy = project.graphs[&0].sets.get(&0).expect("source").clone();
        project.graphs.get_mut(&0).expect("graph zero").sets.insert(
            1,
            Set {
                rows: vec![xy.rows[0][..2].to_vec()],
                count: 1,
                kind: "xy".into(),
                ..Set::default()
            },
        );
        assert!(
            operate(
                &mut project,
                0,
                SetOperation::Sort {
                    set: 1,
                    axis: "y1".into(),
                    descending: false,
                },
            )
            .is_err()
        );
    }
}
