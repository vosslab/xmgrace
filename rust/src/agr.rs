use crate::{Diagnostic, Graph, Project, number, set_world};
use std::fmt::Write;

const DESKTOP_DEFAULTS: &str = include_str!("../data/desktop_defaults.agr");

const GRAPH_KEYS: &[&str] = &[
    "title",
    "subtitle",
    "title size",
    "subtitle size",
    "title color",
    "subtitle color",
    "legend",
    "legend loctype",
    "legend char size",
    "legend color",
    "frame color",
    "frame linewidth",
    "frame linestyle",
    "frame pattern",
    "frame background color",
    "frame background pattern",
    "xaxes scale",
    "yaxes scale",
    "xaxes invert",
    "yaxes invert",
];
const AXIS_KEYS: &[&str] = &[
    "label",
    "label char size",
    "label color",
    "bar",
    "bar color",
    "bar linewidth",
    "tick",
    "tick major",
    "tick minor ticks",
    "tick major size",
    "tick minor size",
    "tick major grid",
    "tick major color",
    "tick minor grid",
    "tick minor color",
    "tick place",
    "ticklabel",
    "ticklabel place",
    "ticklabel stagger",
    "ticklabel format",
    "ticklabel prec",
    "ticklabel prepend",
    "ticklabel append",
    "ticklabel char size",
    "ticklabel color",
];
const SET_KEYS: &[&str] = &[
    "hidden",
    "symbol",
    "symbol size",
    "symbol color",
    "symbol pattern",
    "symbol fill color",
    "symbol fill pattern",
    "symbol linewidth",
    "symbol char",
    "symbol skip",
    "line type",
    "line linestyle",
    "line linewidth",
    "line color",
    "line pattern",
    "baseline type",
    "baseline",
    "dropline",
    "fill type",
    "fill rule",
    "fill color",
    "fill pattern",
    "legend",
    "comment",
    "errorbar",
    "errorbar place",
    "errorbar color",
    "errorbar size",
    "errorbar linewidth",
    "errorbar linestyle",
    "errorbar riser linewidth",
    "errorbar riser linestyle",
];
const PASSIVE_SET: &[(&str, &str)] = &[
    ("symbol linestyle", "1"),
    ("symbol char font", "0"),
    ("avalue", "off"),
    ("avalue type", "2"),
    ("avalue char size", "1.000000"),
    ("avalue font", "0"),
    ("avalue color", "1"),
    ("avalue rot", "0"),
    ("avalue format", "general"),
    ("avalue prec", "3"),
    ("avalue prepend", "\"\""),
    ("avalue append", "\"\""),
    ("avalue offset", "0.000000 , 0.000000"),
    ("errorbar pattern", "1"),
    ("errorbar riser clip", "off"),
    ("errorbar riser clip length", "0.100000"),
];

pub(crate) fn decode(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        text.trim_start_matches('\u{feff}').to_owned()
    } else {
        bytes.iter().copied().map(char::from).collect()
    }
}

fn columns<const N: usize>(input: &str) -> Option<[f64; N]> {
    input
        .split(',')
        .map(|field| number(field.trim()))
        .collect::<Option<Vec<_>>>()?
        .try_into()
        .ok()
}

fn rectangle(input: &str, mut bounds: [f64; 4]) -> Option<[f64; 4]> {
    if let Some(rectangle) = columns(input) {
        return Some(rectangle);
    }
    let (field, value) = split_first(input);
    let index = ["xmin", "ymin", "xmax", "ymax"]
        .iter()
        .position(|&name| name == field)?;
    bounds[index] = number(value)?;
    Some(bounds)
}

fn numbered(input: &str, prefix: char) -> Option<(u32, &str)> {
    let rest = input.strip_prefix(prefix)?;
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    Some((rest[..end].parse().ok()?, rest[end..].trim_start()))
}

