//! Renderer-backed graph, set, and axis appearance operations.
//!
//! These typed updates keep the browser controls on the same settings keys as
//! AGR import and SVG rendering. Every operation validates a complete proposed
//! value before changing the project.

use crate::{Diagnostic, Project, agr};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(
    tag = "method",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum AppearanceOperation {
    Graph {
        graph: u32,
        kind: Option<String>,
        legend: Option<bool>,
        frame: Option<bool>,
        frame_width: Option<f64>,
        title_size: Option<f64>,
        subtitle_size: Option<f64>,
        stacked: Option<bool>,
        bar_gap: Option<f64>,
    },
    Set {
        graph: u32,
        set: u32,
        line_width: Option<f64>,
        line_style: Option<u8>,
        symbol_size: Option<f64>,
        symbol_skip: Option<usize>,
        error_bars: Option<bool>,
        error_size: Option<f64>,
        fill_type: Option<u8>,
        fill_color: Option<u32>,
        baseline_type: Option<u8>,
        baseline: Option<bool>,
        drop_lines: Option<bool>,
    },
    Axis {
        graph: u32,
        axis: String,
        visible: Option<bool>,
        invert: Option<bool>,
        grid: Option<bool>,
        minor_grid: Option<bool>,
        minor_ticks: Option<usize>,
        precision: Option<usize>,
        format: Option<String>,
        placement: Option<String>,
        direction: Option<String>,
        stagger: Option<usize>,
        label: Option<String>,
    },
}

pub(crate) fn apply(
    project: &mut Project,
    operation: AppearanceOperation,
) -> Result<(), Diagnostic> {
    project.require_editable()?;
    match operation {
        AppearanceOperation::Graph {
            graph,
            kind,
            legend,
            frame,
            frame_width,
            title_size,
            subtitle_size,
            stacked,
            bar_gap,
        } => {
            let graph_ref = graph_mut(project, graph)?;
            let mut candidate = graph_ref.clone();
            if let Some(kind) = kind {
                let kind = graph_kind(&kind)?;
                for set in candidate.sets.values() {
                    if !agr::set_kind_supported_for_graph(kind, &set.kind) {
                        return Err(Diagnostic::new(format!(
                            "{} data cannot be displayed by a {} graph.",
                            set.kind.to_ascii_uppercase(),
                            kind
                        )));
                    }
                }
                candidate.kind = kind.into();
            }
            optional_bool(&mut candidate.settings, "legend", legend);
            if let Some(frame) = frame {
                candidate
                    .settings
                    .insert("frame pattern".into(), u8::from(frame).to_string());
            }
            optional_number(
                &mut candidate.settings,
                "frame linewidth",
                frame_width,
                positive,
            )?;
            optional_number(&mut candidate.settings, "title size", title_size, positive)?;
            optional_number(
                &mut candidate.settings,
                "subtitle size",
                subtitle_size,
                positive,
            )?;
            optional_bool(&mut candidate.settings, "stacked", stacked);
            optional_number(&mut candidate.settings, "bar hgap", bar_gap, bar_gap_valid)?;
            *graph_ref = candidate;
        }
        AppearanceOperation::Set {
            graph,
            set,
            line_width,
            line_style,
            symbol_size,
            symbol_skip,
            error_bars,
            error_size,
            fill_type,
            fill_color,
            baseline_type,
            baseline,
            drop_lines,
        } => {
            if let Some(color) = fill_color
                && color > 15
                && !project.colors.contains_key(&color)
            {
                return Err(Diagnostic::new("Select a defined fill color."));
            }
            let graph = graph_mut(project, graph)?;
            let set = graph
                .sets
                .get_mut(&set)
                .ok_or_else(|| Diagnostic::new("Select an existing set."))?;
            let mut settings = set.settings.clone();
            optional_number(&mut settings, "line linewidth", line_width, positive)?;
            optional_style(&mut settings, "line linestyle", line_style)?;
            optional_number(&mut settings, "symbol size", symbol_size, positive)?;
            if let Some(skip) = symbol_skip {
                settings.insert("symbol skip".into(), skip.to_string());
            }
            optional_bool(&mut settings, "errorbar", error_bars);
            optional_number(&mut settings, "errorbar size", error_size, positive)?;
            for (key, choice, maximum) in [
                ("fill type", fill_type, 2),
                ("baseline type", baseline_type, 5),
            ] {
                if let Some(choice) = choice {
                    if choice > maximum {
                        return Err(Diagnostic::new(format!("Select a valid {key}.")));
                    }
                    settings.insert(key.into(), choice.to_string());
                }
            }
            if let Some(color) = fill_color {
                settings.insert("fill color".into(), color.to_string());
            }
            optional_bool(&mut settings, "baseline", baseline);
            optional_bool(&mut settings, "dropline", drop_lines);
            set.settings = settings;
        }
        AppearanceOperation::Axis {
            graph,
            axis,
            visible,
            invert,
            grid,
            minor_grid,
            minor_ticks,
            precision,
            format,
            placement,
            direction,
            stagger,
            label,
        } => {
            let prefix = match axis.as_str() {
                "x" => "xaxis",
                "y" => "yaxis",
                "alternateX" => "altxaxis",
                "alternateY" => "altyaxis",
                _ => return Err(Diagnostic::new("Select a primary or alternate axis.")),
            };
            let graph = graph_mut(project, graph)?;
            let mut candidate = graph.clone();
            optional_bool(&mut candidate.settings, prefix, visible);
            if matches!(axis.as_str(), "x" | "y") {
                optional_bool(
                    &mut candidate.settings,
                    &format!("{axis}axes invert"),
                    invert,
                );
            }
            optional_bool(
                &mut candidate.settings,
                &format!("{prefix} tick major grid"),
                grid,
            );
            optional_bool(
                &mut candidate.settings,
                &format!("{prefix} tick minor grid"),
                minor_grid,
            );
            if let Some(ticks) = minor_ticks {
                if ticks > 99 {
                    return Err(Diagnostic::new("Minor ticks must be between 0 and 99."));
                }
                candidate
                    .settings
                    .insert(format!("{prefix} tick minor ticks"), ticks.to_string());
            }
            if let Some(precision) = precision {
                if precision > 12 {
                    return Err(Diagnostic::new("Tick precision must be between 0 and 12."));
                }
                candidate
                    .settings
                    .insert(format!("{prefix} ticklabel prec"), precision.to_string());
            }
            if let Some(format) = format {
                if !crate::tick_format::recognized(&format) {
                    return Err(Diagnostic::new("Select a supported tick label format."));
                }
                candidate
                    .settings
                    .insert(format!("{prefix} ticklabel format"), format);
            }
            for (key, selected, choices) in [
                ("ticklabel place", placement, ["normal", "opposite", "both"]),
                ("tick direction", direction, ["in", "out", "both"]),
            ] {
                if let Some(selected) = selected {
                    if !choices.contains(&selected.as_str()) {
                        return Err(Diagnostic::new("Select a valid tick option."));
                    }
                    candidate
                        .settings
                        .insert(format!("{prefix} {key}"), selected);
                }
            }
            if let Some(stagger) = stagger {
                if stagger > 10 {
                    return Err(Diagnostic::new("Tick staggering must be between 0 and 10."));
                }
                candidate
                    .settings
                    .insert(format!("{prefix} ticklabel stagger"), stagger.to_string());
            }
            if let Some(label) = label {
                candidate
                    .settings
                    .insert(format!("{prefix} label"), crate::quote(&label));
            }
            *graph = candidate;
        }
    }
    Ok(())
}

fn graph_mut(project: &mut Project, graph: u32) -> Result<&mut crate::Graph, Diagnostic> {
    project
        .graphs
        .get_mut(&graph)
        .ok_or_else(|| Diagnostic::new("Select an existing graph."))
}

fn graph_kind(kind: &str) -> Result<&'static str, Diagnostic> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "xy" => Ok("XY"),
        "chart" => Ok("Chart"),
        "polar" => Ok("Polar"),
        "pie" => Ok("Pie"),
        "fixed" => Ok("Fixed"),
        _ => Err(Diagnostic::new(
            "Graph type must be XY, Chart, Polar, Pie, or Fixed.",
        )),
    }
}

