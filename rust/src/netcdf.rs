//! Minimal, bounded reader for the browser-safe NetCDF classic file formats.
//!
//! NetCDF CDF-1 and CDF-2 are self-contained binary table formats.  This
//! module deliberately accepts one-dimensional numeric variables only, which
//! maps directly to a Grace XY set without a browser-side native library.

use crate::Diagnostic;
use serde::Serialize;

const DIMENSION: u32 = 10;
const VARIABLE: u32 = 11;
const ATTRIBUTE: u32 = 12;
const MAX_ITEMS: usize = 4_096;
const MAX_VALUES: usize = 1_000_000;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct VariableInfo {
    pub(crate) name: String,
    pub(crate) length: usize,
}

#[derive(Debug, Clone)]
struct Dimension {
    length: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueType {
    Byte,
    Short,
    Int,
    Float,
    Double,
}

impl ValueType {
    fn parse(value: u32) -> Result<Self, Diagnostic> {
        match value {
            1 => Ok(Self::Byte),
            3 => Ok(Self::Short),
            4 => Ok(Self::Int),
            5 => Ok(Self::Float),
            6 => Ok(Self::Double),
            2 => Err(Diagnostic::new(
                "NetCDF character variables cannot be plotted as XY data.",
            )),
            _ => Err(Diagnostic::new(
                "This NetCDF variable uses an unsupported data type.",
            )),
        }
    }

    const fn width(self) -> usize {
        match self {
            Self::Byte => 1,
            Self::Short => 2,
            Self::Int | Self::Float => 4,
            Self::Double => 8,
        }
    }

    fn value(self, bytes: &[u8]) -> Result<f64, Diagnostic> {
        let value = match self {
            Self::Byte => i8::from_be_bytes([bytes[0]]) as f64,
            Self::Short => i16::from_be_bytes([bytes[0], bytes[1]]) as f64,
            Self::Int => i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f64,
            Self::Float => {
                f32::from_bits(u32::from_be_bytes(bytes.try_into().expect("fixed width"))) as f64
            }
            Self::Double => {
                f64::from_bits(u64::from_be_bytes(bytes.try_into().expect("fixed width")))
            }
        };
        value
            .is_finite()
            .then_some(value)
            .ok_or_else(|| Diagnostic::new("NetCDF data contains a non-finite numeric value."))
    }
}

#[derive(Debug, Clone)]
struct Variable {
    name: String,
    dimensions: Vec<usize>,
    value_type: Option<ValueType>,
    begin: usize,
    vsize: usize,
}

#[derive(Debug)]
struct File<'a> {
    bytes: &'a [u8],
    records: usize,
    dimensions: Vec<Dimension>,
    variables: Vec<Variable>,
    record_stride: usize,
}

