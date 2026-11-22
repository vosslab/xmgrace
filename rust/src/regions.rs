//! Browser-safe Grace data regions.
//!
//! Regions select points in graph world coordinates. They deliberately contain
//! no browser state, so data transformations and the command window use the
//! same membership rules as graphical editing.

use crate::{Diagnostic, Project};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write;

/// An active region is linked to one graph, or all graphs when `graph` is `None`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct Region {
    #[serde(default = "active")]
    pub(crate) active: bool,
    #[serde(default)]
    pub(crate) graph: Option<u32>,
    #[serde(flatten)]
    pub(crate) geometry: RegionGeometry,
}

fn active() -> bool {
    true
}

/// Parses one classic AGR region directive (without its leading `@`).
///
/// `Ok(false)` means that the directive is not region syntax and must be
/// handled by the caller's normal AGR parser. Region style settings are
/// consumed because this model currently stores selection geometry only.
pub(crate) fn parse_agr(project: &mut Project, command: &str) -> Result<bool, Diagnostic> {
    let (head, tail) = split_first(command);
    if head.eq_ignore_ascii_case("link") {
        let (region, rest) =
            region_number(tail).ok_or_else(|| Diagnostic::new("Invalid region link"))?;
        let graph = rest
            .strip_prefix("to")
            .map(str::trim_start)
            .and_then(graph_number)
            .ok_or_else(|| Diagnostic::new("Invalid region link"))?;
        let entry = project.regions.entry(region).or_insert_with(default_region);
        entry.graph = Some(graph);
        return Ok(true);
    }
    let Some((id, rest)) = region_number(command) else {
        return Ok(false);
    };
    let (property, value) = split_first(rest);
    let region = project.regions.entry(id).or_insert_with(default_region);
    match property.to_ascii_lowercase().as_str() {
        "on" if value.is_empty() => region.active = true,
        "off" if value.is_empty() => region.active = false,
        "type" => region.geometry = geometry_for_type(value)?,
        "line" => set_line(region, coordinates::<4>(value)?),
        "xy" => add_polygon_point(region, coordinates::<2>(value)?),
        // These lines are emitted by Grace but are display styling rather than
        // selection semantics. Consume them so an otherwise usable project is
        // never made read-only merely for containing a region.
        "linestyle" | "linewidth" | "color" => {}
        _ => return Err(Diagnostic::new("Invalid region setting")),
    }
    Ok(true)
}

/// Encodes the project-owned region subset using classic Grace AGR grammar.
pub(crate) fn write_agr(regions: &BTreeMap<u32, Region>) -> String {
    let mut output = String::new();
    for (id, region) in regions {
        let _ = writeln!(
            output,
            "@r{id} {}",
            if region.active { "on" } else { "off" }
        );
        if let Some(graph) = region.graph {
            let _ = writeln!(output, "@link r{id} to g{graph}");
        }
        let _ = writeln!(output, "@r{id} type {}", region_type(&region.geometry));
        match &region.geometry {
            RegionGeometry::InsidePolygon { points }
            | RegionGeometry::OutsidePolygon { points } => {
                for [x, y] in points {
                    let _ = writeln!(output, "@r{id} xy {x}, {y}");
                }
            }
            RegionGeometry::AboveLine { first, second }
            | RegionGeometry::BelowLine { first, second }
            | RegionGeometry::LeftOfLine { first, second }
            | RegionGeometry::RightOfLine { first, second } => {
                write_line(&mut output, *id, *first, *second);
            }
            RegionGeometry::InHorizontalRange { start, stop }
            | RegionGeometry::OutHorizontalRange { start, stop } => {
                write_line(&mut output, *id, [*start, 0.], [*stop, 0.]);
            }
            RegionGeometry::InVerticalRange { start, stop }
            | RegionGeometry::OutVerticalRange { start, stop } => {
                write_line(&mut output, *id, [0., *start], [0., *stop]);
            }
        }
    }
    output
}

fn write_line(output: &mut String, id: u32, [x1, y1]: [f64; 2], [x2, y2]: [f64; 2]) {
    let _ = writeln!(output, "@r{id} line {x1}, {y1}, {x2}, {y2}");
}

fn default_region() -> Region {
    Region {
        active: false,
        graph: None,
        geometry: RegionGeometry::AboveLine {
            first: [0., 0.],
            second: [1., 1.],
        },
    }
}

fn split_first(input: &str) -> (&str, &str) {
    input
        .trim()
        .split_once(|ch: char| ch.is_ascii_whitespace())
        .map_or((input.trim(), ""), |(head, tail)| (head, tail.trim_start()))
}

