//! Coordinate transforms used by Cartesian Grace axes.

use crate::{Settings, value};

/// The coordinate systems supported by classic Grace Cartesian axes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Scale {
    #[default]
    Normal,
    Logarithmic,
    Reciprocal,
    Logit,
}

impl Scale {
    /// Reads an axis scale setting, using Grace's normal scale for unknown values.
    pub(crate) fn from_settings(settings: &Settings, axis: &str) -> Self {
        Self::parse(value(settings, &format!("{axis}axes scale"), "Normal")).unwrap_or(Self::Normal)
    }

    /// Parses the names saved by Grace and a concise logarithmic alias.
    pub(crate) fn parse(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("normal") {
            Some(Self::Normal)
        } else if name.eq_ignore_ascii_case("logarithmic") || name.eq_ignore_ascii_case("log") {
            Some(Self::Logarithmic)
        } else if name.eq_ignore_ascii_case("reciprocal") {
            Some(Self::Reciprocal)
        } else if name.eq_ignore_ascii_case("logit") {
            Some(Self::Logit)
        } else {
            None
        }
    }

    /// Returns the spelling used in Grace project files and controls.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Logarithmic => "Logarithmic",
            Self::Reciprocal => "Reciprocal",
            Self::Logit => "Logit",
        }
    }

    /// Projects a world coordinate into the evenly spaced coordinate system.
    pub(crate) fn project(self, coordinate: f64) -> Option<f64> {
        let projected = match self {
            Self::Normal => coordinate,
            Self::Logarithmic if coordinate > 0. => coordinate.log10(),
            Self::Reciprocal if coordinate != 0. => coordinate.recip(),
            Self::Logit if coordinate > 0. && coordinate < 1. => {
                (coordinate / (1. - coordinate)).ln()
            }
            _ => return None,
        };
        projected.is_finite().then_some(projected)
    }

    /// Returns a world coordinate from its projected representation.
    pub(crate) fn unproject(self, coordinate: f64) -> Option<f64> {
        if !coordinate.is_finite() {
            return None;
        }
        let world = match self {
            Self::Normal => coordinate,
            Self::Logarithmic => 10_f64.powf(coordinate),
            Self::Reciprocal if coordinate != 0. => coordinate.recip(),
            // This form avoids overflowing exp() at either end of the logit scale.
            Self::Logit if coordinate >= 0. => 1. / (1. + (-coordinate).exp()),
            Self::Logit => {
                let exponential = coordinate.exp();
                exponential / (1. + exponential)
            }
            _ => return None,
        };
        world.is_finite().then_some(world)
    }

    /// Checks the ordered world range that can be represented by this scale.
    pub(crate) fn valid_range(self, low: f64, high: f64) -> bool {
        if !low.is_finite() || !high.is_finite() || low >= high {
            return false;
        }
        match self {
            Self::Normal => true,
            Self::Logarithmic => low > 0.,
            Self::Reciprocal => low.signum() == high.signum() && low != 0. && high != 0.,
            Self::Logit => low > 0. && high < 1.,
        }
    }

    /// True for Grace's base-ten logarithmic scale.
    pub(crate) const fn is_log(self) -> bool {
        matches!(self, Self::Logarithmic)
    }
}

#[cfg(test)]
mod tests {
    use super::Scale;

    #[test]
    fn projects_and_inverts_all_supported_scales() {
        for (scale, world) in [
            (Scale::Normal, -3.5),
            (Scale::Logarithmic, 100.),
            (Scale::Reciprocal, -4.),
            (Scale::Logit, 0.2),
        ] {
            let projected = scale.project(world).expect("valid coordinate projects");
            let restored = scale.unproject(projected).expect("projection inverts");
            assert!((restored - world).abs() < 1e-12, "{scale:?}");
        }
    }

    #[test]
    fn rejects_invalid_domains_and_ranges() {
        assert!(Scale::Logarithmic.project(0.).is_none());
        assert!(Scale::Reciprocal.project(0.).is_none());
        assert!(Scale::Logit.project(1.).is_none());
        assert!(!Scale::Reciprocal.valid_range(-1., 1.));
        assert!(!Scale::Logit.valid_range(0., 0.5));
        assert!(!Scale::Logit.valid_range(0.1, 1.));
    }
}
