//! Host-independent implementations of the remaining classic data transforms.
//!
//! Each transform returns a new XY data table. The project layer chooses the
//! destination set and applies the result only after this module succeeds.

use crate::{Diagnostic, expression::Expression};
use serde::Deserialize;

const MAX_PAIRWISE_OPERATIONS: usize = 10_000_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(
    tag = "method",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum AdvancedOperation {
    Correlation {
        maximum_lag: usize,
        covariance: bool,
        unbiased: bool,
    },
    DigitalFilter,
    Convolution,
    Geometric {
        angle: f64,
        origin: [f64; 2],
        scale: [f64; 2],
        translation: [f64; 2],
        order: Option<GeometricOrder>,
    },
    Sampling {
        start: usize,
        step: usize,
        condition: Option<String>,
    },
    Pruning {
        tolerance: f64,
        kind: Option<PruningKind>,
        delta_x: Option<f64>,
        delta_y: Option<f64>,
    },
}

/// The six order choices presented by classic Grace's geometry dialog.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GeometricOrder {
    RotateTranslateScale,
    RotateScaleTranslate,
    TranslateScaleRotate,
    TranslateRotateScale,
    ScaleTranslateRotate,
    ScaleRotateTranslate,
}

/// The four classic Grace pruning strategies in world coordinates.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PruningKind {
    Interpolation,
    Circle,
    Ellipse,
    Rectangle,
}

pub(crate) fn transform(
    rows: &[Vec<f64>],
    secondary: Option<&[Vec<f64>]>,
    operation: AdvancedOperation,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    validate_rows(rows)?;
    match operation {
        AdvancedOperation::Correlation {
            maximum_lag,
            covariance,
            unbiased,
        } => {
            let other = secondary.unwrap_or(rows);
            correlation(rows, other, maximum_lag, covariance, unbiased)
        }
        AdvancedOperation::DigitalFilter => {
            let weights =
                secondary.ok_or_else(|| Diagnostic::new("Select a set of filter weights."))?;
            digital_filter(rows, weights)
        }
        AdvancedOperation::Convolution => {
            let other = secondary.ok_or_else(|| Diagnostic::new("Select a set to convolve."))?;
            convolution(rows, other)
        }
        AdvancedOperation::Geometric {
            angle,
            origin,
            scale,
            translation,
            order,
        } => geometric(rows, angle, origin, scale, translation, order),
        AdvancedOperation::Sampling {
            start,
            step,
            condition,
        } => sampling(rows, start, step, condition.as_deref()),
        AdvancedOperation::Pruning {
            tolerance,
            kind,
            delta_x,
            delta_y,
        } => pruning(rows, tolerance, kind, delta_x, delta_y),
    }
}

fn validate_rows(rows: &[Vec<f64>]) -> Result<(), Diagnostic> {
    if rows.len() < 2 {
        return Err(Diagnostic::new(
            "A transformation needs at least two points.",
        ));
    }
    if rows
        .iter()
        .any(|row| row.len() < 2 || !row.iter().all(|value| value.is_finite()))
    {
        return Err(Diagnostic::new(
            "Data rows must contain finite X and Y values.",
        ));
    }
    Ok(())
}

fn correlation(
    rows: &[Vec<f64>],
    other: &[Vec<f64>],
    maximum_lag: usize,
    covariance: bool,
    unbiased: bool,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    validate_rows(other)?;
    if rows.len() != other.len() {
        return Err(Diagnostic::new(
            "Correlation sets must have the same length.",
        ));
    }
    if !(1..=rows.len()).contains(&maximum_lag) {
        return Err(Diagnostic::new(
            "Maximum lag must be between one and the set length.",
        ));
    }
    if rows.len().saturating_mul(maximum_lag) > MAX_PAIRWISE_OPERATIONS {
        return Err(Diagnostic::new(
            "Correlation request is too large for the browser calculation limit.",
        ));
    }
    let (left_mean, right_mean) = if covariance {
        (mean_y(rows), mean_y(other))
    } else {
        (0., 0.)
    };
    let mut result = Vec::with_capacity(maximum_lag);
    let mut zero_lag = 0.;
    for lag in 0..maximum_lag {
        let sum: f64 = (0..rows.len() - lag)
            .map(|index| (other[index][1] - right_mean) * (rows[index + lag][1] - left_mean))
            .sum();
        let scale = if unbiased {
            (rows.len() - lag) as f64
        } else {
            1.
        };
        let value = sum / scale;
        if lag == 0 {
            zero_lag = value.abs();
        }
        result.push(vec![lag as f64, value]);
    }
    if !unbiased && zero_lag != 0. {
        for row in &mut result {
            row[1] /= zero_lag;
        }
    }
    if !result.iter().all(|row| row[1].is_finite()) {
        return Err(Diagnostic::new("Correlation produced a non-finite value."));
    }
    Ok(result)
}

