//! Typed project-wide graph and page edits shared by browser dialogs and scripts.

use crate::{Diagnostic, Graph, Project};
use serde::Deserialize;

const VIEW_LEFT: f64 = 0.15;
const VIEW_BOTTOM: f64 = 0.15;
const VIEW_RIGHT: f64 = 1.15;
const VIEW_TOP: f64 = 0.85;

#[derive(Debug, Deserialize)]
#[serde(
    tag = "method",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum GraphOperation {
    Create,
    Delete {
        graph: u32,
    },
    Duplicate {
        graph: u32,
    },
    Visibility {
        graph: u32,
        visible: bool,
    },
    Viewport {
        graph: u32,
        bounds: [f64; 4],
    },
    Arrange {
        rows: usize,
        columns: usize,
        gap: f64,
    },
    Overlay {
        graph: u32,
        other: u32,
        shared_x: bool,
        shared_y: bool,
    },
    Page {
        width: f64,
        height: f64,
        background: u32,
        fill: bool,
    },
}

/// Applies an operation only after all of its preconditions are checked.
pub(crate) fn apply(project: &mut Project, operation: GraphOperation) -> Result<(), Diagnostic> {
    project.require_editable()?;
    match operation {
        GraphOperation::Create => {
            let id = next_id(project)?;
            project.graphs.insert(id, Graph::default());
        }
        GraphOperation::Delete { graph } => {
            require_graph(project, graph)?;
            if project.graphs.len() == 1 {
                return Err(Diagnostic::new("Keep at least one graph in the project."));
            }
            project.graphs.remove(&graph);
            project
                .objects
                .retain(|_, object| object.linked_graph() != Some(graph));
            project
                .regions
                .retain(|_, region| region.graph != Some(graph));
        }
        GraphOperation::Duplicate { graph } => {
            let source = require_graph(project, graph)?.clone();
            let id = next_id(project)?;
            project.graphs.insert(id, source);
        }
        GraphOperation::Visibility { graph, visible } => {
            require_graph_mut(project, graph)?.visible = visible;
        }
        GraphOperation::Viewport { graph, bounds } => {
            validate_view(bounds)?;
            require_graph_mut(project, graph)?.view = bounds;
        }
        GraphOperation::Arrange { rows, columns, gap } => arrange(project, rows, columns, gap)?,
        GraphOperation::Overlay {
            graph,
            other,
            shared_x,
            shared_y,
        } => overlay(project, graph, other, shared_x, shared_y)?,
        GraphOperation::Page {
            width,
            height,
            background,
            fill,
        } => {
            if !width.is_finite() || !height.is_finite() || width <= 0. || height <= 0. {
                return Err(Diagnostic::new(
                    "Page width and height must be positive finite values.",
                ));
            }
            if background > 15 && !project.colors.contains_key(&background) {
                return Err(Diagnostic::new("Select a defined background color."));
            }
            project.page = [width, height];
            project.background = background;
            project.fill_page = fill;
        }
    }
    Ok(())
}

fn require_graph(project: &Project, id: u32) -> Result<&Graph, Diagnostic> {
    project
        .graphs
        .get(&id)
        .ok_or_else(|| Diagnostic::new("Select an existing graph."))
}

fn require_graph_mut(project: &mut Project, id: u32) -> Result<&mut Graph, Diagnostic> {
    project
        .graphs
        .get_mut(&id)
        .ok_or_else(|| Diagnostic::new("Select an existing graph."))
}

fn next_id(project: &Project) -> Result<u32, Diagnostic> {
    (0..=u32::MAX)
        .find(|id| !project.graphs.contains_key(id))
        .ok_or_else(|| Diagnostic::new("This project has no available graph identifiers."))
}

fn validate_view([left, bottom, right, top]: [f64; 4]) -> Result<(), Diagnostic> {
    if ![left, bottom, right, top]
        .iter()
        .all(|value| value.is_finite())
        || left >= right
        || bottom >= top
    {
        return Err(Diagnostic::new(
            "Viewport limits must be finite and increase from left to right and bottom to top.",
        ));
    }
    Ok(())
}