fn property<'a>(input: &'a str, keys: &[&str]) -> Option<(&'a str, &'a str)> {
    keys.iter()
        .filter_map(|key| {
            let value = input.strip_prefix(key)?.strip_prefix(' ')?.trim_start();
            Some((&input[..key.len()], value))
        })
        .max_by_key(|(key, _)| key.len())
}

fn split_first(input: &str) -> (&str, &str) {
    input
        .split_once(' ')
        .map_or((input, ""), |(first, rest)| (first, rest.trim_start()))
}

fn unsupported(project: &mut Project, line: usize, command: &str) {
    if project.diagnostics.len() < 40 {
        let description: String = command.chars().take(100).collect();
        project.diagnostics.push(Diagnostic::at(
            line,
            format!("Unsupported Grace setting: {description}"),
        ));
    }
}

fn width(kind: &str) -> Option<usize> {
    crate::dataset::SetKind::parse(kind)
        .ok()
        .map(crate::dataset::SetKind::width)
}

fn graph_kind_supported(kind: &str) -> bool {
    matches!(
        kind.to_ascii_lowercase().as_str(),
        "xy" | "chart" | "polar" | "pie" | "fixed"
    )
}

pub(crate) fn set_kind_supported_for_graph(graph_kind: &str, set_kind: &str) -> bool {
    match graph_kind.to_ascii_lowercase().as_str() {
        "chart" => matches!(set_kind, "xy" | "bar" | "bardy" | "bardydy"),
        "polar" | "pie" => set_kind == "xy",
        "xy" | "fixed" => matches!(
            set_kind,
            "xy" | "xydx"
                | "xydy"
                | "xydxdx"
                | "xydydy"
                | "xydxdy"
                | "xydxdxdydy"
                | "xyhilo"
                | "xyr"
                | "xysize"
                | "xycolor"
                | "xycolpat"
                | "xyvmap"
                | "xyboxplot"
                | "xyz"
        ),
        _ => false,
    }
}

fn drawing_kind(command: &str) -> Option<&'static str> {
    let kind = command.strip_prefix("with ")?.trim();
    ["string", "line", "box", "ellipse"]
        .into_iter()
        .find(|candidate| kind.eq_ignore_ascii_case(candidate))
}

fn drawing_property(command: &str, kind: &str) -> bool {
    command
        .get(..kind.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(kind))
        && command[kind.len()..]
            .chars()
            .next()
            .is_none_or(|character| character.is_ascii_whitespace())
}

fn drawing_complete(command: &str, kind: &str) -> bool {
    command.trim().eq_ignore_ascii_case(&format!("{kind} def"))
}

fn finish_drawing(project: &mut Project, commands: &mut Vec<(usize, String)>) {
    if commands.is_empty() {
        return;
    }
    let fallback_line = commands[0].0;
    let references = commands
        .iter()
        .map(|(line, command)| (*line, command.as_str()))
        .collect::<Vec<_>>();
    match crate::objects::parse_block(&references) {
        Ok(object) => {
            let id = (0..)
                .find(|id| !project.objects.contains_key(id))
                .expect("u32 drawing object identifiers are finite");
            project.objects.insert(id, object);
        }
        Err(mut diagnostic) => {
            if diagnostic.line.is_none() {
                diagnostic.line = diagnostic
                    .message
                    .strip_prefix("Line ")
                    .and_then(|message| message.split_once(':'))
                    .and_then(|(line, _)| line.parse().ok())
                    .or(Some(fallback_line));
            }
            project.diagnostics.push(diagnostic);
        }
    }
    commands.clear();
}

fn region_directive(command: &str) -> bool {
    let (head, tail) = split_first(command);
    let region_number = |value: &str| {
        value
            .strip_prefix('r')
            .or_else(|| value.strip_prefix('R'))
            .is_some_and(|rest| {
                rest.chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_digit())
            })
    };
    region_number(head) || (head.eq_ignore_ascii_case("link") && region_number(tail))
}