fn digital_filter(rows: &[Vec<f64>], weights: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, Diagnostic> {
    validate_rows(weights)?;
    if weights.len() > rows.len() {
        return Err(Diagnostic::new(
            "The filter weight set cannot be longer than the filtered set.",
        ));
    }
    if rows.len().saturating_mul(weights.len()) > MAX_PAIRWISE_OPERATIONS {
        return Err(Diagnostic::new(
            "Digital filter request is too large for the browser calculation limit.",
        ));
    }
    let length = weights.len();
    let mut result = Vec::with_capacity(rows.len() - length + 1);
    for first in 0..=rows.len() - length {
        let y = weights
            .iter()
            .zip(&rows[first..first + length])
            .map(|(weight, row)| weight[1] * row[1])
            .sum();
        let x = if length % 2 == 1 {
            rows[first + length / 2][0]
        } else {
            (rows[first + length / 2 - 1][0] + rows[first + length / 2][0]) / 2.
        };
        result.push(vec![x, y]);
    }
    Ok(result)
}

fn convolution(rows: &[Vec<f64>], other: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, Diagnostic> {
    validate_rows(other)?;
    if rows.len().saturating_mul(other.len()) > MAX_PAIRWISE_OPERATIONS {
        return Err(Diagnostic::new(
            "Convolution request is too large for the browser calculation limit.",
        ));
    }
    let mut result = vec![vec![0., 0.]; rows.len() + other.len() - 1];
    for (left, row) in rows.iter().enumerate() {
        for (right, weight) in other.iter().enumerate() {
            result[left + right][1] += row[1] * weight[1];
        }
    }
    for (index, row) in result.iter_mut().enumerate() {
        row[0] = index as f64;
    }
    Ok(result)
}

fn geometric(
    rows: &[Vec<f64>],
    angle: f64,
    origin: [f64; 2],
    scale: [f64; 2],
    translation: [f64; 2],
    order: Option<GeometricOrder>,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    if !angle.is_finite()
        || !origin.iter().all(|value| value.is_finite())
        || !scale.iter().all(|value| value.is_finite())
        || !translation.iter().all(|value| value.is_finite())
    {
        return Err(Diagnostic::new(
            "Geometric transform values must be finite.",
        ));
    }
    let order = order.unwrap_or(GeometricOrder::RotateTranslateScale);
    let sine = angle.to_radians().sin();
    let cosine = angle.to_radians().cos();
    let steps = match order {
        GeometricOrder::RotateTranslateScale => [0, 1, 2],
        GeometricOrder::RotateScaleTranslate => [0, 2, 1],
        GeometricOrder::TranslateScaleRotate => [1, 2, 0],
        GeometricOrder::TranslateRotateScale => [1, 0, 2],
        GeometricOrder::ScaleTranslateRotate => [2, 1, 0],
        GeometricOrder::ScaleRotateTranslate => [2, 0, 1],
    };
    rows.iter()
        .map(|row| {
            let (mut x, mut y) = (row[0], row[1]);
            for step in steps {
                match step {
                    0 => {
                        let relative_x = x - origin[0];
                        let relative_y = y - origin[1];
                        x = origin[0] + cosine * relative_x - sine * relative_y;
                        y = origin[1] + sine * relative_x + cosine * relative_y;
                    }
                    1 => {
                        x += translation[0];
                        y += translation[1];
                    }
                    2 => {
                        x *= scale[0];
                        y *= scale[1];
                    }
                    _ => unreachable!(),
                }
            }
            let mut transformed = row.clone();
            transformed[0] = x;
            transformed[1] = y;
            if transformed.iter().all(|value| value.is_finite()) {
                Ok(transformed)
            } else {
                Err(Diagnostic::new(
                    "Geometric transform produced a non-finite point.",
                ))
            }
        })
        .collect()
}

fn sampling(
    rows: &[Vec<f64>],
    start: usize,
    step: usize,
    condition: Option<&str>,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    if step == 0 {
        return Err(Diagnostic::new("Sampling step must be at least one."));
    }
    if start >= rows.len() {
        return Err(Diagnostic::new(
            "Sampling start must select a point in the set.",
        ));
    }
    let condition = condition
        .map(|source| Expression::parse(source, &["x", "y", "i"]))
        .transpose()?;
    rows.iter()
        .enumerate()
        .skip(start)
        .step_by(step)
        .filter_map(|(index, row)| {
            let accepted = condition
                .as_ref()
                .map(|formula| formula.evaluate(&[row[0], row[1], index as f64]))
                .transpose();
            match accepted {
                Ok(Some(value)) => (value.round() != 0.).then(|| Ok(row.clone())),
                Ok(None) => Some(Ok(row.clone())),
                Err(error) => Some(Err(error)),
            }
        })
        .collect()
}

fn pruning(
    rows: &[Vec<f64>],
    tolerance: f64,
    kind: Option<PruningKind>,
    delta_x: Option<f64>,
    delta_y: Option<f64>,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    if !tolerance.is_finite() || tolerance < 0. {
        return Err(Diagnostic::new(
            "Pruning tolerance must be a non-negative finite value.",
        ));
    }
    let kind = kind.unwrap_or(PruningKind::Interpolation);
    if matches!(kind, PruningKind::Interpolation) {
        return interpolation_pruning(rows, tolerance);
    }
    let delta_x = delta_x.unwrap_or(tolerance).abs();
    let delta_y = delta_y.unwrap_or(tolerance).abs();
    if delta_x == 0. || delta_y == 0. || !delta_x.is_finite() || !delta_y.is_finite() {
        return Err(Diagnostic::new(
            "Pruning deltas must be positive finite values.",
        ));
    }
    let mut retained = Vec::with_capacity(rows.len());
    retained.push(rows[0].clone());
    for row in &rows[1..] {
        let close = retained.iter().any(|existing| match kind {
            PruningKind::Circle => {
                let x = (row[0] - existing[0]) / delta_x;
                let y = (row[1] - existing[1]) / delta_x;
                x * x + y * y < 1.
            }
            PruningKind::Ellipse => {
                let x = (row[0] - existing[0]) / delta_x;
                let y = (row[1] - existing[1]) / delta_y;
                x * x + y * y < 1.
            }
            PruningKind::Rectangle => {
                (row[0] - existing[0]).abs() < delta_x && (row[1] - existing[1]).abs() < delta_y
            }
            PruningKind::Interpolation => unreachable!(),
        });
        if !close {
            retained.push(row.clone());
        }
    }
    Ok(retained)
}

fn interpolation_pruning(rows: &[Vec<f64>], tolerance: f64) -> Result<Vec<Vec<f64>>, Diagnostic> {
    let mut keep = vec![false; rows.len()];
    keep[0] = true;
    keep[rows.len() - 1] = true;
    mark_significant(rows, 0, rows.len() - 1, tolerance, &mut keep);
    Ok(rows
        .iter()
        .zip(keep)
        .filter(|(_, retained)| *retained)
        .map(|(row, _)| row.clone())
        .collect())
}

fn mark_significant(
    rows: &[Vec<f64>],
    first: usize,
    last: usize,
    tolerance: f64,
    keep: &mut [bool],
) {
    if last <= first + 1 {
        return;
    }
    let (start, end) = (&rows[first], &rows[last]);
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length = (dx * dx + dy * dy).sqrt();
    let (index, distance) = (first + 1..last)
        .map(|index| {
            let point = &rows[index];
            let distance = if length == 0. {
                (point[0] - start[0]).hypot(point[1] - start[1])
            } else {
                ((point[0] - start[0]) * dy - (point[1] - start[1]) * dx).abs() / length
            };
            (index, distance)
        })
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .expect("range has at least one point");
    if distance > tolerance {
        keep[index] = true;
        mark_significant(rows, first, index, tolerance, keep);
        mark_significant(rows, index, last, tolerance, keep);
    }
}

fn mean_y(rows: &[Vec<f64>]) -> f64 {
    rows.iter().map(|row| row[1]).sum::<f64>() / rows.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<Vec<f64>> {
        vec![vec![0., 1.], vec![1., 2.], vec![2., 3.], vec![3., 4.]]
    }

    #[test]
    fn filter_and_convolution_match_the_classic_indexing() {
        let weights = vec![vec![0., 1.], vec![1., 2.], vec![2., 1.]];
        assert_eq!(
            transform(&rows(), Some(&weights), AdvancedOperation::DigitalFilter).unwrap(),
            vec![vec![1., 8.], vec![2., 12.]]
        );
        assert_eq!(
            transform(
                &rows()[..2],
                Some(&weights[..2]),
                AdvancedOperation::Convolution
            )
            .unwrap(),
            vec![vec![0., 1.], vec![1., 4.], vec![2., 4.]]
        );
    }

    #[test]
    fn sampling_and_geometry_preserve_extra_columns() {
        let rows = vec![vec![1., 0., 7.], vec![2., 0., 8.], vec![3., 0., 9.]];
        assert_eq!(
            transform(
                &rows,
                None,
                AdvancedOperation::Sampling {
                    start: 0,
                    step: 2,
                    condition: None,
                }
            )
            .unwrap(),
            vec![vec![1., 0., 7.], vec![3., 0., 9.]]
        );
        let result = transform(
            &rows,
            None,
            AdvancedOperation::Geometric {
                angle: 90.,
                origin: [0., 0.],
                scale: [1., 1.],
                translation: [0., 0.],
                order: None,
            },
        )
        .unwrap();
        assert!(result[0][0].abs() < 1e-12);
        assert_eq!(result[0][1], 1.);
        assert_eq!(result[0][2], 7.);
    }

    #[test]
    fn pruning_removes_collinear_points() {
        let rows = vec![vec![0., 0.], vec![1., 0.], vec![2., 0.], vec![3., 1.]];
        assert_eq!(
            transform(
                &rows,
                None,
                AdvancedOperation::Pruning {
                    tolerance: 0.01,
                    kind: None,
                    delta_x: None,
                    delta_y: None,
                },
            )
            .unwrap(),
            vec![vec![0., 0.], vec![2., 0.], vec![3., 1.]]
        );
    }

    #[test]
    fn sampling_expression_uses_classic_rounded_truthiness() {
        assert_eq!(
            transform(
                &rows(),
                None,
                AdvancedOperation::Sampling {
                    start: 0,
                    step: 1,
                    condition: Some("i % 2".into()),
                },
            )
            .unwrap(),
            vec![vec![1., 2.], vec![3., 4.]]
        );
    }

    #[test]
    fn rectangle_pruning_uses_classic_delta_window() {
        let rows = vec![vec![0., 0.], vec![0.5, 0.5], vec![2., 0.]];
        assert_eq!(
            transform(
                &rows,
                None,
                AdvancedOperation::Pruning {
                    tolerance: 1.,
                    kind: Some(PruningKind::Rectangle),
                    delta_x: Some(1.),
                    delta_y: Some(1.),
                },
            )
            .unwrap(),
            vec![vec![0., 0.], vec![2., 0.]]
        );
    }

    #[test]
    fn classic_correlation_default_is_normalized_at_zero_lag() {
        let result = transform(
            &rows(),
            None,
            AdvancedOperation::Correlation {
                maximum_lag: 2,
                covariance: false,
                unbiased: false,
            },
        )
        .unwrap();
        assert_eq!(result, vec![vec![0., 1.], vec![1., 20. / 30.]]);
    }
}