impl<'a> File<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, Diagnostic> {
        if bytes.starts_with(b"\x89HDF\r\n\x1a\n") {
            return Err(Diagnostic::new(
                "NetCDF-4/HDF5 files are not supported; export this data as NetCDF classic.",
            ));
        }
        if bytes.len() < 4 || &bytes[..3] != b"CDF" {
            return Err(Diagnostic::new(
                "Select a NetCDF classic CDF-1 or CDF-2 file.",
            ));
        }
        let offset_width = match bytes[3] {
            1 => 4,
            2 => 8,
            5 => return Err(Diagnostic::new("NetCDF CDF-5 files are not supported yet.")),
            _ => {
                return Err(Diagnostic::new(
                    "This file is not a supported NetCDF classic file.",
                ));
            }
        };
        let mut reader = Reader::new(bytes, 4);
        let records = reader.u32()? as usize;
        let dimensions = read_dimensions(&mut reader)?;
        skip_attributes(&mut reader)?;
        let variables = read_variables(&mut reader, offset_width, &dimensions)?;
        if variables
            .iter()
            .any(|variable| variable.begin < reader.cursor)
        {
            return Err(Diagnostic::new(
                "NetCDF data begins inside its header, so the file is malformed.",
            ));
        }
        let record_stride = variables
            .iter()
            .filter(|variable| {
                variable
                    .dimensions
                    .first()
                    .is_some_and(|dimension| dimensions[*dimension].length.is_none())
            })
            .try_fold(0usize, |sum, variable| sum.checked_add(variable.vsize))
            .ok_or_else(|| Diagnostic::new("NetCDF record layout is too large."))?;
        if records > MAX_VALUES {
            return Err(Diagnostic::new(format!(
                "NetCDF has {records} records. The browser limit is {MAX_VALUES}."
            )));
        }
        Ok(Self {
            bytes,
            records,
            dimensions,
            variables,
            record_stride,
        })
    }

    fn info(&self) -> Result<Vec<VariableInfo>, Diagnostic> {
        self.variables
            .iter()
            .filter(|variable| variable.dimensions.len() == 1 && variable.value_type.is_some())
            .map(|variable| {
                Ok(VariableInfo {
                    name: variable.name.clone(),
                    length: self.length(variable)?,
                })
            })
            .collect()
    }

    fn variable(&self, name: &str) -> Result<&Variable, Diagnostic> {
        self.variables
            .iter()
            .find(|variable| variable.name == name)
            .ok_or_else(|| Diagnostic::new(format!("NetCDF has no variable named {name:?}.")))
    }

    fn length(&self, variable: &Variable) -> Result<usize, Diagnostic> {
        if variable.dimensions.len() != 1 || variable.value_type.is_none() {
            return Err(Diagnostic::new(format!(
                "NetCDF variable {:?} is not a one-dimensional numeric variable.",
                variable.name
            )));
        }
        let length = self.dimensions[variable.dimensions[0]]
            .length
            .unwrap_or(self.records);
        if length > MAX_VALUES {
            return Err(Diagnostic::new(format!(
                "NetCDF variable {:?} has {length} values. The browser limit is {MAX_VALUES}.",
                variable.name
            )));
        }
        Ok(length)
    }

    fn values(&self, variable: &Variable) -> Result<Vec<f64>, Diagnostic> {
        let length = self.length(variable)?;
        let value_type = variable.value_type.ok_or_else(|| {
            Diagnostic::new(format!(
                "NetCDF variable {:?} is not numeric.",
                variable.name
            ))
        })?;
        let width = value_type.width();
        let offset = if self.dimensions[variable.dimensions[0]].length.is_none() {
            if self.record_stride == 0 {
                return Err(Diagnostic::new(
                    "NetCDF record variables have no record layout.",
                ));
            }
            None
        } else {
            Some(variable.begin)
        };
        (0..length)
            .map(|index| {
                let start = match offset {
                    Some(first) => first.checked_add(
                        index
                            .checked_mul(width)
                            .ok_or_else(|| Diagnostic::new("NetCDF value offset overflows."))?,
                    ),
                    None => variable.begin.checked_add(
                        index
                            .checked_mul(self.record_stride)
                            .ok_or_else(|| Diagnostic::new("NetCDF record offset overflows."))?,
                    ),
                }
                .ok_or_else(|| Diagnostic::new("NetCDF value offset overflows."))?;
                let end = start
                    .checked_add(width)
                    .ok_or_else(|| Diagnostic::new("NetCDF value range overflows."))?;
                let bytes = self.bytes.get(start..end).ok_or_else(|| {
                    Diagnostic::new(format!(
                        "NetCDF variable {:?} ends before its declared data.",
                        variable.name
                    ))
                })?;
                value_type.value(bytes)
            })
            .collect()
    }
}

/// Lists browser-plot-compatible variables in a CDF-1 or CDF-2 file.
pub(crate) fn variables(bytes: &[u8]) -> Result<Vec<VariableInfo>, Diagnostic> {
    File::parse(bytes)?.info()
}