pub fn open(bytes: &[u8]) -> Result<Project, Diagnostic> {
    let text = decode(bytes);
    let mut project = Project::for_agr_import();
    let mut graph_id = 0;
    let mut target: Option<(u32, u32)> = None;
    let mut effective_type = None;
    let mut saw_version = false;
    let mut drawing: Vec<(usize, String)> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let raw = raw.trim();
        if raw.is_empty() || raw.starts_with('#') {
            continue;
        }
        if raw == "&" {
            target = None;
            effective_type = None;
            continue;
        }
        if let Some(command) = raw.strip_prefix('@') {
            let command = command.trim();
            if let Some((_, opener)) = drawing.first() {
                let kind = drawing_kind(opener).expect("drawing opener is validated");
                if drawing_property(command, kind) {
                    drawing.push((line, command.into()));
                    if drawing_complete(command, kind) {
                        finish_drawing(&mut project, &mut drawing);
                    }
                    continue;
                }
                finish_drawing(&mut project, &mut drawing);
            }
            if drawing_kind(command).is_some() {
                drawing.push((line, command.into()));
                continue;
            }
            if region_directive(command)
                && crate::regions::parse_agr(&mut project, command)
                    .map_err(|error| Diagnostic::at(line, error.message))?
            {
                continue;
            }
            let (head, tail) = split_first(command);
            match head.to_ascii_lowercase().as_str() {
                "version" => {
                    let version = tail
                        .parse::<u32>()
                        .map_err(|_| Diagnostic::at(line, "Invalid Grace project version"))?;
                    saw_version = true;
                    if !(50000..=50125).contains(&version) {
                        unsupported(&mut project, line, command);
                    }
                }
                "target" => {
                    let value = tail.to_ascii_lowercase();
                    target = if let Some((graph, set)) = value.split_once('.') {
                        numbered(graph, 'g')
                            .filter(|(_, rest)| rest.is_empty())
                            .and_then(|(g, _)| {
                                numbered(set, 's')
                                    .filter(|(_, rest)| rest.is_empty())
                                    .map(|(s, _)| (g, s))
                            })
                    } else {
                        numbered(&value, 's')
                            .filter(|(_, rest)| rest.is_empty())
                            .map(|(s, _)| (graph_id, s))
                    };
                    if target.is_none() {
                        return Err(Diagnostic::at(line, "Invalid dataset target"));
                    }
                    effective_type = None;
                }
                "type" if target.is_some() => {
                    let (g, s) = target.expect("target is present");
                    let kind = tail.to_ascii_lowercase();
                    project
                        .graphs
                        .entry(g)
                        .or_default()
                        .sets
                        .entry(s)
                        .or_default()
                        .kind = kind.clone();
                    if width(&kind).is_none() {
                        unsupported(&mut project, line, command);
                    }
                    effective_type = Some(kind);
                }
                _ => apply(&mut project, &mut graph_id, line, command)?,
            }
            continue;
        }
        finish_drawing(&mut project, &mut drawing);
        let Some((g, s)) = target else {
            return Err(Diagnostic::at(line, "Data row has no Grace dataset target"));
        };
        let kind = effective_type.as_deref().unwrap_or("xy");
        let Some(expected) = width(kind) else {
            continue;
        };
        let row: Option<Vec<_>> = raw.split_ascii_whitespace().map(number).collect();
        let Some(row) = row.filter(|row| row.len() == expected) else {
            return Err(Diagnostic::at(
                line,
                format!("Expected {expected} finite columns for {kind}"),
            ));
        };
        let set = project
            .graphs
            .entry(g)
            .or_default()
            .sets
            .entry(s)
            .or_default();
        set.kind = kind.into();
        set.rows.push(row);
        set.count += 1;
    }
    finish_drawing(&mut project, &mut drawing);
    if !saw_version {
        return Err(Diagnostic::new(
            "Expected a Grace project with an @version directive.",
        ));
    }
    if project.graphs.is_empty() {
        project.graphs.insert(0, Graph::default());
    }
    for graph in project.graphs.values_mut() {
        set_world(graph, graph.world)?;
        let [left, bottom, right, top] = graph.view;
        if left >= right || bottom >= top {
            return Err(Diagnostic::new("Viewport limits must increase."));
        }
        for set in graph.sets.values() {
            if !set_kind_supported_for_graph(&graph.kind, &set.kind) {
                project.diagnostics.push(Diagnostic::new(format!(
                    "Unsupported {} set in {} graph",
                    set.kind, graph.kind
                )));
            }
        }
    }
    for (id, object) in &project.objects {
        if let Some(graph) = object.linked_graph()
            && !project.graphs.contains_key(&graph)
        {
            project.diagnostics.push(Diagnostic::new(format!(
                "Drawing object {id} refers to missing graph {graph}"
            )));
        }
    }
    Ok(project)
}