fn region_number(input: &str) -> Option<(u32, &str)> {
    let input = input.trim_start();
    let rest = input
        .strip_prefix('r')
        .or_else(|| input.strip_prefix('R'))?;
    let digits = rest
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(rest.len());
    let id = rest[..digits].parse().ok()?;
    Some((id, rest[digits..].trim_start()))
}

fn graph_number(input: &str) -> Option<u32> {
    let rest = input.trim();
    let rest = rest.strip_prefix('g').or_else(|| rest.strip_prefix('G'))?;
    if rest.is_empty() || !rest.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    rest.parse().ok()
}

fn coordinates<const N: usize>(input: &str) -> Result<[f64; N], Diagnostic> {
    let values = input
        .split(',')
        .map(|field| {
            field
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
        })
        .collect::<Option<Vec<_>>>()
        .and_then(|values| values.try_into().ok());
    values.ok_or_else(|| Diagnostic::new("Region coordinates must be finite numbers."))
}

fn geometry_for_type(name: &str) -> Result<RegionGeometry, Diagnostic> {
    let line = || ([0., 0.], [1., 1.]);
    match name.trim().to_ascii_lowercase().as_str() {
        "above" => {
            let (first, second) = line();
            Ok(RegionGeometry::AboveLine { first, second })
        }
        "below" => {
            let (first, second) = line();
            Ok(RegionGeometry::BelowLine { first, second })
        }
        "left" => {
            let (first, second) = line();
            Ok(RegionGeometry::LeftOfLine { first, second })
        }
        "right" => {
            let (first, second) = line();
            Ok(RegionGeometry::RightOfLine { first, second })
        }
        "polyi" => Ok(RegionGeometry::InsidePolygon { points: Vec::new() }),
        "polyo" => Ok(RegionGeometry::OutsidePolygon { points: Vec::new() }),
        "horizi" => Ok(RegionGeometry::InHorizontalRange {
            start: 0.,
            stop: 0.,
        }),
        "horizo" => Ok(RegionGeometry::OutHorizontalRange {
            start: 0.,
            stop: 0.,
        }),
        "verti" => Ok(RegionGeometry::InVerticalRange {
            start: 0.,
            stop: 0.,
        }),
        "verto" => Ok(RegionGeometry::OutVerticalRange {
            start: 0.,
            stop: 0.,
        }),
        _ => Err(Diagnostic::new("Unknown Grace region type.")),
    }
}

fn region_type(geometry: &RegionGeometry) -> &'static str {
    match geometry {
        RegionGeometry::AboveLine { .. } => "above",
        RegionGeometry::BelowLine { .. } => "below",
        RegionGeometry::LeftOfLine { .. } => "left",
        RegionGeometry::RightOfLine { .. } => "right",
        RegionGeometry::InsidePolygon { .. } => "polyi",
        RegionGeometry::OutsidePolygon { .. } => "polyo",
        RegionGeometry::InHorizontalRange { .. } => "horizi",
        RegionGeometry::OutHorizontalRange { .. } => "horizo",
        RegionGeometry::InVerticalRange { .. } => "verti",
        RegionGeometry::OutVerticalRange { .. } => "verto",
    }
}

fn set_line(region: &mut Region, [x1, y1, x2, y2]: [f64; 4]) {
    match &mut region.geometry {
        RegionGeometry::AboveLine { first, second }
        | RegionGeometry::BelowLine { first, second }
        | RegionGeometry::LeftOfLine { first, second }
        | RegionGeometry::RightOfLine { first, second } => {
            *first = [x1, y1];
            *second = [x2, y2];
        }
        RegionGeometry::InHorizontalRange { start, stop }
        | RegionGeometry::OutHorizontalRange { start, stop } => (*start, *stop) = (x1, x2),
        RegionGeometry::InVerticalRange { start, stop }
        | RegionGeometry::OutVerticalRange { start, stop } => (*start, *stop) = (y1, y2),
        RegionGeometry::InsidePolygon { .. } | RegionGeometry::OutsidePolygon { .. } => {}
    }
}

fn add_polygon_point(region: &mut Region, [x, y]: [f64; 2]) {
    match &mut region.geometry {
        RegionGeometry::InsidePolygon { points } | RegionGeometry::OutsidePolygon { points } => {
            points.push([x, y]);
        }
        _ => {}
    }
}

