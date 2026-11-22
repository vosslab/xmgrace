//! Deterministic XY data transformations shared by the browser commands.

use crate::Diagnostic;
use serde::Deserialize;

const MAX_OUTPUT_POINTS: usize = 100_000;
const MAX_RUNNING_SAMPLES: usize = 5_000_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub(crate) enum SeriesOperation {
    Histogram {
        start: f64,
        stop: f64,
        bins: usize,
        cumulative: bool,
        normalize: bool,
    },
    Running {
        length: usize,
        statistic: RunningStatistic,
    },
    Interpolate {
        start: f64,
        stop: f64,
        points: usize,
        interpolation: InterpolationMethod,
        strict: bool,
    },
    Difference {
        scheme: DifferenceScheme,
    },
    Integrate,
    Regression {
        degree: usize,
        #[serde(default)]
        model: RegressionModel,
        #[serde(default)]
        output: RegressionOutput,
    },
    SeasonalDifference {
        period: usize,
    },
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RunningStatistic {
    Mean,
    Median,
    Minimum,
    Maximum,
    Stddev,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum InterpolationMethod {
    Linear,
    Spline,
    Akima,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DifferenceScheme {
    Forward,
    Backward,
    Centered,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RegressionModel {
    #[default]
    Polynomial,
    Power,
    Exponential,
    Logarithmic,
    Reciprocal,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RegressionOutput {
    #[default]
    Fitted,
    Residuals,
}

pub(crate) fn transform(
    rows: &[Vec<f64>],
    operation: SeriesOperation,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    let points = xy(rows)?;
    match operation {
        SeriesOperation::Histogram {
            start,
            stop,
            bins,
            cumulative,
            normalize,
        } => histogram(points, start, stop, bins, cumulative, normalize),
        SeriesOperation::Running { length, statistic } => running(points, length, statistic),
        SeriesOperation::Interpolate {
            start,
            stop,
            points: count,
            interpolation,
            strict,
        } => interpolate(points, start, stop, count, interpolation, strict),
        SeriesOperation::Difference { scheme } => difference(points, scheme),
        SeriesOperation::Integrate => integrate(points),
        SeriesOperation::Regression {
            degree,
            model,
            output,
        } => regression(points, degree, model, output),
        SeriesOperation::SeasonalDifference { period } => seasonal_difference(points, period),
    }
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(message)
}

fn xy(rows: &[Vec<f64>]) -> Result<Vec<(f64, f64)>, Diagnostic> {
    if rows.is_empty() {
        return Err(invalid("Select a non-empty XY data set."));
    }
    if rows.len() > MAX_OUTPUT_POINTS {
        return Err(invalid("This operation supports at most 100,000 points."));
    }
    rows.iter()
        .map(|row| match row.as_slice() {
            [x, y, ..] if x.is_finite() && y.is_finite() => Ok((*x, *y)),
            _ => Err(invalid("Select finite XY data.")),
        })
        .collect()
}

fn histogram(
    points: Vec<(f64, f64)>,
    start: f64,
    stop: f64,
    bins: usize,
    cumulative: bool,
    normalize: bool,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    if !start.is_finite() || !stop.is_finite() || start >= stop {
        return Err(invalid("Histogram limits must be finite and increasing."));
    }
    if bins == 0 || bins > MAX_OUTPUT_POINTS.saturating_sub(1) {
        return Err(invalid("Choose between 1 and 99,999 histogram bins."));
    }
    let width = (stop - start) / bins as f64;
    let mut counts = vec![0usize; bins];
    for (_, value) in &points {
        if *value < start || *value > stop {
            continue;
        }
        // Grace's bins include both endpoints and scan from the first bin,
        // so a shared edge belongs to the lower bin.
        let bin =
            (((*value - start) / width).ceil() as isize - 1).clamp(0, bins as isize - 1) as usize;
        counts[bin] += 1;
    }
    let mut total = 0usize;
    let mut output = Vec::with_capacity(bins + 1);
    output.push(vec![start, 0.]);
    for (index, count) in counts.into_iter().enumerate() {
        total += count;
        let raw = if cumulative {
            total as f64
        } else {
            count as f64
        };
        let value = if normalize {
            if cumulative {
                raw / points.len() as f64
            } else {
                raw / (width * points.len() as f64)
            }
        } else {
            raw
        };
        output.push(vec![start + width * (index + 1) as f64, value]);
    }
    Ok(output)
}

fn running(
    points: Vec<(f64, f64)>,
    length: usize,
    statistic: RunningStatistic,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    if length < 2 || length > points.len() {
        return Err(invalid(
            "Running window length must be between 2 and the set length.",
        ));
    }
    let work = length.saturating_mul(points.len() - length + 1);
    if work > MAX_RUNNING_SAMPLES {
        return Err(invalid(
            "Running window is too large for interactive calculation.",
        ));
    }
    let mut output = Vec::with_capacity(points.len() - length + 1);
    for window in points.windows(length) {
        let x = window.iter().map(|(x, _)| x).sum::<f64>() / length as f64;
        let value = match statistic {
            RunningStatistic::Mean => window.iter().map(|(_, y)| y).sum::<f64>() / length as f64,
            RunningStatistic::Median => {
                let mut values: Vec<_> = window.iter().map(|(_, y)| *y).collect();
                values.sort_by(f64::total_cmp);
                (values[(length - 1) / 2] + values[length / 2]) / 2.
            }
            RunningStatistic::Minimum => {
                window.iter().map(|(_, y)| *y).fold(f64::INFINITY, f64::min)
            }
            RunningStatistic::Maximum => window
                .iter()
                .map(|(_, y)| *y)
                .fold(f64::NEG_INFINITY, f64::max),
            RunningStatistic::Stddev => {
                let mean = window.iter().map(|(_, y)| y).sum::<f64>() / length as f64;
                (window.iter().map(|(_, y)| (*y - mean).powi(2)).sum::<f64>() / (length - 1) as f64)
                    .sqrt()
            }
        };
        output.push(vec![x, value]);
    }
    Ok(output)
}

fn monotonic(points: &[(f64, f64)], strict: bool) -> Result<bool, Diagnostic> {
    if points.len() < 2 {
        return Err(invalid("Interpolation requires at least two points."));
    }
    let direction = points
        .windows(2)
        .find_map(|pair| (pair[1].0 != pair[0].0).then_some(pair[1].0 > pair[0].0))
        .ok_or_else(|| invalid("Interpolation requires varying X coordinates."))?;
    let valid = points.windows(2).all(|pair| {
        let difference = pair[1].0 - pair[0].0;
        (difference > 0.) == direction && (!strict || difference != 0.)
            || (!strict && difference == 0.)
    });
    if !valid {
        return Err(invalid(if strict {
            "Spline interpolation requires strictly monotonic X coordinates."
        } else {
            "Linear interpolation requires monotonic X coordinates."
        }));
    }
    Ok(direction)
}

fn interpolate(
    mut points: Vec<(f64, f64)>,
    start: f64,
    stop: f64,
    count: usize,
    method: InterpolationMethod,
    strict: bool,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    if !start.is_finite() || !stop.is_finite() || start > stop {
        return Err(invalid(
            "Interpolation limits must be finite and increasing.",
        ));
    }
    if count == 0 || count > MAX_OUTPUT_POINTS {
        return Err(invalid(
            "Choose between 1 and 100,000 interpolation points.",
        ));
    }
    let strict_x = !matches!(method, InterpolationMethod::Linear);
    if !monotonic(&points, strict_x)? {
        points.reverse();
    }
    let (low, high) = (points[0].0, points.last().expect("non-empty").0);
    let mesh = if count == 1 {
        vec![start]
    } else {
        (0..count)
            .map(|i| start + (stop - start) * i as f64 / (count - 1) as f64)
            .collect()
    };
    let values = match method {
        InterpolationMethod::Linear => mesh
            .iter()
            .map(|&x| linear(&points, x))
            .collect::<Result<Vec<_>, _>>()?,
        InterpolationMethod::Spline => cubic_spline(&points, &mesh)?,
        InterpolationMethod::Akima => akima(&points, &mesh)?,
    };
    Ok(mesh
        .into_iter()
        .zip(values)
        .filter(|(x, _)| !strict || (*x >= low && *x <= high))
        .map(|(x, y)| vec![x, y])
        .collect())
}

fn interval(points: &[(f64, f64)], x: f64) -> usize {
    match points.binary_search_by(|point| point.0.total_cmp(&x)) {
        Ok(index) => index.min(points.len() - 2),
        Err(index) => index.saturating_sub(1).min(points.len() - 2),
    }
}

fn linear(points: &[(f64, f64)], x: f64) -> Result<f64, Diagnostic> {
    if let Some(pair) = points
        .windows(2)
        .find(|pair| pair[0].0 == x && pair[1].0 == x)
    {
        return Ok((pair[0].1 + pair[1].1) / 2.);
    }
    let i = interval(points, x);
    let ((x0, y0), (x1, y1)) = (points[i], points[i + 1]);
    if x0 == x1 {
        return Ok((y0 + y1) / 2.);
    }
    let value = y0 + (x - x0) * (y1 - y0) / (x1 - x0);
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| invalid("Interpolation produced an invalid value."))
}

fn cubic_spline(points: &[(f64, f64)], mesh: &[f64]) -> Result<Vec<f64>, Diagnostic> {
    let n = points.len();
    if n < 3 {
        return mesh.iter().map(|&x| linear(points, x)).collect();
    }
    let mut lower = vec![0.; n];
    let mut diagonal = vec![1.; n];
    let mut upper = vec![0.; n];
    let mut rhs = vec![0.; n];
    for i in 1..n - 1 {
        let h0 = points[i].0 - points[i - 1].0;
        let h1 = points[i + 1].0 - points[i].0;
        lower[i] = h0;
        diagonal[i] = 2. * (h0 + h1);
        upper[i] = h1;
        rhs[i] = 6. * ((points[i + 1].1 - points[i].1) / h1 - (points[i].1 - points[i - 1].1) / h0);
    }
    for i in 1..n {
        let factor = lower[i] / diagonal[i - 1];
        diagonal[i] -= factor * upper[i - 1];
        rhs[i] -= factor * rhs[i - 1];
    }
    let mut second = vec![0.; n];
    second[n - 1] = rhs[n - 1] / diagonal[n - 1];
    for i in (0..n - 1).rev() {
        second[i] = (rhs[i] - upper[i] * second[i + 1]) / diagonal[i];
    }
    mesh.iter()
        .map(|&x| {
            let i = interval(points, x);
            let h = points[i + 1].0 - points[i].0;
            let a = (points[i + 1].0 - x) / h;
            let b = (x - points[i].0) / h;
            let value = a * points[i].1
                + b * points[i + 1].1
                + ((a.powi(3) - a) * second[i] + (b.powi(3) - b) * second[i + 1]) * h.powi(2) / 6.;
            value
                .is_finite()
                .then_some(value)
                .ok_or_else(|| invalid("Interpolation produced an invalid value."))
        })
        .collect()
}

fn akima(points: &[(f64, f64)], mesh: &[f64]) -> Result<Vec<f64>, Diagnostic> {
    if points.len() < 5 {
        return cubic_spline(points, mesh);
    }
    let slopes: Vec<_> = points
        .windows(2)
        .map(|pair| (pair[1].1 - pair[0].1) / (pair[1].0 - pair[0].0))
        .collect();
    let mut tangent = vec![0.; points.len()];
    tangent[0] = slopes[0];
    tangent[1] = (slopes[0] + slopes[1]) / 2.;
    for i in 2..points.len() - 2 {
        let w0 = (slopes[i + 1] - slopes[i]).abs();
        let w1 = (slopes[i - 1] - slopes[i - 2]).abs();
        tangent[i] = if w0 + w1 == 0. {
            (slopes[i - 1] + slopes[i]) / 2.
        } else {
            (w0 * slopes[i - 1] + w1 * slopes[i]) / (w0 + w1)
        };
    }
    let n = points.len();
    tangent[n - 2] = (slopes[n - 3] + slopes[n - 2]) / 2.;
    tangent[n - 1] = slopes[n - 2];
    mesh.iter()
        .map(|&x| {
            let i = interval(points, x);
            let h = points[i + 1].0 - points[i].0;
            let t = (x - points[i].0) / h;
            let value = (2. * t.powi(3) - 3. * t.powi(2) + 1.) * points[i].1
                + (t.powi(3) - 2. * t.powi(2) + t) * h * tangent[i]
                + (-2. * t.powi(3) + 3. * t.powi(2)) * points[i + 1].1
                + (t.powi(3) - t.powi(2)) * h * tangent[i + 1];
            value
                .is_finite()
                .then_some(value)
                .ok_or_else(|| invalid("Interpolation produced an invalid value."))
        })
        .collect()
}

fn difference(
    points: Vec<(f64, f64)>,
    scheme: DifferenceScheme,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    let need = match scheme {
        DifferenceScheme::Centered => 3,
        _ => 2,
    };
    if points.len() < need {
        return Err(invalid("This difference requires more points."));
    }
    let range: Box<dyn Iterator<Item = usize>> = match scheme {
        DifferenceScheme::Centered => Box::new(1..points.len() - 1),
        _ => Box::new(0..points.len() - 1),
    };
    range
        .map(|i| {
            let (left, middle, right) = match scheme {
                DifferenceScheme::Forward => (points[i], points[i], points[i + 1]),
                DifferenceScheme::Backward => (points[i], points[i], points[i + 1]),
                DifferenceScheme::Centered => (points[i - 1], points[i], points[i + 1]),
            };
            let (x, y) = match scheme {
                DifferenceScheme::Centered => (middle.0, (right.1 - left.1) / (right.0 - left.0)),
                DifferenceScheme::Forward | DifferenceScheme::Backward => {
                    (middle.0, (right.1 - left.1) / (right.0 - left.0))
                }
            };
            y.is_finite()
                .then_some(vec![x, y])
                .ok_or_else(|| invalid("Differences require distinct neighboring X coordinates."))
        })
        .collect()
}

fn integrate(points: Vec<(f64, f64)>) -> Result<Vec<Vec<f64>>, Diagnostic> {
    if points.len() < 2 {
        return Err(invalid("Integration requires at least two points."));
    }
    let mut total = 0.;
    let mut output = vec![vec![points[0].0, 0.]];
    for pair in points.windows(2) {
        total += (pair[1].0 - pair[0].0) * (pair[0].1 + pair[1].1) / 2.;
        if !total.is_finite() {
            return Err(invalid("Integration produced an invalid value."));
        }
        output.push(vec![pair[1].0, total]);
    }
    Ok(output)
}

fn seasonal_difference(
    points: Vec<(f64, f64)>,
    period: usize,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    if period == 0 || period >= points.len() {
        return Err(invalid(
            "Seasonal period must be between 1 and one less than the set length.",
        ));
    }
    Ok((0..points.len() - period)
        .map(|i| vec![points[i].0, points[i].1 - points[i + period].1])
        .collect())
}

fn regression(
    points: Vec<(f64, f64)>,
    degree: usize,
    model: RegressionModel,
    output: RegressionOutput,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    if !matches!(model, RegressionModel::Polynomial) && degree != 1 {
        return Err(invalid("Non-polynomial regression uses degree 1."));
    }
    if degree > 10 || points.len() < degree + 1 {
        return Err(invalid(
            "Regression degree must be 0 through 10 with enough points.",
        ));
    }
    let fitting_points = points
        .iter()
        .map(|&(x, y)| match model {
            RegressionModel::Polynomial => Ok((x, y)),
            RegressionModel::Power if x > 0. && y > 0. => Ok((x.ln(), y.ln())),
            RegressionModel::Power => Err(invalid(
                "Power regression requires positive X and Y values.",
            )),
            RegressionModel::Exponential if y > 0. => Ok((x, y.ln())),
            RegressionModel::Exponential => Err(invalid(
                "Exponential regression requires positive Y values.",
            )),
            RegressionModel::Logarithmic if x > 0. => Ok((x.ln(), y)),
            RegressionModel::Logarithmic => Err(invalid(
                "Logarithmic regression requires positive X values.",
            )),
            RegressionModel::Reciprocal if y != 0. => Ok((x, 1. / y)),
            RegressionModel::Reciprocal => {
                Err(invalid("Reciprocal regression requires nonzero Y values."))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let low = fitting_points
        .iter()
        .map(|p| p.0)
        .fold(f64::INFINITY, f64::min);
    let high = fitting_points
        .iter()
        .map(|p| p.0)
        .fold(f64::NEG_INFINITY, f64::max);
    if degree > 0 && low == high {
        return Err(invalid("Regression requires varying X coordinates."));
    }
    let m = degree + 1;
    let n = points.len();
    let scale = if low == high { 1. } else { (high - low) / 2. };
    let center = (high + low) / 2.;
    let mut columns = (0..m)
        .map(|power| {
            fitting_points
                .iter()
                .map(|(x, _)| ((x - center) / scale).powi(power as i32))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut r = vec![vec![0.; m]; m];
    for j in 0..m {
        for i in 0..j {
            r[i][j] = columns[i].iter().zip(&columns[j]).map(|(a, b)| a * b).sum();
            for k in 0..n {
                columns[j][k] -= r[i][j] * columns[i][k];
            }
        }
        r[j][j] = columns[j].iter().map(|v| v * v).sum::<f64>().sqrt();
        if r[j][j] <= f64::EPSILON {
            return Err(invalid("Regression input is numerically singular."));
        }
        for value in &mut columns[j] {
            *value /= r[j][j];
        }
    }
    let mut coefficients = vec![0.; m];
    for i in (0..m).rev() {
        let projection: f64 = columns[i]
            .iter()
            .zip(fitting_points.iter())
            .map(|(q, (_, y))| q * y)
            .sum();
        let tail: f64 = (i + 1..m).map(|j| r[i][j] * coefficients[j]).sum();
        coefficients[i] = (projection - tail) / r[i][i];
    }
    points
        .into_iter()
        .map(|(x, observed)| {
            let fitted_x = match model {
                RegressionModel::Power | RegressionModel::Logarithmic => x.ln(),
                _ => x,
            };
            let z = (fitted_x - center) / scale;
            let transformed = coefficients
                .iter()
                .rev()
                .fold(0., |sum, coefficient| sum * z + coefficient);
            let fitted = match model {
                RegressionModel::Polynomial | RegressionModel::Logarithmic => transformed,
                RegressionModel::Power | RegressionModel::Exponential => transformed.exp(),
                RegressionModel::Reciprocal => 1. / transformed,
            };
            let value = match output {
                RegressionOutput::Fitted => fitted,
                RegressionOutput::Residuals => observed - fitted,
            };
            value
                .is_finite()
                .then_some(vec![x, value])
                .ok_or_else(|| invalid("Regression produced an invalid value."))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rows(values: &[(f64, f64)]) -> Vec<Vec<f64>> {
        values.iter().map(|(x, y)| vec![*x, *y]).collect()
    }
    fn close(left: f64, right: f64) {
        assert!((left - right).abs() < 1e-9, "{left} != {right}");
    }
    #[test]
    fn histogram_matches_grace_staircase_shape() {
        let out = transform(
            &rows(&[(0., 0.), (1., 0.5), (2., 1.)]),
            SeriesOperation::Histogram {
                start: 0.,
                stop: 1.,
                bins: 2,
                cumulative: false,
                normalize: false,
            },
        )
        .unwrap();
        assert_eq!(out, vec![vec![0., 0.], vec![0.5, 2.], vec![1., 1.]]);
    }
    #[test]
    fn running_and_differences_use_xy_coordinates() {
        let points = rows(&[(0., 0.), (1., 1.), (2., 4.)]);
        let average = transform(
            &points,
            SeriesOperation::Running {
                length: 2,
                statistic: RunningStatistic::Mean,
            },
        )
        .unwrap();
        assert_eq!(average, vec![vec![0.5, 0.5], vec![1.5, 2.5]]);
        let diff = transform(
            &points,
            SeriesOperation::Difference {
                scheme: DifferenceScheme::Centered,
            },
        )
        .unwrap();
        assert_eq!(diff, vec![vec![1., 2.]]);
    }
    #[test]
    fn running_stddev_uses_grace_sample_divisor() {
        let out = transform(
            &rows(&[(0., 1.), (2., 5.), (5., 2.), (9., 10.)]),
            SeriesOperation::Running {
                length: 2,
                statistic: RunningStatistic::Stddev,
            },
        )
        .unwrap();
        close(out[0][1], 2.828_427_124_746_190_3);
        close(out[1][1], 2.121_320_343_559_642_4);
        close(out[2][1], 5.656_854_249_492_381);
    }
    #[test]
    fn interpolation_strict_drops_extrapolation() {
        let out = transform(
            &rows(&[(0., 0.), (2., 2.)]),
            SeriesOperation::Interpolate {
                start: -1.,
                stop: 3.,
                points: 5,
                interpolation: InterpolationMethod::Linear,
                strict: true,
            },
        )
        .unwrap();
        assert_eq!(out, vec![vec![0., 0.], vec![1., 1.], vec![2., 2.]]);
    }
    #[test]
    fn linear_interpolation_averages_exact_duplicate_abscissas() {
        let out = transform(
            &rows(&[(0., 0.), (1., 10.), (1., 20.), (2., 40.)]),
            SeriesOperation::Interpolate {
                start: 0.,
                stop: 2.,
                points: 3,
                interpolation: InterpolationMethod::Linear,
                strict: false,
            },
        )
        .unwrap();
        assert_eq!(out, vec![vec![0., 0.], vec![1., 15.], vec![2., 40.]]);
    }
    #[test]
    fn integration_and_seasonal_difference() {
        let points = rows(&[(0., 0.), (1., 1.), (2., 2.)]);
        let integral = transform(&points, SeriesOperation::Integrate).unwrap();
        assert_eq!(integral, vec![vec![0., 0.], vec![1., 0.5], vec![2., 2.]]);
        let seasonal =
            transform(&points, SeriesOperation::SeasonalDifference { period: 1 }).unwrap();
        assert_eq!(seasonal, vec![vec![0., -1.], vec![1., -1.]]);
    }
    #[test]
    fn regression_recovers_quadratic() {
        let out = transform(
            &rows(&[(-2., 7.), (-1., 2.), (0., 1.), (1., 4.), (2., 11.)]),
            SeriesOperation::Regression {
                degree: 2,
                model: RegressionModel::Polynomial,
                output: RegressionOutput::Fitted,
            },
        )
        .unwrap();
        for (index, row) in out.iter().enumerate() {
            close(row[1], [7., 2., 1., 4., 11.][index]);
        }
    }
    #[test]
    fn classic_regression_models_and_residuals() {
        let power = transform(
            &rows(&[(1., 3.), (2., 12.), (3., 27.)]),
            SeriesOperation::Regression {
                degree: 1,
                model: RegressionModel::Power,
                output: RegressionOutput::Fitted,
            },
        )
        .unwrap();
        close(power[2][1], 27.);
        let residuals = transform(
            &rows(&[(0., 1.), (1., 3.), (2., 5.)]),
            SeriesOperation::Regression {
                degree: 1,
                model: RegressionModel::Polynomial,
                output: RegressionOutput::Residuals,
            },
        )
        .unwrap();
        for row in residuals {
            close(row[1], 0.);
        }
    }
}
