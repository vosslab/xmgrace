//! Curve fitting and feature extraction over ordinary XY rows.
//!
//! The algorithms in this module are host independent. The Wasm adapter owns
//! command decoding and result-set allocation, while this module only returns
//! calculated rows or a diagnostic.

use crate::{Diagnostic, Graph, expression::Expression};
use serde::Deserialize;

const MAX_PARAMETERS: usize = 16;
const MAX_ITERATIONS: usize = 10_000;
const MAX_MODEL_EVALUATIONS: usize = 5_000_000;

/// Options for a Levenberg-Marquardt fit.
///
/// `formula` accepts either an expression or the classic `y = expression`
/// form. It can refer to `x` and the supplied parameters as `a0`, `a1`, ....
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FitOptions {
    pub(crate) formula: String,
    pub(crate) parameters: Vec<f64>,
    pub(crate) iterations: usize,
    pub(crate) tolerance: f64,
    #[serde(default)]
    pub(crate) lower: Option<Vec<f64>>,
    #[serde(default)]
    pub(crate) upper: Option<Vec<f64>>,
    /// The data set produced after fitting. Defaults to the fitted curve.
    #[serde(default)]
    pub(crate) output: Option<FitOutput>,
}

/// Selects the generated data set, without changing the fit report.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FitOutput {
    Fitted,
    Residuals,
}

/// Scientific results of a nonlinear least-squares fit.
#[derive(Debug)]
pub(crate) struct FitResult {
    pub(crate) rows: Vec<Vec<f64>>,
    pub(crate) parameters: Vec<f64>,
    pub(crate) sum_squared_residuals: f64,
    pub(crate) iterations: usize,
}

/// A scalar property extracted from every visible set in a graph.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Feature {
    YMinimum,
    YMaximum,
    YAverage,
    YStandardDeviation,
    YMedian,
    XMinimum,
    XMaximum,
    XAverage,
    XStandardDeviation,
    XMedian,
    Frequency,
    Period,
    ZeroCrossing,
    RiseTime,
    FallTime,
    Slope,
    YIntercept,
    SetLength,
    HalfMaximumWidth,
    BarycenterX,
    BarycenterY,
    XAtYMaximum,
    YAtXMaximum,
    Integral,
}