fn apply(
    project: &mut Project,
    graph_id: &mut u32,
    line: usize,
    command: &str,
) -> Result<(), Diagnostic> {
    let normalized = command
        .split_ascii_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let is_default = || {
        DESKTOP_DEFAULTS.lines().any(|raw| {
            raw.strip_prefix('@').is_some_and(|entry| {
                entry
                    .split_ascii_whitespace()
                    .eq(normalized.split_ascii_whitespace())
            })
        })
    };
    if let Some(value) = command.strip_prefix("page size ") {
        let value = value.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
        project.page = columns(&value)
            .or_else(|| {
                let (width, height) = value.split_once(' ')?;
                Some([number(width)?, number(height)?])
            })
            .filter(|page| page.iter().all(|size| *size > 0. && *size <= 100_000.))
            .ok_or_else(|| Diagnostic::at(line, "Invalid page size"))?;
    } else if let Some(value) = command.strip_prefix("background color ") {
        project.background = value
            .parse()
            .map_err(|_| Diagnostic::at(line, "Invalid background color"))?;
    } else if let Some(value) = command.strip_prefix("page background fill ") {
        project.fill_page = value == "on";
    } else if let Some(value) = command.strip_prefix("reference date ") {
        project.reference_date =
            number(value).ok_or_else(|| Diagnostic::at(line, "Invalid reference date"))?;
    } else if let Some(value) = command.strip_prefix("timestamp def ")
        && value.len() >= 2
        && value.starts_with('"')
        && value.ends_with('"')
        && !crate::unquote(value).contains(['\\', '"'])
    {
        project.timestamp = Some(value.into());
    } else if let Some(value) = command.strip_prefix("map color ") {
        let (id, specification) = value
            .split_once(" to ")
            .ok_or_else(|| Diagnostic::at(line, "Invalid color mapping"))?;
        let (rgb, _) = specification
            .strip_prefix('(')
            .and_then(|entry| entry.split_once(')'))
            .ok_or_else(|| Diagnostic::at(line, "Invalid color mapping"))?;
        let rgb: Option<Vec<u8>> = rgb
            .split(',')
            .map(|part| part.trim().parse().ok())
            .collect();
        let rgb = rgb
            .and_then(|rgb| <[u8; 3]>::try_from(rgb).ok())
            .ok_or_else(|| Diagnostic::at(line, "Invalid RGB values"))?;
        project.colors.insert(
            id.parse()
                .map_err(|_| Diagnostic::at(line, "Invalid color index"))?,
            rgb,
        );
        project.color_commands.push(command.into());
    } else if let Some((g, "")) = command
        .strip_prefix("with ")
        .and_then(|rest| numbered(rest, 'g'))
    {
        *graph_id = g;
        project.graphs.entry(g).or_default();
    } else if let Some((g, setting)) = numbered(command, 'g') {
        let graph = project.graphs.entry(g).or_default();
        match setting {
            "on" | "hidden false" => graph.visible = true,
            "off" | "hidden true" => graph.visible = false,
            other if other.starts_with("type ") => {
                graph.kind = other[5..].into();
                if !graph_kind_supported(&graph.kind) {
                    unsupported(project, line, command);
                }
            }
            setting if setting.starts_with("stacked ") => {
                let value = setting["stacked ".len()..].trim();
                if matches!(value, "true" | "false" | "on" | "off") {
                    graph.settings.insert("stacked".into(), value.into());
                } else {
                    unsupported(project, line, command);
                }
            }
            setting if setting.starts_with("bar hgap ") => {
                let value = setting["bar hgap ".len()..].trim();
                if number(value).is_some_and(|gap| (0. ..1.).contains(&gap)) {
                    graph.settings.insert("bar hgap".into(), value.into());
                } else {
                    unsupported(project, line, command);
                }
            }
            "fixedpoint off"
            | "fixedpoint type 0"
            | "fixedpoint xy 0.000000, 0.000000"
            | "fixedpoint format general general"
            | "fixedpoint prec 6, 6" => {}
            _ if is_default() => {}
            _ => unsupported(project, line, command),
        }
    } else if let Some((set_id, setting)) = numbered(command, 's') {
        let set = project
            .graphs
            .entry(*graph_id)
            .or_default()
            .sets
            .entry(set_id)
            .or_default();
        if let Some(kind) = setting.strip_prefix("type ") {
            set.kind = kind.to_ascii_lowercase();
            if width(&set.kind).is_none() {
                unsupported(project, line, command);
            }
        } else if PASSIVE_SET.iter().any(|(key, value)| {
            setting
                .strip_prefix(key)
                .and_then(|x| x.strip_prefix(' '))
                .is_some_and(|rest| rest.trim() == *value)
        }) {
        } else if let Some((key, value)) = property(setting, SET_KEYS) {
            if !valid_property(key, value) {
                unsupported(project, line, command);
            } else {
                set.settings.insert(key.into(), value.into());
            }
        } else {
            unsupported(project, line, command);
        }
    } else if let Some((key, value)) = graph_property(&normalized, command) {
        let graph = project.graphs.entry(*graph_id).or_default();
        match key.as_str() {
            "world" => {
                graph.world = rectangle(value, graph.world)
                    .ok_or_else(|| Diagnostic::at(line, "Invalid world limits"))?
            }
            "view" => {
                graph.view = rectangle(value, graph.view)
                    .filter(|v| v.iter().all(|n| n.abs() <= 100.))
                    .ok_or_else(|| Diagnostic::at(line, "Invalid viewport"))?
            }
            _ if valid_property(&key, value) => {
                let key = if key == "legend" && value.contains(',') {
                    "legend position".into()
                } else {
                    key
                };
                graph.settings.insert(key, value.into());
            }
            _ if is_default() => {}
            _ => unsupported(project, line, command),
        }
    } else if !is_default() {
        unsupported(project, line, command);
    }
    Ok(())
}