/// Classic Grace region choices, represented by their explicit geometry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum RegionGeometry {
    InsidePolygon { points: Vec<[f64; 2]> },
    OutsidePolygon { points: Vec<[f64; 2]> },
    AboveLine { first: [f64; 2], second: [f64; 2] },
    BelowLine { first: [f64; 2], second: [f64; 2] },
    LeftOfLine { first: [f64; 2], second: [f64; 2] },
    RightOfLine { first: [f64; 2], second: [f64; 2] },
    InHorizontalRange { start: f64, stop: f64 },
    OutHorizontalRange { start: f64, stop: f64 },
    InVerticalRange { start: f64, stop: f64 },
    OutVerticalRange { start: f64, stop: f64 },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub(crate) enum RegionOperation {
    Create { region: Region },
    Replace { id: u32, region: Region },
    Delete { id: u32 },
    Visibility { id: u32, active: bool },
}

impl Region {
    /// Returns whether a finite point belongs to this active region.
    pub(crate) fn contains(&self, graph: u32, x: f64, y: f64) -> bool {
        if !self.active
            || self.graph.is_some_and(|linked| linked != graph)
            || !x.is_finite()
            || !y.is_finite()
        {
            return false;
        }
        match &self.geometry {
            RegionGeometry::InsidePolygon { points } => in_polygon(x, y, points),
            RegionGeometry::OutsidePolygon { points } => !in_polygon(x, y, points),
            RegionGeometry::AboveLine { first, second } => above(x, y, *first, *second),
            RegionGeometry::BelowLine { first, second } => below(x, y, *first, *second),
            RegionGeometry::LeftOfLine { first, second } => left(x, y, *first, *second),
            RegionGeometry::RightOfLine { first, second } => right(x, y, *first, *second),
            RegionGeometry::InHorizontalRange { start, stop } => between(x, *start, *stop),
            RegionGeometry::OutHorizontalRange { start, stop } => !between(x, *start, *stop),
            RegionGeometry::InVerticalRange { start, stop } => between(y, *start, *stop),
            RegionGeometry::OutVerticalRange { start, stop } => !between(y, *start, *stop),
        }
    }
}

/// Applies an all-or-nothing region change to the project.
pub(crate) fn apply(project: &mut Project, operation: RegionOperation) -> Result<(), Diagnostic> {
    project.require_editable()?;
    match operation {
        RegionOperation::Create { region } => {
            validate(project, &region)?;
            let id = next_id(project)?;
            project.regions.insert(id, region);
        }
        RegionOperation::Replace { id, region } => {
            validate(project, &region)?;
            if !project.regions.contains_key(&id) {
                return Err(Diagnostic::new("Select an existing region."));
            }
            project.regions.insert(id, region);
        }
        RegionOperation::Delete { id } => {
            if project.regions.remove(&id).is_none() {
                return Err(Diagnostic::new("Select an existing region."));
            }
        }
        RegionOperation::Visibility { id, active } => {
            let region = project
                .regions
                .get_mut(&id)
                .ok_or_else(|| Diagnostic::new("Select an existing region."))?;
            region.active = active;
        }
    }
    Ok(())
}

fn next_id(project: &Project) -> Result<u32, Diagnostic> {
    (0..=u32::MAX)
        .find(|id| !project.regions.contains_key(id))
        .ok_or_else(|| Diagnostic::new("This project has no available region identifiers."))
}

fn validate(project: &Project, region: &Region) -> Result<(), Diagnostic> {
    if region
        .graph
        .is_some_and(|graph| !project.graphs.contains_key(&graph))
    {
        return Err(Diagnostic::new("Link the region to an existing graph."));
    }
    match &region.geometry {
        RegionGeometry::InsidePolygon { points } | RegionGeometry::OutsidePolygon { points } => {
            if points.len() < 3 || !points.iter().flatten().all(|value| value.is_finite()) {
                return Err(Diagnostic::new(
                    "A polygon region needs at least three finite points.",
                ));
            }
        }
        RegionGeometry::AboveLine { first, second }
        | RegionGeometry::BelowLine { first, second }
        | RegionGeometry::LeftOfLine { first, second }
        | RegionGeometry::RightOfLine { first, second } => {
            if !first.iter().chain(second).all(|value| value.is_finite()) || first == second {
                return Err(Diagnostic::new(
                    "A region line needs two distinct finite points.",
                ));
            }
        }
        RegionGeometry::InHorizontalRange { start, stop }
        | RegionGeometry::OutHorizontalRange { start, stop }
        | RegionGeometry::InVerticalRange { start, stop }
        | RegionGeometry::OutVerticalRange { start, stop } => {
            if !start.is_finite() || !stop.is_finite() || start > stop {
                return Err(Diagnostic::new(
                    "Region range endpoints must be finite and in ascending order.",
                ));
            }
        }
    }
    Ok(())
}

fn between(value: f64, start: f64, stop: f64) -> bool {
    value >= start && value <= stop
}