/// Fits `formula` to finite XY rows and returns both a result set and report.
pub(crate) fn fit(rows: &[Vec<f64>], options: FitOptions) -> Result<FitResult, Diagnostic> {
    let (xs, ys) = xy(rows)?;
    let parameter_count = options.parameters.len();
    if parameter_count == 0 || parameter_count > MAX_PARAMETERS || rows.len() < parameter_count {
        return Err(Diagnostic::new(format!(
            "Fit needs between 1 and {MAX_PARAMETERS} parameters and at least that many points."
        )));
    }
    if !(1..=MAX_ITERATIONS).contains(&options.iterations)
        || !options.tolerance.is_finite()
        || options.tolerance <= 0.
    {
        return Err(Diagnostic::new(
            "Fit iterations must be positive and tolerance must be finite and positive.",
        ));
    }
    let evaluations_per_iteration = parameter_count
        .checked_mul(2)
        .and_then(|value| value.checked_add(2))
        .ok_or_else(|| Diagnostic::new("Fit configuration is too large."))?;
    let estimated_evaluations = rows
        .len()
        .checked_mul(evaluations_per_iteration)
        .and_then(|value| value.checked_mul(options.iterations))
        .ok_or_else(|| Diagnostic::new("Fit configuration is too large."))?;
    if estimated_evaluations > MAX_MODEL_EVALUATIONS {
        return Err(Diagnostic::new(format!(
            "Fit exceeds the browser work limit of {MAX_MODEL_EVALUATIONS} model evaluations. Reduce points, parameters, or iterations."
        )));
    }
    if !options.parameters.iter().all(|value| value.is_finite()) {
        return Err(Diagnostic::new("Fit parameters must be finite."));
    }
    let (lower, upper) = bounds(&options, parameter_count)?;
    let formula = formula(&options.formula)?;
    let variables: Vec<String> = std::iter::once("x".to_string())
        .chain((0..parameter_count).map(|index| format!("a{index}")))
        .collect();
    let variable_names: Vec<&str> = variables.iter().map(String::as_str).collect();
    let expression = Expression::parse(formula, &variable_names)?;

    let output = options.output.unwrap_or(FitOutput::Fitted);
    let mut parameters = options.parameters;
    clamp(&mut parameters, &lower, &upper);
    let mut lambda = 1e-3;
    let mut current = residuals(&expression, &xs, &ys, &parameters)?;
    let mut current_error = squared_norm(&current);
    let residual_limit = options.tolerance.powi(2) * rows.len() as f64;
    let mut converged = current_error <= residual_limit;

    let mut iterations = 0;
    for _ in 0..options.iterations {
        if converged {
            break;
        }
        iterations += 1;
        let jacobian = jacobian(&expression, &xs, &parameters)?;
        let mut normal = vec![vec![0.; parameter_count]; parameter_count];
        let mut gradient = vec![0.; parameter_count];
        for (row, residual) in jacobian.iter().zip(&current) {
            for column in 0..parameter_count {
                gradient[column] += row[column] * residual;
                for other in 0..parameter_count {
                    normal[column][other] += row[column] * row[other];
                }
            }
        }
        for (index, diagonal) in normal.iter_mut().enumerate() {
            diagonal[index] += lambda * diagonal[index].max(1.);
        }
        let Some(delta) = solve(normal, gradient) else {
            return Err(Diagnostic::new(
                "Fit could not solve the model Jacobian. Choose different parameters or data.",
            ));
        };
        let mut proposed: Vec<f64> = parameters
            .iter()
            .zip(&delta)
            .map(|(parameter, change)| parameter + change)
            .collect();
        clamp(&mut proposed, &lower, &upper);
        let proposed_residuals = residuals(&expression, &xs, &ys, &proposed)?;
        let proposed_error = squared_norm(&proposed_residuals);
        if proposed_error < current_error {
            let movement = squared_norm(&delta).sqrt();
            let scale = 1. + squared_norm(&parameters).sqrt();
            parameters = proposed;
            current = proposed_residuals;
            let improvement = current_error - proposed_error;
            current_error = proposed_error;
            lambda = (lambda * 0.3).max(1e-15);
            if movement <= options.tolerance * scale
                || improvement <= options.tolerance * (1. + current_error)
            {
                converged = true;
                break;
            }
        } else {
            lambda = (lambda * 10.).min(1e15);
        }
    }
    if !converged {
        return Err(Diagnostic::new(format!(
            "Fit did not converge within {} iterations. Adjust the initial parameters, tolerance, or model.",
            options.iterations
        )));
    }

    let rows = match output {
        FitOutput::Fitted => xs
            .iter()
            .map(|x| {
                let y = model(&expression, *x, &parameters)?;
                Ok(vec![*x, y])
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?,
        FitOutput::Residuals => xs
            .iter()
            .zip(&current)
            .map(|(x, residual)| vec![*x, *residual])
            .collect(),
    };
    Ok(FitResult {
        rows,
        parameters,
        sum_squared_residuals: current_error,
        iterations,
    })
}

/// Produces one point per visible set. The X coordinate is the set's ordinal
/// position, matching the classic "Index" abscissa choice.
pub(crate) fn extract(graph: &Graph, feature: Feature) -> Result<Vec<Vec<f64>>, Diagnostic> {
    graph
        .sets
        .values()
        .filter(|set| {
            set.settings
                .get("hidden")
                .is_none_or(|hidden| hidden != "true")
        })
        .enumerate()
        .map(|(index, set)| Ok(vec![index as f64, measure(&set.rows, &feature)?]))
        .collect()
}

fn formula(input: &str) -> Result<&str, Diagnostic> {
    let input = input.trim();
    let expression = input
        .strip_prefix("y =")
        .or_else(|| input.strip_prefix("Y ="))
        .unwrap_or(input)
        .trim();
    if expression.is_empty() {
        return Err(Diagnostic::new("Enter a fit formula."));
    }
    Ok(expression)
}

fn bounds(options: &FitOptions, count: usize) -> Result<(Vec<f64>, Vec<f64>), Diagnostic> {
    let lower = options
        .lower
        .clone()
        .unwrap_or_else(|| vec![f64::NEG_INFINITY; count]);
    let upper = options
        .upper
        .clone()
        .unwrap_or_else(|| vec![f64::INFINITY; count]);
    if lower.len() != count
        || upper.len() != count
        || lower
            .iter()
            .zip(&upper)
            .any(|(low, high)| low.is_nan() || high.is_nan() || low > high)
    {
        return Err(Diagnostic::new(
            "Fit bounds must have one ordered pair for every parameter.",
        ));
    }
    Ok((lower, upper))
}

fn clamp(parameters: &mut [f64], lower: &[f64], upper: &[f64]) {
    for ((parameter, low), high) in parameters.iter_mut().zip(lower).zip(upper) {
        *parameter = parameter.clamp(*low, *high);
    }
}

fn residuals(
    expression: &Expression,
    xs: &[f64],
    ys: &[f64],
    parameters: &[f64],
) -> Result<Vec<f64>, Diagnostic> {
    xs.iter()
        .zip(ys)
        .map(|(x, y)| Ok(y - model(expression, *x, parameters)?))
        .collect()
}

fn model(expression: &Expression, x: f64, parameters: &[f64]) -> Result<f64, Diagnostic> {
    let values: Vec<f64> = std::iter::once(x)
        .chain(parameters.iter().copied())
        .collect();
    expression.evaluate(&values)
}

fn jacobian(
    expression: &Expression,
    xs: &[f64],
    parameters: &[f64],
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    xs.iter()
        .map(|x| {
            (0..parameters.len())
                .map(|column| {
                    let step = 1e-6 * parameters[column].abs().max(1.);
                    let mut plus = parameters.to_vec();
                    let mut minus = parameters.to_vec();
                    plus[column] += step;
                    minus[column] -= step;
                    Ok(
                        (model(expression, *x, &plus)? - model(expression, *x, &minus)?)
                            / (2. * step),
                    )
                })
                .collect()
        })
        .collect()
}

fn solve(mut matrix: Vec<Vec<f64>>, mut rhs: Vec<f64>) -> Option<Vec<f64>> {
    for column in 0..rhs.len() {
        let pivot = (column..rhs.len()).max_by(|left, right| {
            matrix[*left][column]
                .abs()
                .total_cmp(&matrix[*right][column].abs())
        })?;
        if matrix[pivot][column].abs() <= f64::EPSILON {
            return None;
        }
        matrix.swap(column, pivot);
        rhs.swap(column, pivot);
        let divisor = matrix[column][column];
        for value in &mut matrix[column][column..] {
            *value /= divisor;
        }
        rhs[column] /= divisor;
        for row in 0..rhs.len() {
            if row == column {
                continue;
            }
            let factor = matrix[row][column];
            for entry in column..rhs.len() {
                matrix[row][entry] -= factor * matrix[column][entry];
            }
            rhs[row] -= factor * rhs[column];
        }
    }
    Some(rhs)
}

fn squared_norm(values: &[f64]) -> f64 {
    values.iter().map(|value| value * value).sum()
}

fn xy(rows: &[Vec<f64>]) -> Result<(Vec<f64>, Vec<f64>), Diagnostic> {
    if rows.len() < 2 {
        return Err(Diagnostic::new("Choose a set with at least two XY points."));
    }
    let mut xs = Vec::with_capacity(rows.len());
    let mut ys = Vec::with_capacity(rows.len());
    for row in rows {
        let Some((&x, &y)) = row.first().zip(row.get(1)) else {
            return Err(Diagnostic::new("Choose an XY data set."));
        };
        if !x.is_finite() || !y.is_finite() {
            return Err(Diagnostic::new("Fit data must be finite."));
        }
        xs.push(x);
        ys.push(y);
    }
    Ok((xs, ys))
}

fn measure(rows: &[Vec<f64>], feature: &Feature) -> Result<f64, Diagnostic> {
    let (xs, ys) = xy(rows)?;
    match feature {
        Feature::YMinimum => minimum(&ys),
        Feature::YMaximum => maximum(&ys),
        Feature::YAverage => Ok(mean(&ys)),
        Feature::YStandardDeviation => standard_deviation(&ys),
        Feature::YMedian => Ok(median(ys)),
        Feature::XMinimum => minimum(&xs),
        Feature::XMaximum => maximum(&xs),
        Feature::XAverage => Ok(mean(&xs)),
        Feature::XStandardDeviation => standard_deviation(&xs),
        Feature::XMedian => Ok(median(xs)),
        Feature::Frequency => dominant_frequency(&xs, &ys),
        Feature::Period => dominant_frequency(&xs, &ys).map(|frequency| 1. / frequency),
        Feature::ZeroCrossing => zero_crossing(&xs, &ys),
        Feature::RiseTime => transition_time(&xs, &ys, true),
        Feature::FallTime => transition_time(&xs, &ys, false),
        Feature::Slope => linear_regression(&xs, &ys).map(|(slope, _)| slope),
        Feature::YIntercept => linear_regression(&xs, &ys).map(|(_, intercept)| intercept),
        Feature::SetLength => Ok(rows.len() as f64),
        Feature::HalfMaximumWidth => half_maximum_width(&xs, &ys),
        Feature::BarycenterX => barycenter(&xs, &ys),
        Feature::BarycenterY => barycenter(&ys, &xs),
        Feature::XAtYMaximum => max_position(&ys, &xs),
        Feature::YAtXMaximum => max_position(&xs, &ys),
        Feature::Integral => Ok(trapezoid(&xs, &ys)),
    }
}

fn minimum(values: &[f64]) -> Result<f64, Diagnostic> {
    values
        .iter()
        .copied()
        .reduce(f64::min)
        .ok_or_else(|| Diagnostic::new("Feature extraction needs data."))
}

fn maximum(values: &[f64]) -> Result<f64, Diagnostic> {
    values
        .iter()
        .copied()
        .reduce(f64::max)
        .ok_or_else(|| Diagnostic::new("Feature extraction needs data."))
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn standard_deviation(values: &[f64]) -> Result<f64, Diagnostic> {
    if values.len() < 2 {
        return Err(Diagnostic::new(
            "Standard deviation needs at least two points.",
        ));
    }
    let average = mean(values);
    Ok((values
        .iter()
        .map(|value| (value - average).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64)
        .sqrt())
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.
    } else {
        values[middle]
    }
}

fn dominant_frequency(xs: &[f64], ys: &[f64]) -> Result<f64, Diagnostic> {
    let period = xs.last().expect("nonempty") - xs[0];
    if period <= 0. || !period.is_finite() {
        return Err(Diagnostic::new("Frequency needs an increasing X range."));
    }
    if ys.len() > 20_000 {
        return Err(Diagnostic::new(
            "Frequency extraction is limited to 20,000 points.",
        ));
    }
    let mut peak = (f64::NEG_INFINITY, 0usize);
    for frequency in 1..ys.len() / 2 {
        let mut real = 0.;
        let mut imaginary = 0.;
        for (index, value) in ys.iter().enumerate() {
            let angle = std::f64::consts::TAU * frequency as f64 * index as f64 / ys.len() as f64;
            real += value * angle.cos();
            imaginary -= value * angle.sin();
        }
        let power = real.mul_add(real, imaginary * imaginary);
        if power > peak.0 {
            peak = (power, frequency);
        }
    }
    if peak.1 == 0 {
        return Err(Diagnostic::new(
            "Frequency extraction needs at least three points.",
        ));
    }
    Ok(peak.1 as f64 / period)
}

fn zero_crossing(xs: &[f64], ys: &[f64]) -> Result<f64, Diagnostic> {
    for index in 0..ys.len() - 1 {
        if ys[index] == 0. {
            return Ok(xs[index]);
        }
        if ys[index] * ys[index + 1] < 0. {
            return interpolate(ys[index], xs[index], ys[index + 1], xs[index + 1], 0.);
        }
    }
    Err(Diagnostic::new("The set has no zero crossing."))
}

fn transition_time(xs: &[f64], ys: &[f64], rising: bool) -> Result<f64, Diagnostic> {
    let low = minimum(ys)?;
    let high = maximum(ys)?;
    let first = if rising {
        low + 0.1 * (high - low)
    } else {
        low + 0.9 * (high - low)
    };
    let second = if rising {
        low + 0.9 * (high - low)
    } else {
        low + 0.1 * (high - low)
    };
    let first_x = crossing(xs, ys, first, rising)?;
    let second_x = crossing_after(xs, ys, second, rising, first_x)?;
    Ok((second_x - first_x).abs())
}

fn crossing(xs: &[f64], ys: &[f64], target: f64, rising: bool) -> Result<f64, Diagnostic> {
    crossing_after(xs, ys, target, rising, f64::NEG_INFINITY)
}

fn crossing_after(
    xs: &[f64],
    ys: &[f64],
    target: f64,
    rising: bool,
    after: f64,
) -> Result<f64, Diagnostic> {
    for index in 0..ys.len() - 1 {
        let crosses = if rising {
            ys[index] <= target && ys[index + 1] >= target
        } else {
            ys[index] >= target && ys[index + 1] <= target
        };
        if crosses {
            let x = interpolate(ys[index], xs[index], ys[index + 1], xs[index + 1], target)?;
            if x >= after {
                return Ok(x);
            }
        }
    }
    Err(Diagnostic::new(
        "The set does not cross the requested feature level.",
    ))
}

fn half_maximum_width(xs: &[f64], ys: &[f64]) -> Result<f64, Diagnostic> {
    let low = minimum(ys)?;
    let high = maximum(ys)?;
    let target = (low + high) / 2.;
    let peak = ys
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(index, _)| index)
        .expect("nonempty");
    let left = (0..peak)
        .rev()
        .find_map(|index| {
            (ys[index] <= target && ys[index + 1] >= target)
                .then(|| interpolate(ys[index], xs[index], ys[index + 1], xs[index + 1], target))
        })
        .transpose()?
        .ok_or_else(|| Diagnostic::new("The peak has no left half-maximum crossing."))?;
    let right = (peak..ys.len() - 1)
        .find_map(|index| {
            (ys[index] >= target && ys[index + 1] <= target)
                .then(|| interpolate(ys[index], xs[index], ys[index + 1], xs[index + 1], target))
        })
        .transpose()?
        .ok_or_else(|| Diagnostic::new("The peak has no right half-maximum crossing."))?;
    Ok((right - left).abs())
}

fn barycenter(weights: &[f64], values: &[f64]) -> Result<f64, Diagnostic> {
    let total: f64 = weights.iter().sum();
    if total == 0. {
        return Err(Diagnostic::new("Barycenter needs nonzero total weight."));
    }
    Ok(weights
        .iter()
        .zip(values)
        .map(|(weight, value)| weight * value)
        .sum::<f64>()
        / total)
}

fn max_position(reference: &[f64], values: &[f64]) -> Result<f64, Diagnostic> {
    let index = reference
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(index, _)| index)
        .ok_or_else(|| Diagnostic::new("Feature extraction needs data."))?;
    Ok(values[index])
}

fn linear_regression(xs: &[f64], ys: &[f64]) -> Result<(f64, f64), Diagnostic> {
    if xs.len() < 4 {
        return Err(Diagnostic::new(
            "Linear features need at least four points.",
        ));
    }
    let x_average = mean(xs);
    let y_average = mean(ys);
    let denominator: f64 = xs.iter().map(|x| (x - x_average).powi(2)).sum();
    if denominator == 0. {
        return Err(Diagnostic::new("Linear features need distinct X values."));
    }
    let numerator: f64 = xs
        .iter()
        .zip(ys)
        .map(|(x, y)| (x - x_average) * (y - y_average))
        .sum();
    let slope = numerator / denominator;
    Ok((slope, y_average - slope * x_average))
}

fn trapezoid(xs: &[f64], ys: &[f64]) -> f64 {
    xs.windows(2)
        .zip(ys.windows(2))
        .map(|(x, y)| (x[1] - x[0]) * (y[0] + y[1]) / 2.)
        .sum()
}

fn interpolate(x1: f64, y1: f64, x2: f64, y2: f64, at: f64) -> Result<f64, Diagnostic> {
    if x1 == x2 {
        return Err(Diagnostic::new(
            "Feature extraction cannot interpolate equal values.",
        ));
    }
    Ok(y1 + (at - x1) * (y2 - y1) / (x2 - x1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Set;
    use std::collections::BTreeMap;

    #[test]
    fn extracts_classic_y_maxima_by_set_index() {
        let graph = Graph {
            sets: BTreeMap::from([
                (
                    0,
                    Set {
                        rows: vec![vec![0., 1.], vec![1., 3.]],
                        count: 2,
                        ..Set::default()
                    },
                ),
                (
                    3,
                    Set {
                        rows: vec![vec![0., 2.], vec![1., 4.]],
                        count: 2,
                        ..Set::default()
                    },
                ),
            ]),
            ..Graph::default()
        };
        assert_eq!(
            extract(&graph, Feature::YMaximum).unwrap(),
            vec![vec![0., 3.], vec![1., 4.]]
        );
    }

    #[test]
    fn calculates_half_maximum_width() {
        let rows = vec![vec![0., 0.], vec![1., 2.], vec![2., 2.], vec![3., 0.]];
        assert_eq!(measure(&rows, &Feature::HalfMaximumWidth).unwrap(), 2.);
    }

    #[test]
    fn fits_a_line_from_initial_guesses() {
        let rows = vec![vec![0., 1.], vec![1., 3.], vec![2., 5.], vec![3., 7.]];
        let result = fit(
            &rows,
            FitOptions {
                formula: "a0 + a1 * x".into(),
                parameters: vec![0., 0.],
                iterations: 100,
                tolerance: 1e-8,
                lower: None,
                upper: None,
                output: None,
            },
        )
        .unwrap();
        assert!((result.rows[3][1] - 7.).abs() < 1e-7);
        assert!((result.parameters[0] - 1.).abs() < 1e-7);
        assert!((result.parameters[1] - 2.).abs() < 1e-7);
        assert!(result.sum_squared_residuals < 1e-12);
        assert!(result.iterations > 0);
    }

    #[test]
    fn reports_exact_initial_fit_and_can_emit_residuals() {
        let rows = vec![vec![0., 1.], vec![1., 3.], vec![2., 5.]];
        let result = fit(
            &rows,
            FitOptions {
                formula: "a0 + a1 * x".into(),
                parameters: vec![1., 2.],
                iterations: 10,
                tolerance: 1e-8,
                lower: None,
                upper: None,
                output: Some(FitOutput::Residuals),
            },
        )
        .unwrap();
        assert_eq!(result.iterations, 0);
        assert_eq!(result.sum_squared_residuals, 0.);
        assert_eq!(result.rows, vec![vec![0., 0.], vec![1., 0.], vec![2., 0.]]);
    }

    #[test]
    fn rejects_a_fit_that_exceeds_the_worker_budget() {
        let rows: Vec<Vec<f64>> = (0..100)
            .map(|index| vec![index as f64, index as f64])
            .collect();
        let error = fit(
            &rows,
            FitOptions {
                formula: "a0 * x".into(),
                parameters: vec![1.; MAX_PARAMETERS],
                iterations: MAX_ITERATIONS,
                tolerance: 1e-8,
                lower: None,
                upper: None,
                output: None,
            },
        )
        .unwrap_err();
        assert!(error.message.contains("work limit"));
    }
}