fn graph_property<'a>(normalized: &str, raw: &'a str) -> Option<(String, &'a str)> {
    let (head, tail) = split_first(normalized);
    if matches!(head, "xaxis" | "yaxis" | "altxaxis" | "altyaxis") {
        let rest = split_first(raw).1;
        if matches!(tail, "on" | "off") {
            return Some((head.into(), rest));
        }
        if matches!(tail, "tick in" | "tick out" | "tick both") {
            return Some((format!("{head} tick direction"), split_first(rest).1));
        }
        let (key, _) = property(tail, AXIS_KEYS)?;
        let length = key.split_ascii_whitespace().count();
        let mut remaining = rest;
        for _ in 0..length {
            remaining = split_first(remaining).1;
        }
        return Some((format!("{head} {key}"), remaining));
    }
    property(raw, &["world", "view"])
        .or_else(|| property(raw, GRAPH_KEYS))
        .map(|(key, value)| (key.into(), value))
}

fn valid_property(key: &str, value: &str) -> bool {
    if value.starts_with('"') {
        return (matches!(key, "title" | "subtitle" | "legend" | "comment")
            || [" label", " prepend", " append"]
                .iter()
                .any(|suffix| key.ends_with(suffix)))
            && value.len() >= 2
            && value.ends_with('"')
            && quoted_contents_valid(crate::unquote(value));
    }
    match key {
        "xaxes scale" | "yaxes scale" => crate::scale::Scale::parse(value).is_some(),
        "legend loctype" => matches!(value, "view" | "world"),
        "legend" if value.contains(',') => columns::<2>(value).is_some(),
        "line type" => matches!(value, "0" | "1" | "2" | "3"),
        "symbol" => value.parse::<u8>().is_ok_and(|symbol| symbol <= 11),
        "symbol char" => value.parse::<u8>().is_ok(),
        "errorbar place" => matches!(value, "normal" | "opposite" | "both"),
        "baseline type" => value.parse::<u8>().is_ok_and(|kind| kind <= 5),
        "fill type" => value.parse::<u8>().is_ok_and(|kind| kind <= 2),
        "fill rule" => value.parse::<u8>().is_ok_and(|rule| rule <= 1),
        _ if key.ends_with("ticklabel format") => crate::tick_format::recognized(value),
        _ if key.ends_with("tick direction") => matches!(value, "in" | "out" | "both"),
        _ if key.ends_with("tick place") || key.ends_with("ticklabel place") => {
            matches!(value, "normal" | "opposite" | "both")
        }
        _ if key.ends_with("pattern") => matches!(value, "0" | "1"),
        _ if key.ends_with("linestyle") => value.parse::<u8>().is_ok_and(|style| style <= 8),
        _ => number(value).is_some() || matches!(value, "on" | "off" | "true" | "false"),
    }
}