/// Reads a one-dimensional Y variable and an optional matching X variable.
///
/// When no X variable is selected, X uses Grace's one-based point index.
pub(crate) fn read_xy(bytes: &[u8], x: Option<&str>, y: &str) -> Result<Vec<Vec<f64>>, Diagnostic> {
    let file = File::parse(bytes)?;
    let y = file.values(file.variable(y)?)?;
    let x = match x {
        Some(name) => {
            let x = file.values(file.variable(name)?)?;
            if x.len() != y.len() {
                return Err(Diagnostic::new(
                    "The NetCDF X and Y variables must have the same length.",
                ));
            }
            x
        }
        None => (1..=y.len()).map(|index| index as f64).collect(),
    };
    Ok(x.into_iter().zip(y).map(|(x, y)| vec![x, y]).collect())
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8], cursor: usize) -> Self {
        Self { bytes, cursor }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], Diagnostic> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or_else(|| Diagnostic::new("NetCDF header is too large."))?;
        let result = self
            .bytes
            .get(self.cursor..end)
            .ok_or_else(|| Diagnostic::new("NetCDF header ends unexpectedly."))?;
        self.cursor = end;
        Ok(result)
    }

    fn u32(&mut self) -> Result<u32, Diagnostic> {
        Ok(u32::from_be_bytes(
            self.take(4)?.try_into().expect("fixed width"),
        ))
    }

    fn u64(&mut self) -> Result<u64, Diagnostic> {
        Ok(u64::from_be_bytes(
            self.take(8)?.try_into().expect("fixed width"),
        ))
    }

    fn string(&mut self) -> Result<String, Diagnostic> {
        let length = self.u32()? as usize;
        if length > self.bytes.len() || length > 1_048_576 {
            return Err(Diagnostic::new("NetCDF name is too large."));
        }
        let bytes = self.take(length)?.to_vec();
        self.pad(length)?;
        std::str::from_utf8(&bytes)
            .map(str::to_owned)
            .map_err(|_| Diagnostic::new("NetCDF names must use UTF-8."))
    }

    fn pad(&mut self, length: usize) -> Result<(), Diagnostic> {
        let padding = (4 - length % 4) % 4;
        self.take(padding).map(|_| ())
    }

    fn list_count(&mut self, expected: u32) -> Result<Option<usize>, Diagnostic> {
        let tag = self.u32()?;
        let count = self.u32()? as usize;
        if tag == 0 && count == 0 {
            return Ok(None);
        }
        if tag != expected || count > MAX_ITEMS {
            return Err(Diagnostic::new(
                "NetCDF header has an unsupported or oversized list.",
            ));
        }
        Ok(Some(count))
    }
}

fn read_dimensions(reader: &mut Reader<'_>) -> Result<Vec<Dimension>, Diagnostic> {
    let Some(count) = reader.list_count(DIMENSION)? else {
        return Ok(Vec::new());
    };
    let mut dimensions = Vec::with_capacity(count);
    for _ in 0..count {
        let _name = reader.string()?;
        let length = reader.u32()? as usize;
        dimensions.push(Dimension {
            length: (length != 0).then_some(length),
        });
    }
    if dimensions
        .iter()
        .filter(|dimension| dimension.length.is_none())
        .count()
        > 1
    {
        return Err(Diagnostic::new(
            "NetCDF supports only one unlimited dimension.",
        ));
    }
    Ok(dimensions)
}

fn skip_attributes(reader: &mut Reader<'_>) -> Result<(), Diagnostic> {
    let Some(count) = reader.list_count(ATTRIBUTE)? else {
        return Ok(());
    };
    for _ in 0..count {
        let _name = reader.string()?;
        let value_type = reader.u32()?;
        let count = reader.u32()? as usize;
        let length = count
            .checked_mul(attribute_width(value_type)?)
            .ok_or_else(|| Diagnostic::new("NetCDF attribute is too large."))?;
        reader.take(length)?;
        reader.pad(length)?;
    }
    Ok(())
}