// These predicates intentionally match Grace's coordinate-oriented definitions.
// A vertical line has no above/below side, and a horizontal line has no left/right side.
fn above(x: f64, y: f64, [x1, y1]: [f64; 2], [x2, y2]: [f64; 2]) -> bool {
    if x1 == x2 {
        return false;
    }
    y >= y1 + (y2 - y1) * (x - x1) / (x2 - x1)
}

fn below(x: f64, y: f64, [x1, y1]: [f64; 2], [x2, y2]: [f64; 2]) -> bool {
    if x1 == x2 {
        return false;
    }
    y <= y1 + (y2 - y1) * (x - x1) / (x2 - x1)
}

fn left(x: f64, y: f64, [x1, y1]: [f64; 2], [x2, y2]: [f64; 2]) -> bool {
    if y1 == y2 {
        return false;
    }
    x <= x1 + (x2 - x1) * (y - y1) / (y2 - y1)
}

fn right(x: f64, y: f64, [x1, y1]: [f64; 2], [x2, y2]: [f64; 2]) -> bool {
    if y1 == y2 {
        return false;
    }
    x >= x1 + (x2 - x1) * (y - y1) / (y2 - y1)
}

fn in_polygon(x: f64, y: f64, points: &[[f64; 2]]) -> bool {
    let mut intersections = 0;
    for index in 0..points.len() {
        let [x1, y1] = points[index];
        let [x2, y2] = points[(index + 1) % points.len()];
        if y1 == y2 || (y < y1 && y < y2) || (y > y1 && y > y2) {
            continue;
        }
        let crossing = if x1 == x2 {
            x1
        } else {
            x1 + (y - y1) * (x2 - x1) / (y2 - y1)
        };
        if crossing <= x && (y != y1 || y1 > y2) && (y != y2 || y2 > y1) {
            intersections += 1;
        }
    }
    intersections % 2 == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Project;

    fn region(geometry: RegionGeometry) -> Region {
        Region {
            active: true,
            graph: Some(0),
            geometry,
        }
    }

    #[test]
    fn respects_graph_links_and_classic_line_degeneracy() {
        let above_region = region(RegionGeometry::AboveLine {
            first: [0., 0.],
            second: [2., 2.],
        });
        assert!(above_region.contains(0, 1., 2.));
        assert!(!above_region.contains(0, 1., 0.));
        assert!(!above_region.contains(1, 1., 2.));
        let vertical = region(RegionGeometry::AboveLine {
            first: [0., 0.],
            second: [0., 2.],
        });
        assert!(!vertical.contains(0, 1., 3.));
    }

    #[test]
    fn handles_polygon_and_closed_ranges() {
        let polygon = region(RegionGeometry::InsidePolygon {
            points: vec![[0., 0.], [2., 0.], [0., 2.]],
        });
        assert!(polygon.contains(0, 0.5, 0.5));
        assert!(!polygon.contains(0, 1.5, 1.5));
        let range = region(RegionGeometry::InHorizontalRange {
            start: 1.,
            stop: 2.,
        });
        assert!(range.contains(0, 1., 30.));
        assert!(range.contains(0, 2., 30.));
    }

    #[test]
    fn replaces_only_after_validation() {
        let mut project = Project::new();
        project.regions = BTreeMap::new();
        apply(
            &mut project,
            RegionOperation::Create {
                region: region(RegionGeometry::InVerticalRange {
                    start: 0.,
                    stop: 1.,
                }),
            },
        )
        .unwrap();
        let bad = Region {
            active: true,
            graph: Some(99),
            geometry: RegionGeometry::InVerticalRange {
                start: 0.,
                stop: 1.,
            },
        };
        assert!(
            apply(
                &mut project,
                RegionOperation::Replace { id: 0, region: bad }
            )
            .is_err()
        );
        assert!(project.regions[&0].contains(0, 2., 0.5));
    }

    #[test]
    fn reads_and_writes_classic_region_directives() {
        let mut project = Project::new();
        project.regions = BTreeMap::new();
        for command in [
            "r7 on",
            "link r7 to g0",
            "r7 type polyi",
            "r7 xy 0, 0",
            "r7 xy 3, 0",
            "r7 xy 0, 3",
            "r7 color 1",
        ] {
            assert!(parse_agr(&mut project, command).unwrap());
        }
        assert!(project.regions[&7].contains(0, 0.5, 0.5));
        let output = write_agr(&project.regions);
        assert!(output.contains("@r7 type polyi\n"));
        assert!(output.contains("@link r7 to g0\n"));

        let mut reloaded = Project::new();
        reloaded.regions = BTreeMap::new();
        for line in output.lines() {
            assert!(parse_agr(&mut reloaded, line.trim_start_matches('@')).unwrap());
        }
        assert_eq!(project.regions, reloaded.regions);
    }
}