fn quoted_contents_valid(value: &str) -> bool {
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return false;
        }
    }
    !escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_kinds_backed_by_the_renderer_are_editable() {
        for kind in ["XY", "Chart", "Polar", "Pie", "Fixed"] {
            let source = format!(
                "@version 50125\n@g0 type {kind}\n@with g0\n@target G0.S0\n@type xy\n0 1\n1 2\n&\n"
            );
            let project = open(source.as_bytes()).expect("open supported graph kind");
            assert!(
                project.diagnostics.is_empty(),
                "{kind}: {:?}",
                project.diagnostics
            );
        }
    }

    #[test]
    fn chart_bar_data_keeps_chart_options() {
        let project = open(
            b"@version 50125\n@g0 type Chart\n@g0 stacked true\n@g0 bar hgap 0.005\n@with g0\n@target G0.S0\n@type bar\n0 1\n1 2\n&\n",
        )
        .expect("open chart");
        let graph = &project.graphs[&0];
        assert!(project.diagnostics.is_empty(), "{:?}", project.diagnostics);
        assert_eq!(graph.settings["stacked"], "true");
        assert_eq!(graph.settings["bar hgap"], "0.005");
        assert_eq!(graph.sets[&0].kind, "bar");
    }

    #[test]
    fn chart_settings_save_as_classic_graph_directives() {
        let source = b"@version 50125\n@g0 type Chart\n@g0 stacked true\n@g0 bar hgap 0.005\n@with g0\n@target G0.S0\n@type bar\n0 1\n1 2\n&\n";
        let project = open(source).expect("open chart");
        let saved = String::from_utf8(save(&project).expect("save chart")).expect("UTF-8 AGR");
        assert!(saved.contains("@g0 stacked true\n@g0 bar hgap 0.005\n@with g0"));
        assert!(!saved.contains("@    stacked"));
        assert!(!saved.contains("@    bar hgap"));

        let reopened = open(saved.as_bytes()).expect("reopen chart");
        assert!(
            reopened.diagnostics.is_empty(),
            "{:?}",
            reopened.diagnostics
        );
        assert_eq!(reopened.graphs[&0].settings["stacked"], "true");
        assert_eq!(reopened.graphs[&0].settings["bar hgap"], "0.005");
    }

    #[test]
    fn empty_placeholder_set_is_not_serialized_as_an_invalid_type() {
        let mut project = Project::new();
        project
            .graphs
            .get_mut(&0)
            .expect("default graph")
            .sets
            .insert(9, crate::Set::default());
        let saved = save(&project).expect("save placeholder set");
        assert!(!String::from_utf8_lossy(&saved).contains("s9 type"));
        let reopened = open(&saved).expect("reopen placeholder set");
        assert!(
            reopened.diagnostics.is_empty(),
            "{:?}",
            reopened.diagnostics
        );
    }

    #[test]
    fn drawing_objects_round_trip_before_their_world_graph() {
        let source = b"@version 50125\n@with string\n@    string loctype world\n@    string g9\n@    string 1, 2\n@    string def \"world label\"\n@string def\n@with line\n@    line 1, 2, 3, 4\n@    line arrow 3\n@line def\n@with box\n@    box 0.1, 0.2, 0.3, 0.4\n@box def\n@with ellipse\n@    ellipse 0.2, 0.3, 0.4, 0.5\n@ellipse def\n@with g9\n";
        let project = open(source).expect("open objects");
        assert!(project.diagnostics.is_empty(), "{:?}", project.diagnostics);
        assert_eq!(project.objects.len(), 4);

        let saved = save(&project).expect("save objects");
        let reopened = open(&saved).expect("reopen objects");
        assert!(
            reopened.diagnostics.is_empty(),
            "{:?}",
            reopened.diagnostics
        );
        assert_eq!(reopened.objects, project.objects);
    }

    #[test]
    fn malformed_drawing_object_keeps_its_source_line_diagnostic() {
        let project = open(b"@version 50125\n@with line\n@    line loctype nowhere\n@line def\n")
            .expect("preview malformed object");
        assert_eq!(project.diagnostics.len(), 1);
        assert_eq!(project.diagnostics[0].line, Some(3));
    }

    #[test]
    fn regions_round_trip_with_graph_links() {
        let source = b"@version 50125\n@r4 on\n@link r4 to g2\n@r4 type polyi\n@r4 xy 0, 0\n@r4 xy 1, 0\n@r4 xy 0, 1\n@with g2\n";
        let project = open(source).expect("open regions");
        assert!(project.diagnostics.is_empty(), "{:?}", project.diagnostics);
        assert_eq!(project.regions.len(), 1);

        let saved = save(&project).expect("save regions");
        let reopened = open(&saved).expect("reopen regions");
        assert_eq!(reopened.regions, project.regions);
    }

    #[test]
    fn alternate_axes_and_calendar_reference_round_trip() {
        let project = open(b"@version 50125\n@reference date 2440587.5\n@with g0\n@world 0, 0, 1, 1\n@altxaxis on\n@altxaxis tick out\n@altxaxis tick major 1\n@altxaxis ticklabel format yymmdd\n").unwrap();
        assert!(project.diagnostics.is_empty(), "{:?}", project.diagnostics);
        let result = save(&project).unwrap();
        let reopened = open(&result).unwrap();
        assert_eq!(reopened.reference_date, 2_440_587.5);
        assert_eq!(reopened.graphs[&0].settings, project.graphs[&0].settings);
        assert!(crate::svg(&reopened).contains("1970-01-01"));
    }

    #[test]
    fn preserves_set_area_fill_and_baseline_options() {
        let source = b"@version 50125\n@with g0\n@s0 type xy\n@s0 fill type 2\n@s0 fill rule 1\n@s0 fill color 6\n@s0 fill pattern 1\n@s0 baseline type 5\n@s0 baseline on\n@s0 dropline on\n@target G0.S0\n@type xy\n0 1\n1 2\n&\n";
        let project = open(source).expect("open filled set");
        assert!(project.diagnostics.is_empty(), "{:?}", project.diagnostics);
        let settings = &project.graphs[&0].sets[&0].settings;
        assert_eq!(settings["fill type"], "2");
        assert_eq!(settings["fill pattern"], "1");
        assert_eq!(settings["baseline type"], "5");
        let reopened = open(&save(&project).expect("save filled set")).expect("reopen filled set");
        assert_eq!(reopened.graphs[&0].sets[&0].settings, *settings);
    }
}