fn optional_bool(settings: &mut crate::Settings, key: &str, value: Option<bool>) {
    if let Some(value) = value {
        settings.insert(key.into(), if value { "on" } else { "off" }.into());
    }
}

fn optional_number(
    settings: &mut crate::Settings,
    key: &str,
    value: Option<f64>,
    validate: impl Fn(f64) -> bool,
) -> Result<(), Diagnostic> {
    if let Some(value) = value {
        if !validate(value) {
            return Err(Diagnostic::new(format!("Enter a valid {key}.")));
        }
        settings.insert(key.into(), value.to_string());
    }
    Ok(())
}

fn optional_style(
    settings: &mut crate::Settings,
    key: &str,
    value: Option<u8>,
) -> Result<(), Diagnostic> {
    if let Some(value) = value {
        if value > 8 {
            return Err(Diagnostic::new("Line style must be between 0 and 8."));
        }
        settings.insert(key.into(), value.to_string());
    }
    Ok(())
}

fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.
}

fn bar_gap_valid(value: f64) -> bool {
    value.is_finite() && (0. ..1.).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Project, dataset};

    #[test]
    fn graph_kind_requires_compatible_sets_without_changing_graph() {
        let mut project = Project::new();
        let graph = project.graphs.get_mut(&0).unwrap();
        dataset::replace(graph, None, dataset::SetKind::Xydx, vec![vec![0., 1., 0.1]]).unwrap();
        let error = apply(
            &mut project,
            AppearanceOperation::Graph {
                graph: 0,
                kind: Some("Pie".into()),
                legend: None,
                frame: None,
                frame_width: None,
                title_size: None,
                subtitle_size: None,
                stacked: None,
                bar_gap: None,
            },
        )
        .unwrap_err();
        assert!(error.message.contains("cannot"));
        assert_eq!(project.graphs[&0].kind, "XY");
    }

    #[test]
    fn axis_updates_only_requested_renderer_settings() {
        let mut project = Project::new();
        apply(
            &mut project,
            AppearanceOperation::Axis {
                graph: 0,
                axis: "x".into(),
                visible: None,
                invert: Some(true),
                grid: Some(true),
                minor_grid: Some(false),
                minor_ticks: Some(4),
                precision: Some(3),
                format: Some("decimal".into()),
                placement: None,
                direction: None,
                stagger: None,
                label: None,
            },
        )
        .unwrap();
        let settings = &project.graphs[&0].settings;
        assert_eq!(settings["xaxes invert"], "on");
        assert_eq!(settings["xaxis tick major grid"], "on");
        assert_eq!(settings["xaxis tick minor ticks"], "4");
        assert_eq!(settings["xaxis ticklabel format"], "decimal");
    }
}