fn arrange(project: &mut Project, rows: usize, columns: usize, gap: f64) -> Result<(), Diagnostic> {
    if rows == 0 || columns == 0 || rows.checked_mul(columns).is_none() {
        return Err(Diagnostic::new("Use a positive, practical graph grid."));
    }
    if !gap.is_finite() || gap < 0. {
        return Err(Diagnostic::new(
            "Graph spacing must be a non-negative finite value.",
        ));
    }
    let horizontal = VIEW_RIGHT - VIEW_LEFT;
    let vertical = VIEW_TOP - VIEW_BOTTOM;
    let width = (horizontal - gap * (columns.saturating_sub(1) as f64)) / columns as f64;
    let height = (vertical - gap * (rows.saturating_sub(1) as f64)) / rows as f64;
    if width <= 0. || height <= 0. || !width.is_finite() || !height.is_finite() {
        return Err(Diagnostic::new(
            "Graph spacing leaves no room for the requested grid.",
        ));
    }
    let count = rows * columns;
    while project.graphs.len() < count {
        let id = next_id(project)?;
        project.graphs.insert(id, Graph::default());
    }
    for (index, graph) in project.graphs.values_mut().take(count).enumerate() {
        let row = index / columns;
        let column = index % columns;
        let left = VIEW_LEFT + column as f64 * (width + gap);
        let bottom = VIEW_BOTTOM + (rows - row - 1) as f64 * (height + gap);
        graph.view = [left, bottom, left + width, bottom + height];
        graph.visible = true;
    }
    Ok(())
}

fn overlay(
    project: &mut Project,
    graph: u32,
    other: u32,
    shared_x: bool,
    shared_y: bool,
) -> Result<(), Diagnostic> {
    if graph == other {
        return Err(Diagnostic::new("Choose two different graphs to overlay."));
    }
    let source = require_graph(project, other)?;
    let (view, world) = (source.view, source.world);
    validate_view(view)?;
    let target = require_graph_mut(project, graph)?;
    target.view = view;
    if shared_x {
        target.world[0] = world[0];
        target.world[2] = world[2];
    }
    if shared_y {
        target.world[1] = world[1];
        target.world[3] = world[3];
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{GraphOperation, apply};
    use crate::Project;

    #[test]
    fn creates_and_duplicates_lowest_available_graph_ids() {
        let mut project = Project::new();
        apply(&mut project, GraphOperation::Create).expect("create G1");
        apply(&mut project, GraphOperation::Delete { graph: 0 }).expect("delete G0");
        apply(&mut project, GraphOperation::Duplicate { graph: 1 }).expect("reuse G0");
        assert!(project.graphs.contains_key(&0));
        assert!(project.graphs.contains_key(&1));
    }

    #[test]
    fn arrangement_creates_a_non_overlapping_grid() {
        let mut project = Project::new();
        apply(
            &mut project,
            GraphOperation::Arrange {
                rows: 2,
                columns: 2,
                gap: 0.05,
            },
        )
        .expect("arrange grid");
        assert_eq!(project.graphs.len(), 4);
        let first = project.graphs[&0].view;
        let second = project.graphs[&1].view;
        assert!(first[2] < second[0]);
    }

    #[test]
    fn invalid_arrangement_does_not_mutate_the_project() {
        let mut project = Project::new();
        let before = project.graphs[&0].view;
        let failure = apply(
            &mut project,
            GraphOperation::Arrange {
                rows: 1,
                columns: 2,
                gap: 2.,
            },
        );
        assert!(failure.is_err());
        assert_eq!(project.graphs.len(), 1);
        assert_eq!(project.graphs[&0].view, before);
    }

    #[test]
    fn overlay_can_share_one_axis_without_changing_the_other() {
        let mut project = Project::new();
        apply(&mut project, GraphOperation::Create).expect("create G1");
        project.graphs.get_mut(&1).expect("G1").world = [5., 10., 15., 20.];
        project.graphs.get_mut(&1).expect("G1").view = [0.3, 0.3, 0.9, 0.7];
        apply(
            &mut project,
            GraphOperation::Overlay {
                graph: 0,
                other: 1,
                shared_x: true,
                shared_y: false,
            },
        )
        .expect("overlay");
        assert_eq!(project.graphs[&0].view, [0.3, 0.3, 0.9, 0.7]);
        assert_eq!(project.graphs[&0].world, [5., 0., 15., 1.]);
    }
}
