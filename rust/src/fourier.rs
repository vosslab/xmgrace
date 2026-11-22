//! Fourier transforms compatible with the bundled Grace 5.1.25 implementation.
//!
//! Grace's historical DFT normalizes a forward transform, while its radix-two
//! FFT normalizes an inverse transform.  The distinction is intentional here
//! because exported numerical results must retain those established semantics.

use crate::Diagnostic;
use serde::Deserialize;
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Algorithm {
    Dft,
    Fft,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Input {
    Real,
    Complex,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Direction {
    Forward,
    Inverse,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Output {
    Magnitude,
    Phase,
    Coefficients,
    Windowed,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Abscissa {
    Index,
    Frequency,
    Period,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Window {
    None,
    Triangular,
    Hann,
    Welch,
    Hamming,
    Blackman,
    Parzen,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FourierOptions {
    pub(crate) algorithm: Algorithm,
    pub(crate) input: Input,
    pub(crate) direction: Direction,
    pub(crate) output: Output,
    pub(crate) abscissa: Abscissa,
    pub(crate) window: Window,
}

#[derive(Debug, Clone, Copy)]
struct Complex {
    real: f64,
    imaginary: f64,
}

impl Complex {
    fn magnitude(self) -> f64 {
        self.real.hypot(self.imaginary)
    }
}

/// Transforms valid XY rows and returns rows suitable for a new Grace XY set.
///
/// Real input treats Y as the signal and keeps X for output frequency or period
/// coordinates. Complex input treats X and Y as real and imaginary values.
pub(crate) fn transform(
    rows: &[Vec<f64>],
    options: FourierOptions,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    let count = rows.len();
    if count < 2 {
        return Err(Diagnostic::new(
            "A Fourier transform needs at least two points.",
        ));
    }
    if rows
        .iter()
        .any(|row| row.len() != 2 || row.iter().any(|coordinate| !coordinate.is_finite()))
    {
        return Err(Diagnostic::new("A Fourier transform needs finite XY data."));
    }
    if matches!(options.algorithm, Algorithm::Fft) && !count.is_power_of_two() {
        return Err(Diagnostic::new(
            "FFT input length must be a power of two. Use DFT or resample the data.",
        ));
    }

    let windowed = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let weight = window_weight(options.window, index, count);
            match options.input {
                Input::Real => Complex {
                    real: row[1] * weight,
                    imaginary: 0.,
                },
                Input::Complex => Complex {
                    real: row[0] * weight,
                    imaginary: row[1] * weight,
                },
            }
        })
        .collect::<Vec<_>>();

    if matches!(options.output, Output::Windowed) {
        return Ok(rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let weight = window_weight(options.window, index, count);
                match options.input {
                    Input::Real => vec![row[0], row[1] * weight],
                    Input::Complex => vec![row[0] * weight, row[1] * weight],
                }
            })
            .collect());
    }

    let coefficients = match options.algorithm {
        Algorithm::Dft => dft(&windowed, options.direction),
        Algorithm::Fft => fft(windowed, options.direction),
    };
    let x_coordinates = coordinates(rows, options.abscissa)?;

    match options.output {
        Output::Coefficients => Ok(coefficients
            .into_iter()
            .map(|coefficient| vec![coefficient.real, coefficient.imaginary])
            .collect()),
        Output::Magnitude => Ok((0..count / 2)
            .map(|index| {
                let magnitude = if index == 0 {
                    2. * coefficients[0].magnitude()
                } else {
                    coefficients[index].magnitude() + coefficients[count - index].magnitude()
                };
                vec![x_coordinates[index], magnitude]
            })
            .collect()),
        Output::Phase => Ok((0..count / 2)
            .map(|index| {
                vec![
                    x_coordinates[index],
                    -coefficients[index]
                        .imaginary
                        .atan2(coefficients[index].real),
                ]
            })
            .collect()),
        Output::Windowed => unreachable!("windowed output returned before transform"),
    }
}

fn coordinates(rows: &[Vec<f64>], abscissa: Abscissa) -> Result<Vec<f64>, Diagnostic> {
    let count = rows.len();
    let first = rows[0][0];
    let last = rows[count - 1][0];
    let span = last - first;
    let delta = span / (count - 1) as f64;
    match abscissa {
        Abscissa::Index => Ok((0..count / 2).map(|index| index as f64).collect()),
        Abscissa::Frequency if span != 0. => {
            Ok((0..count / 2).map(|index| index as f64 / span).collect())
        }
        Abscissa::Period if span != 0. => Ok((0..count / 2)
            .map(|index| {
                if index == 0 {
                    span + delta
                } else {
                    span / index as f64
                }
            })
            .collect()),
        Abscissa::Frequency | Abscissa::Period => Err(Diagnostic::new(
            "Frequency and period output need distinct first and last X values.",
        )),
    }
}

fn window_weight(window: Window, index: usize, count: usize) -> f64 {
    let point = index as f64;
    let last = (count - 1) as f64;
    let centered = (point - 0.5 * last) / (0.5 * last);
    match window {
        Window::None => 1.,
        Window::Triangular => 1. - centered.abs(),
        Window::Hann => 0.5 - 0.5 * (2. * PI * point / last).cos(),
        Window::Welch => 1. - ((point - 0.5 * last) / (0.5 * (count + 1) as f64)).powi(2),
        Window::Hamming => 0.54 - 0.46 * (2. * PI * point / last).cos(),
        Window::Blackman => {
            0.42 - 0.5 * (2. * PI * point / last).cos() + 0.08 * (4. * PI * point / last).cos()
        }
        Window::Parzen => 1. - ((point - 0.5 * last) / (0.5 * (count + 1) as f64)).abs(),
    }
}

fn dft(input: &[Complex], direction: Direction) -> Vec<Complex> {
    let count = input.len() as f64;
    let sign = match direction {
        Direction::Forward => -1.,
        Direction::Inverse => 1.,
    };
    let scale = match direction {
        Direction::Forward => 1. / count,
        Direction::Inverse => 1.,
    };
    (0..input.len())
        .map(|output| {
            let mut result = Complex {
                real: 0.,
                imaginary: 0.,
            };
            for (source, value) in input.iter().enumerate() {
                let angle = sign * 2. * PI * source as f64 * output as f64 / count;
                let cosine = angle.cos();
                let sine = angle.sin();
                result.real += value.real * cosine - value.imaginary * sine;
                result.imaginary += value.real * sine + value.imaginary * cosine;
            }
            Complex {
                real: result.real * scale,
                imaginary: result.imaginary * scale,
            }
        })
        .collect()
}

fn fft(mut values: Vec<Complex>, direction: Direction) -> Vec<Complex> {
    let count = values.len();
    let mut reversed = 0;
    for index in 1..count {
        let mut bit = count >> 1;
        while reversed & bit != 0 {
            reversed ^= bit;
            bit >>= 1;
        }
        reversed ^= bit;
        if index < reversed {
            values.swap(index, reversed);
        }
    }

    let sign = match direction {
        Direction::Forward => -1.,
        Direction::Inverse => 1.,
    };
    let mut width = 2;
    while width <= count {
        let angle = sign * 2. * PI / width as f64;
        let twiddle = Complex {
            real: angle.cos(),
            imaginary: angle.sin(),
        };
        for start in (0..count).step_by(width) {
            let mut factor = Complex {
                real: 1.,
                imaginary: 0.,
            };
            for offset in 0..width / 2 {
                let lower = values[start + offset + width / 2];
                let product = Complex {
                    real: lower.real * factor.real - lower.imaginary * factor.imaginary,
                    imaginary: lower.real * factor.imaginary + lower.imaginary * factor.real,
                };
                let upper = values[start + offset];
                values[start + offset] = Complex {
                    real: upper.real + product.real,
                    imaginary: upper.imaginary + product.imaginary,
                };
                values[start + offset + width / 2] = Complex {
                    real: upper.real - product.real,
                    imaginary: upper.imaginary - product.imaginary,
                };
                factor = Complex {
                    real: factor.real * twiddle.real - factor.imaginary * twiddle.imaginary,
                    imaginary: factor.real * twiddle.imaginary + factor.imaginary * twiddle.real,
                };
            }
        }
        width *= 2;
    }
    if matches!(direction, Direction::Inverse) {
        for value in &mut values {
            value.real /= count as f64;
            value.imaginary /= count as f64;
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(algorithm: Algorithm, output: Output) -> FourierOptions {
        FourierOptions {
            algorithm,
            input: Input::Real,
            direction: Direction::Forward,
            output,
            abscissa: Abscissa::Index,
            window: Window::None,
        }
    }

    fn assert_near(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-10,
            "expected {expected}, received {actual}"
        );
    }

    #[test]
    fn dft_and_fft_keep_graces_historical_normalization() {
        let rows = vec![vec![0., 1.], vec![1., 0.], vec![2., 0.], vec![3., 0.]];
        let dft = transform(&rows, options(Algorithm::Dft, Output::Coefficients)).unwrap();
        let fft = transform(&rows, options(Algorithm::Fft, Output::Coefficients)).unwrap();
        assert_near(dft[0][0], 0.25);
        assert_near(fft[0][0], 1.);
    }

    #[test]
    fn gracebat_fft_sine_magnitude_and_phase_match() {
        // Reproduce with Grace 5.1.25:
        // gracebat sine.agr -pexec 'FFT(S0,REAL,NONE,INDEX,MAGNITUDE)' -saveall out.agr
        let rows = vec![vec![0., 0.], vec![1., 1.], vec![2., 0.], vec![3., -1.]];
        let spectrum = transform(&rows, options(Algorithm::Fft, Output::Magnitude)).unwrap();
        assert_near(spectrum[0][1], 0.);
        assert_near(spectrum[1][0], 1.);
        assert_near(spectrum[1][1], 4.);

        let phase = transform(&rows, options(Algorithm::Fft, Output::Phase)).unwrap();
        assert_near(phase[0][1], 0.);
        assert_near(phase[1][1], PI / 2.);
    }

    #[test]
    fn gracebat_fft_complex_coefficients_match() {
        // Reproduce with Grace 5.1.25:
        // gracebat complex.agr -pexec 'FFT(S0,COMPLEX,NONE,INDEX,COEFFICIENTS)' -saveall out.agr
        let rows = vec![vec![1., 0.], vec![0., 1.], vec![-1., 0.], vec![0., -1.]];
        let mut selected = options(Algorithm::Fft, Output::Coefficients);
        selected.input = Input::Complex;
        let coefficients = transform(&rows, selected).unwrap();
        assert_near(coefficients[0][0], 0.);
        assert_near(coefficients[1][0], 4.);
        assert_near(coefficients[1][1], 0.);
        assert_near(coefficients[3][1], 0.);
    }

    #[test]
    fn gracebat_fft_hamming_magnitude_matches() {
        // Reproduce with Grace 5.1.25:
        // gracebat impulse.agr -pexec 'FFT(S0,REAL,HAMMING,INDEX,MAGNITUDE)' -saveall out.agr
        let rows = vec![vec![0., 1.], vec![1., 0.], vec![2., 0.], vec![3., 0.]];
        let mut selected = options(Algorithm::Fft, Output::Magnitude);
        selected.window = Window::Hamming;
        let spectrum = transform(&rows, selected).unwrap();
        assert_near(spectrum[0][1], 0.16);
        assert_near(spectrum[1][1], 0.16);
    }

    #[test]
    fn fft_rejects_a_non_power_of_two_length() {
        let rows = vec![vec![0., 0.], vec![1., 1.], vec![2., 0.]];
        let error = transform(&rows, options(Algorithm::Fft, Output::Magnitude)).unwrap_err();
        assert!(error.message.contains("power of two"));
    }

    #[test]
    fn window_only_preserves_x_for_real_data() {
        let rows = vec![vec![10., 2.], vec![11., 2.], vec![12., 2.], vec![13., 2.]];
        let mut selected = options(Algorithm::Dft, Output::Windowed);
        selected.window = Window::Hamming;
        let windowed = transform(&rows, selected).unwrap();
        assert_near(windowed[0][0], 10.);
        assert_near(windowed[0][1], 0.16);
        assert_near(windowed[1][1], 1.54);
    }

    #[test]
    fn frequency_requires_an_x_span() {
        let rows = vec![vec![3., 0.], vec![3., 1.]];
        let mut selected = options(Algorithm::Dft, Output::Magnitude);
        selected.abscissa = Abscissa::Frequency;
        let error = transform(&rows, selected).unwrap_err();
        assert!(error.message.contains("distinct"));
    }
}