pub fn save(project: &Project) -> Result<Vec<u8>, Diagnostic> {
    project.require_editable()?;
    let mut output = format!(
        "# Grace project file\n@version 50125\n@page size {}, {}\n@background color {}\n@page background fill {}\n",
        project.page[0] as u32,
        project.page[1] as u32,
        project.background,
        if project.fill_page { "on" } else { "off" }
    );
    for entry in &project.color_commands {
        let _ = writeln!(output, "@{entry}");
    }
    if project.reference_date != 0. {
        let _ = writeln!(output, "@reference date {}", project.reference_date);
    }
    if let Some(timestamp) = &project.timestamp {
        let _ = writeln!(output, "@timestamp off\n@timestamp def {timestamp}");
    }
    for (g, graph) in &project.graphs {
        for (set_id, set) in &graph.sets {
            if set.kind.is_empty() && (!set.rows.is_empty() || !set.settings.is_empty()) {
                return Err(Diagnostic::new(format!(
                    "Set {set_id} has data or settings but no data type."
                )));
            }
        }
        let _ = writeln!(
            output,
            "@g{g} on\n@g{g} hidden {}\n@g{g} type {}",
            !graph.visible, graph.kind
        );
        if let Some(value) = graph.settings.get("stacked") {
            let stacked = match value.as_str() {
                "on" | "true" => "true",
                "off" | "false" => "false",
                _ => return Err(Diagnostic::new("Invalid Chart stacked setting.")),
            };
            let _ = writeln!(output, "@g{g} stacked {stacked}");
        }
        if let Some(value) = graph.settings.get("bar hgap") {
            let gap = number(value)
                .filter(|gap| (0. ..1.).contains(gap))
                .ok_or_else(|| Diagnostic::new("Invalid Chart bar hgap setting."))?;
            let _ = writeln!(output, "@g{g} bar hgap {gap}");
        }
        let _ = writeln!(output, "@with g{g}");
        for (key, [a, b, c, d]) in [("world", graph.world), ("view", graph.view)] {
            let _ = writeln!(output, "@    {key} {a}, {b}, {c}, {d}");
        }
        for (key, value) in &graph.settings {
            if matches!(key.as_str(), "stacked" | "bar hgap") {
                continue;
            }
            if let Some(prefix) = key.strip_suffix(" tick direction") {
                let _ = writeln!(output, "@    {prefix} tick {value}");
                continue;
            }
            let key = if key == "legend position" {
                "legend"
            } else {
                key
            };
            let _ = writeln!(output, "@    {key} {value}");
        }
        for (s, set) in &graph.sets {
            if set.kind.is_empty() {
                continue;
            }
            let _ = writeln!(output, "@    s{s} type {}", set.kind);
            for (key, value) in &set.settings {
                let _ = writeln!(output, "@    s{s} {key} {value}");
            }
        }
    }
    for object in project.objects.values() {
        output.push_str(&crate::objects::write_agr(object));
    }
    output.push_str(&crate::regions::write_agr(&project.regions));
    for (g, graph) in &project.graphs {
        for (s, set) in &graph.sets {
            if set.rows.is_empty() {
                continue;
            }
            let _ = writeln!(output, "@target G{g}.S{s}\n@type {}", set.kind);
            for row in &set.rows {
                let joined = row.iter().map(f64::to_string).collect::<Vec<_>>().join(" ");
                let _ = writeln!(output, "{joined}");
            }
            output.push_str("&\n");
        }
    }
    Ok(output
        .chars()
        .map(|ch| u8::try_from(u32::from(ch)))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|_| output.into_bytes()))
}
