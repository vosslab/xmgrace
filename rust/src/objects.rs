//! Typed drawing objects shared by AGR import, the browser editor, and SVG output.
//!
//! Grace stores drawing objects outside graphs. A world-space object names the graph whose
//! coordinates it uses, while a view-space object uses the page's normalized viewport.

use crate::{Diagnostic, Project};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "space", rename_all = "camelCase")]
pub(crate) enum CoordinateSpace {
    View,
    World { graph: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct Stroke {
    pub(crate) color: u32,
    pub(crate) width: f64,
    pub(crate) style: u8,
}

impl Default for Stroke {
    fn default() -> Self {
        Self {
            color: 1,
            width: 1.,
            style: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct Fill {
    pub(crate) color: u32,
    pub(crate) pattern: u8,
}

impl Default for Fill {
    fn default() -> Self {
        Self {
            color: 1,
            pattern: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct Arrow {
    /// 0 selects no arrow, 1 the first end, 2 the second end, and 3 both ends.
    pub(crate) ends: u8,
    pub(crate) kind: u8,
    pub(crate) size: f64,
    pub(crate) layout: [f64; 2],
}

impl Default for Arrow {
    fn default() -> Self {
        Self {
            ends: 0,
            kind: 1,
            size: 1.,
            layout: [1., 1.],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct TextStyle {
    pub(crate) color: u32,
    pub(crate) rotation: f64,
    pub(crate) font: u8,
    pub(crate) justification: u8,
    pub(crate) size: f64,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            color: 1,
            rotation: 0.,
            font: 0,
            justification: 0,
            size: 1.,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum DrawingObject {
    Text {
        visible: bool,
        coordinates: [f64; 2],
        space: CoordinateSpace,
        text: String,
        style: TextStyle,
    },
    Line {
        visible: bool,
        coordinates: [f64; 4],
        space: CoordinateSpace,
        stroke: Stroke,
        arrow: Arrow,
    },
    Box {
        visible: bool,
        coordinates: [f64; 4],
        space: CoordinateSpace,
        stroke: Stroke,
        fill: Fill,
    },
    Ellipse {
        visible: bool,
        coordinates: [f64; 4],
        space: CoordinateSpace,
        stroke: Stroke,
        fill: Fill,
    },
}

impl DrawingObject {
    pub(crate) fn linked_graph(&self) -> Option<u32> {
        let space = match self {
            Self::Text { space, .. }
            | Self::Line { space, .. }
            | Self::Box { space, .. }
            | Self::Ellipse { space, .. } => space,
        };
        match space {
            CoordinateSpace::View => None,
            CoordinateSpace::World { graph } => Some(*graph),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub(crate) enum ObjectOperation {
    Create { object: DrawingObject },
    Replace { id: u32, object: DrawingObject },
    Delete { id: u32 },
    Visibility { id: u32, visible: bool },
}

pub(crate) fn apply(project: &mut Project, operation: ObjectOperation) -> Result<(), Diagnostic> {
    project.require_editable()?;
    match operation {
        ObjectOperation::Create { object } => {
            validate(project, &object)?;
            let id = next_id(project);
            project.objects.insert(id, object);
        }
        ObjectOperation::Replace { id, object } => {
            validate(project, &object)?;
            if !project.objects.contains_key(&id) {
                return Err(Diagnostic::new("Select an existing drawing object."));
            }
            project.objects.insert(id, object);
        }
        ObjectOperation::Delete { id } => {
            if project.objects.remove(&id).is_none() {
                return Err(Diagnostic::new("Select an existing drawing object."));
            }
        }
        ObjectOperation::Visibility { id, visible } => {
            let object = project
                .objects
                .get_mut(&id)
                .ok_or_else(|| Diagnostic::new("Select an existing drawing object."))?;
            visible_mut(object).clone_from(&visible);
        }
    }
    Ok(())
}

fn next_id(project: &Project) -> u32 {
    (0..)
        .find(|id| !project.objects.contains_key(id))
        .expect("u32 id space is finite")
}

fn visible_mut(object: &mut DrawingObject) -> &mut bool {
    match object {
        DrawingObject::Text { visible, .. }
        | DrawingObject::Line { visible, .. }
        | DrawingObject::Box { visible, .. }
        | DrawingObject::Ellipse { visible, .. } => visible,
    }
}

fn validate(project: &Project, object: &DrawingObject) -> Result<(), Diagnostic> {
    let (coordinates, space, line, fill, text) = match object {
        DrawingObject::Text {
            coordinates,
            space,
            style,
            text,
            ..
        } => {
            if !style.rotation.is_finite() || !style.size.is_finite() || style.size <= 0. {
                return Err(Diagnostic::new(
                    "Text rotation and size must be finite, with a positive size.",
                ));
            }
            (coordinates.as_slice(), space, None, None, Some(text))
        }
        DrawingObject::Line {
            coordinates,
            space,
            stroke,
            arrow,
            ..
        } => {
            if !(0..=3).contains(&arrow.ends)
                || !(0..=2).contains(&arrow.kind)
                || !arrow.size.is_finite()
                || arrow.size <= 0.
                || !arrow.layout.iter().all(|value| value.is_finite())
            {
                return Err(Diagnostic::new("Invalid arrow settings."));
            }
            (coordinates.as_slice(), space, Some(stroke), None, None)
        }
        DrawingObject::Box {
            coordinates,
            space,
            stroke,
            fill,
            ..
        }
        | DrawingObject::Ellipse {
            coordinates,
            space,
            stroke,
            fill,
            ..
        } => (
            coordinates.as_slice(),
            space,
            Some(stroke),
            Some(fill),
            None,
        ),
    };
    if !coordinates.iter().all(|value| value.is_finite()) {
        return Err(Diagnostic::new("Object coordinates must be finite."));
    }
    if line.is_some_and(|stroke| !stroke.width.is_finite() || stroke.width <= 0.) {
        return Err(Diagnostic::new("Line width must be finite and positive."));
    }
    if text.is_some_and(|text| text.contains(['\r', '\n'])) {
        return Err(Diagnostic::new("Text must be a single line."));
    }
    if let CoordinateSpace::World { graph } = space
        && !project.graphs.contains_key(graph)
    {
        return Err(Diagnostic::new(
            "Select an existing graph for a world-space object.",
        ));
    }
    if fill.is_some_and(|fill| fill.pattern > 31) {
        return Err(Diagnostic::new("Invalid fill pattern."));
    }
    Ok(())
}

/// Parses one classic Grace annotation block. `commands` includes the `with` opener and its
/// property lines, with line numbers provided for useful errors.
pub(crate) fn parse_block(commands: &[(usize, &str)]) -> Result<DrawingObject, Diagnostic> {
    let (_, first) = commands
        .first()
        .ok_or_else(|| Diagnostic::new("Empty drawing object."))?;
    let kind = first
        .trim()
        .trim_start_matches('@')
        .trim_start()
        .strip_prefix("with ")
        .ok_or_else(|| Diagnostic::new("Invalid drawing object."))?;
    let kind = kind.trim().to_ascii_lowercase();
    let mut object = match kind.as_str() {
        "string" => DrawingObject::Text {
            visible: true,
            coordinates: [0., 0.],
            space: CoordinateSpace::View,
            text: String::new(),
            style: TextStyle::default(),
        },
        "line" => DrawingObject::Line {
            visible: true,
            coordinates: [0., 0., 0., 0.],
            space: CoordinateSpace::View,
            stroke: Stroke::default(),
            arrow: Arrow::default(),
        },
        "box" => DrawingObject::Box {
            visible: true,
            coordinates: [0., 0., 0., 0.],
            space: CoordinateSpace::View,
            stroke: Stroke::default(),
            fill: Fill::default(),
        },
        "ellipse" => DrawingObject::Ellipse {
            visible: true,
            coordinates: [0., 0., 0., 0.],
            space: CoordinateSpace::View,
            stroke: Stroke::default(),
            fill: Fill::default(),
        },
        _ => return Err(Diagnostic::new("Unsupported drawing object.")),
    };
    let prefix = kind.as_str();
    for &(line, command) in commands.iter().skip(1) {
        let command = command.trim().trim_start_matches('@').trim_start();
        if command.eq_ignore_ascii_case(&format!("{prefix} def")) {
            break;
        }
        parse_property(&mut object, prefix, line, command)?;
    }
    Ok(object)
}

fn parse_property(
    object: &mut DrawingObject,
    prefix: &str,
    line: usize,
    command: &str,
) -> Result<(), Diagnostic> {
    let content = command
        .strip_prefix(prefix)
        .map(str::trim_start)
        .ok_or_else(|| error(line, "Invalid drawing object property."))?;
    let (name, value) = content
        .split_once(' ')
        .map(|(name, value)| (name, value.trim()))
        .unwrap_or((content, ""));
    match name {
        "on" => *visible_mut(object) = true,
        "off" => *visible_mut(object) = false,
        "loctype" => set_space(object, value, line)?,
        value if value.starts_with('g') && value[1..].chars().all(|ch| ch.is_ascii_digit()) => {
            let graph = value[1..]
                .parse()
                .map_err(|_| error(line, "Invalid world graph."))?;
            set_world(object, graph);
        }
        "color" => set_color(object, unsigned::<u32>(value, line)?),
        "linewidth" => set_width(object, number(value, line)?)?,
        "linestyle" => set_style(object, unsigned::<u8>(value, line)?),
        "fill" => parse_fill(object, name, value, line)?,
        "arrow" => parse_arrow(object, name, value, line)?,
        "rot" => text_style_mut(object, line)?.rotation = number(value, line)?,
        "font" => text_style_mut(object, line)?.font = unsigned::<u8>(value, line)?,
        "just" => text_style_mut(object, line)?.justification = unsigned::<u8>(value, line)?,
        "char" if value.starts_with("size ") => {
            text_style_mut(object, line)?.size = number(&value[5..], line)?
        }
        "def" => {
            let text = unescape_agr_string(value, line)?;
            match object {
                DrawingObject::Text { text: target, .. } => *target = text,
                _ => return Err(error(line, "Only a string can define text.")),
            }
        }
        _ if content.contains(',') => set_coordinates(object, content, line)?,
        _ => return Err(error(line, "Unsupported drawing object property.")),
    }
    Ok(())
}

fn parse_fill(
    object: &mut DrawingObject,
    name: &str,
    value: &str,
    line: usize,
) -> Result<(), Diagnostic> {
    let Some(fill) = fill_mut(object) else {
        return Err(error(line, "This object cannot have a fill."));
    };
    match (name, value.split_once(' ')) {
        ("fill", Some(("color", value))) => fill.color = unsigned::<u32>(value, line)?,
        ("fill", Some(("pattern", value))) => fill.pattern = unsigned::<u8>(value, line)?,
        _ => return Err(error(line, "Invalid fill property.")),
    }
    Ok(())
}

fn parse_arrow(
    object: &mut DrawingObject,
    name: &str,
    value: &str,
    line: usize,
) -> Result<(), Diagnostic> {
    let DrawingObject::Line { arrow, .. } = object else {
        return Err(error(line, "Only a line can have an arrow."));
    };
    match (name, value.split_once(' ')) {
        ("arrow", None) => arrow.ends = unsigned::<u8>(value, line)?,
        ("arrow", Some(("type", value))) => arrow.kind = unsigned::<u8>(value, line)?,
        ("arrow", Some(("size", value))) | ("arrow", Some(("length", value))) => {
            arrow.size = number(value, line)?
        }
        ("arrow", Some(("layout", value))) => arrow.layout = coordinates::<2>(value, line)?,
        _ => return Err(error(line, "Invalid arrow property.")),
    }
    Ok(())
}

fn set_coordinates(object: &mut DrawingObject, raw: &str, line: usize) -> Result<(), Diagnostic> {
    match object {
        DrawingObject::Text {
            coordinates: target,
            ..
        } => *target = coordinates(raw, line)?,
        DrawingObject::Line {
            coordinates: target,
            ..
        }
        | DrawingObject::Box {
            coordinates: target,
            ..
        }
        | DrawingObject::Ellipse {
            coordinates: target,
            ..
        } => *target = coordinates(raw, line)?,
    }
    Ok(())
}

fn set_space(object: &mut DrawingObject, value: &str, line: usize) -> Result<(), Diagnostic> {
    let space = match value.to_ascii_lowercase().as_str() {
        "view" => CoordinateSpace::View,
        "world" => match space_mut(object) {
            CoordinateSpace::World { graph } => CoordinateSpace::World { graph: *graph },
            CoordinateSpace::View => CoordinateSpace::World { graph: 0 },
        },
        _ => return Err(error(line, "Object coordinates must be view or world.")),
    };
    *space_mut(object) = space;
    Ok(())
}

fn set_world(object: &mut DrawingObject, graph: u32) {
    *space_mut(object) = CoordinateSpace::World { graph };
}

fn set_color(object: &mut DrawingObject, color: u32) {
    match object {
        DrawingObject::Text { style, .. } => style.color = color,
        DrawingObject::Line { stroke, .. }
        | DrawingObject::Box { stroke, .. }
        | DrawingObject::Ellipse { stroke, .. } => stroke.color = color,
    }
}

fn set_width(object: &mut DrawingObject, width: f64) -> Result<(), Diagnostic> {
    let Some(stroke) = stroke_mut(object) else {
        return Err(Diagnostic::new("Text has no line width."));
    };
    stroke.width = width;
    Ok(())
}

fn set_style(object: &mut DrawingObject, style: u8) {
    if let Some(stroke) = stroke_mut(object) {
        stroke.style = style;
    }
}

fn space_mut(object: &mut DrawingObject) -> &mut CoordinateSpace {
    match object {
        DrawingObject::Text { space, .. }
        | DrawingObject::Line { space, .. }
        | DrawingObject::Box { space, .. }
        | DrawingObject::Ellipse { space, .. } => space,
    }
}

fn stroke_mut(object: &mut DrawingObject) -> Option<&mut Stroke> {
    match object {
        DrawingObject::Text { .. } => None,
        DrawingObject::Line { stroke, .. }
        | DrawingObject::Box { stroke, .. }
        | DrawingObject::Ellipse { stroke, .. } => Some(stroke),
    }
}

fn fill_mut(object: &mut DrawingObject) -> Option<&mut Fill> {
    match object {
        DrawingObject::Box { fill, .. } | DrawingObject::Ellipse { fill, .. } => Some(fill),
        _ => None,
    }
}

fn text_style_mut(object: &mut DrawingObject, line: usize) -> Result<&mut TextStyle, Diagnostic> {
    match object {
        DrawingObject::Text { style, .. } => Ok(style),
        _ => Err(error(line, "Only a string has text styling.")),
    }
}

fn number(value: &str, line: usize) -> Result<f64, Diagnostic> {
    let value = value.trim().replace(['d', 'D'], "e");
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| error(line, "Expected a finite number."))
}

fn unsigned<T>(value: &str, line: usize) -> Result<T, Diagnostic>
where
    T: TryFrom<u64>,
{
    let value = value
        .trim()
        .parse::<u64>()
        .map_err(|_| error(line, "Expected a non-negative whole number."))?;
    T::try_from(value).map_err(|_| error(line, "Number is out of range."))
}

fn coordinates<const N: usize>(value: &str, line: usize) -> Result<[f64; N], Diagnostic> {
    let values = value
        .split(',')
        .map(|value| number(value, line))
        .collect::<Result<Vec<_>, _>>()?;
    values
        .try_into()
        .map_err(|_| error(line, format!("Expected {N} coordinates.")))
}

fn unescape_agr_string(value: &str, line: usize) -> Result<String, Diagnostic> {
    let value = value.trim();
    let value = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .ok_or_else(|| error(line, "Expected a quoted string."))?;
    let mut result = String::new();
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            if character == '\\' || character == '"' {
                result.push(character);
            } else {
                // Grace text commands (for example `\\xg`) are not string escapes.
                result.push('\\');
                result.push(character);
            }
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            result.push(character);
        }
    }
    if escaped {
        return Err(error(line, "Unterminated string escape."));
    }
    Ok(result)
}

fn error(line: usize, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(format!("Line {line}: {}", message.into()))
}

pub(crate) fn write_agr(object: &DrawingObject) -> String {
    let (prefix, visible, coordinates, space, stroke, fill, text, arrow) = match object {
        DrawingObject::Text {
            visible,
            coordinates,
            space,
            text,
            style,
        } => (
            "string",
            *visible,
            coordinates.as_slice(),
            space,
            None,
            None,
            Some((text, style)),
            None,
        ),
        DrawingObject::Line {
            visible,
            coordinates,
            space,
            stroke,
            arrow,
        } => (
            "line",
            *visible,
            coordinates.as_slice(),
            space,
            Some(stroke),
            None,
            None,
            Some(arrow),
        ),
        DrawingObject::Box {
            visible,
            coordinates,
            space,
            stroke,
            fill,
        } => (
            "box",
            *visible,
            coordinates.as_slice(),
            space,
            Some(stroke),
            Some(fill),
            None,
            None,
        ),
        DrawingObject::Ellipse {
            visible,
            coordinates,
            space,
            stroke,
            fill,
        } => (
            "ellipse",
            *visible,
            coordinates.as_slice(),
            space,
            Some(stroke),
            Some(fill),
            None,
            None,
        ),
    };
    let mut output = format!(
        "@with {prefix}\n@    {prefix} {}\n",
        if visible { "on" } else { "off" }
    );
    match space {
        CoordinateSpace::View => output.push_str(&format!("@    {prefix} loctype view\n")),
        CoordinateSpace::World { graph } => output.push_str(&format!(
            "@    {prefix} loctype world\n@    {prefix} g{graph}\n"
        )),
    }
    output.push_str(&format!(
        "@    {prefix} {}\n",
        coordinates
            .iter()
            .map(|value| format!("{value:.12}"))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    if let Some(stroke) = stroke {
        output.push_str(&format!(
            "@    {prefix} linewidth {:.12}\n@    {prefix} linestyle {}\n@    {prefix} color {}\n",
            stroke.width, stroke.style, stroke.color
        ));
    }
    if let Some(fill) = fill {
        output.push_str(&format!(
            "@    {prefix} fill color {}\n@    {prefix} fill pattern {}\n",
            fill.color, fill.pattern
        ));
    }
    if let Some(arrow) = arrow {
        output.push_str(&format!("@    line arrow {}\n@    line arrow type {}\n@    line arrow size {:.12}\n@    line arrow layout {:.12}, {:.12}\n", arrow.ends, arrow.kind, arrow.size, arrow.layout[0], arrow.layout[1]));
    }
    if let Some((text, style)) = text {
        output.push_str(&format!("@    string color {}\n@    string rot {:.12}\n@    string font {}\n@    string just {}\n@    string char size {:.12}\n@    string def \"{}\"\n", style.color, style.rotation, style.font, style.justification, style.size, escape_agr_string(text)));
    }
    output.push_str(&format!("@{prefix} def\n"));
    output
}

fn escape_agr_string(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn parses_and_writes_a_world_line() {
        let commands = [
            (1, "with line"),
            (2, "line on"),
            (3, "line loctype world"),
            (4, "line g3"),
            (5, "line 1, 2, 3, 4"),
            (6, "line linewidth 2"),
            (7, "line linestyle 3"),
            (8, "line color 4"),
            (9, "line arrow 2"),
            (10, "line arrow type 1"),
            (11, "line arrow size 1.5"),
            (12, "line def"),
        ];
        let object = parse_block(&commands).unwrap();
        assert!(matches!(
            object,
            DrawingObject::Line {
                space: CoordinateSpace::World { graph: 3 },
                ..
            }
        ));
        let text = write_agr(&object);
        assert!(text.contains("line loctype world"));
        assert!(text.contains("line g3"));
        assert_eq!(
            parse_block(
                &text
                    .lines()
                    .enumerate()
                    .map(|(index, line)| (index + 1, line.trim_start_matches('@').trim()))
                    .collect::<Vec<_>>()
            )
            .unwrap(),
            object
        );
    }

    #[test]
    fn parses_text_escapes_without_touching_grace_typesetting() {
        let object = parse_block(&[
            (1, "with string"),
            (2, "string 0.2, 0.3"),
            (3, r#"string def "A \"quoted\" \xg""#),
        ])
        .unwrap();
        let DrawingObject::Text { text, .. } = object else {
            panic!("expected text")
        };
        assert_eq!(text, "A \"quoted\" \\xg");
    }

    #[test]
    fn operations_reuse_the_lowest_identifier() {
        let mut project = Project::new();
        project.objects = BTreeMap::new();
        let object = DrawingObject::Text {
            visible: true,
            coordinates: [0., 0.],
            space: CoordinateSpace::View,
            text: "x".into(),
            style: TextStyle::default(),
        };
        apply(
            &mut project,
            ObjectOperation::Create {
                object: object.clone(),
            },
        )
        .unwrap();
        apply(&mut project, ObjectOperation::Delete { id: 0 }).unwrap();
        apply(&mut project, ObjectOperation::Create { object }).unwrap();
        assert!(project.objects.contains_key(&0));
    }
}