fn attribute_width(type_code: u32) -> Result<usize, Diagnostic> {
    match type_code {
        1 | 2 => Ok(1),
        3 => Ok(2),
        4 | 5 => Ok(4),
        6 => Ok(8),
        _ => Err(Diagnostic::new(
            "NetCDF attribute uses an unsupported data type.",
        )),
    }
}

fn read_variables(
    reader: &mut Reader<'_>,
    offset_width: usize,
    dimensions: &[Dimension],
) -> Result<Vec<Variable>, Diagnostic> {
    let Some(count) = reader.list_count(VARIABLE)? else {
        return Ok(Vec::new());
    };
    let mut variables = Vec::with_capacity(count);
    for _ in 0..count {
        let name = reader.string()?;
        let dimensions_count = reader.u32()? as usize;
        if dimensions_count > MAX_ITEMS {
            return Err(Diagnostic::new("NetCDF variable has too many dimensions."));
        }
        let mut variable_dimensions = Vec::with_capacity(dimensions_count);
        for _ in 0..dimensions_count {
            let dimension = reader.u32()? as usize;
            if dimension >= dimensions.len() {
                return Err(Diagnostic::new(
                    "NetCDF variable references an unknown dimension.",
                ));
            }
            variable_dimensions.push(dimension);
        }
        skip_attributes(reader)?;
        let type_code = reader.u32()?;
        let value_type = ValueType::parse(type_code).ok();
        let vsize = reader.u32()? as usize;
        let begin = match offset_width {
            4 => reader.u32()? as u64,
            8 => reader.u64()?,
            _ => unreachable!("validated magic controls offset width"),
        };
        let begin = usize::try_from(begin)
            .map_err(|_| Diagnostic::new("NetCDF data offset does not fit this browser."))?;
        if variables
            .iter()
            .any(|variable: &Variable| variable.name == name)
        {
            return Err(Diagnostic::new(format!(
                "NetCDF has duplicate variable {name:?}."
            )));
        }
        variables.push(Variable {
            name,
            dimensions: variable_dimensions,
            value_type,
            begin,
            vsize,
        });
    }
    Ok(variables)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(bytes: &mut Vec<u8>) {
        while !bytes.len().is_multiple_of(4) {
            bytes.push(0);
        }
    }

    fn string(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend_from_slice(&(value.len() as u32).to_be_bytes());
        bytes.extend_from_slice(value.as_bytes());
        pad(bytes);
    }

    fn fixture() -> Vec<u8> {
        let mut bytes = b"CDF\x01".to_vec();
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&DIMENSION.to_be_bytes());
        bytes.extend_from_slice(&1u32.to_be_bytes());
        string(&mut bytes, "point");
        bytes.extend_from_slice(&3u32.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&VARIABLE.to_be_bytes());
        bytes.extend_from_slice(&2u32.to_be_bytes());
        let mut begins = Vec::new();
        for name in ["time", "signal"] {
            string(&mut bytes, name);
            bytes.extend_from_slice(&1u32.to_be_bytes());
            bytes.extend_from_slice(&0u32.to_be_bytes());
            bytes.extend_from_slice(&0u32.to_be_bytes());
            bytes.extend_from_slice(&0u32.to_be_bytes());
            bytes.extend_from_slice(&4u32.to_be_bytes());
            bytes.extend_from_slice(&12u32.to_be_bytes());
            begins.push(bytes.len());
            bytes.extend_from_slice(&0u32.to_be_bytes());
        }
        let data_start = bytes.len();
        bytes[begins[0]..begins[0] + 4].copy_from_slice(&(data_start as u32).to_be_bytes());
        bytes[begins[1]..begins[1] + 4].copy_from_slice(&((data_start + 12) as u32).to_be_bytes());
        for value in [0i32, 1, 2] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        for value in [2i32, 4, 8] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        bytes
    }

    fn record_fixture() -> Vec<u8> {
        let mut bytes = b"CDF\x01".to_vec();
        bytes.extend_from_slice(&3u32.to_be_bytes());
        bytes.extend_from_slice(&DIMENSION.to_be_bytes());
        bytes.extend_from_slice(&1u32.to_be_bytes());
        string(&mut bytes, "record");
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&VARIABLE.to_be_bytes());
        bytes.extend_from_slice(&2u32.to_be_bytes());
        let mut begins = Vec::new();
        for name in ["x", "y"] {
            string(&mut bytes, name);
            bytes.extend_from_slice(&1u32.to_be_bytes());
            bytes.extend_from_slice(&0u32.to_be_bytes());
            bytes.extend_from_slice(&0u32.to_be_bytes());
            bytes.extend_from_slice(&0u32.to_be_bytes());
            bytes.extend_from_slice(&4u32.to_be_bytes());
            bytes.extend_from_slice(&4u32.to_be_bytes());
            begins.push(bytes.len());
            bytes.extend_from_slice(&0u32.to_be_bytes());
        }
        let data_start = bytes.len();
        bytes[begins[0]..begins[0] + 4].copy_from_slice(&(data_start as u32).to_be_bytes());
        bytes[begins[1]..begins[1] + 4].copy_from_slice(&((data_start + 4) as u32).to_be_bytes());
        for (x, y) in [(10i32, 1i32), (20, 4), (30, 9)] {
            bytes.extend_from_slice(&x.to_be_bytes());
            bytes.extend_from_slice(&y.to_be_bytes());
        }
        bytes
    }

    fn cdf2_fixture() -> Vec<u8> {
        let mut bytes = b"CDF\x02".to_vec();
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&DIMENSION.to_be_bytes());
        bytes.extend_from_slice(&1u32.to_be_bytes());
        string(&mut bytes, "point");
        bytes.extend_from_slice(&2u32.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&VARIABLE.to_be_bytes());
        bytes.extend_from_slice(&1u32.to_be_bytes());
        string(&mut bytes, "signal");
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&6u32.to_be_bytes());
        bytes.extend_from_slice(&16u32.to_be_bytes());
        let begin = bytes.len();
        bytes.extend_from_slice(&0u64.to_be_bytes());
        let data_start = bytes.len();
        bytes[begin..begin + 8].copy_from_slice(&(data_start as u64).to_be_bytes());
        for value in [1.5f64, 2.5] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        bytes
    }

    #[test]
    fn lists_and_reads_a_cdf1_xy_pair() {
        let bytes = fixture();
        assert_eq!(
            variables(&bytes).unwrap(),
            vec![
                VariableInfo {
                    name: "time".into(),
                    length: 3
                },
                VariableInfo {
                    name: "signal".into(),
                    length: 3
                },
            ]
        );
        assert_eq!(
            read_xy(&bytes, Some("time"), "signal").unwrap(),
            vec![vec![0., 2.], vec![1., 4.], vec![2., 8.]]
        );
        assert_eq!(
            read_xy(&bytes, None, "signal").unwrap(),
            vec![vec![1., 2.], vec![2., 4.], vec![3., 8.]]
        );
    }

    #[test]
    fn rejects_non_classic_files() {
        assert!(variables(b"\x89HDF\r\n\x1a\n").is_err());
    }

    #[test]
    fn reads_interleaved_unlimited_record_variables() {
        assert_eq!(
            read_xy(&record_fixture(), Some("x"), "y").unwrap(),
            vec![vec![10., 1.], vec![20., 4.], vec![30., 9.]]
        );
    }

    #[test]
    fn reads_cdf2_offsets() {
        assert_eq!(
            read_xy(&cdf2_fixture(), None, "signal").unwrap(),
            vec![vec![1., 1.5], vec![2., 2.5]]
        );
    }
}
