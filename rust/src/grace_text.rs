//! Translate the useful Grace text escapes into SVG text spans.
//!
//! Grace strings are saved in AGR files, so this parser runs in the portable Rust
//! core. It keeps malformed escapes readable while supporting scientific labels.

#[derive(Clone, Default)]
struct Style {
    font: Option<String>,
    scale: f64,
    shift: f64,
    oblique: bool,
}

impl Style {
    fn normal() -> Self {
        Self {
            scale: 1.,
            ..Self::default()
        }
    }
}

fn xml(text: &str) -> String {
    text.chars()
        .filter(|&ch| ch == '\t' || ch == '\n' || ch >= ' ')
        .fold(String::new(), |mut escaped, ch| {
            match ch {
                '&' => escaped.push_str("&amp;"),
                '<' => escaped.push_str("&lt;"),
                '>' => escaped.push_str("&gt;"),
                '"' => escaped.push_str("&quot;"),
                '\'' => escaped.push_str("&apos;"),
                _ => escaped.push(ch),
            }
            escaped
        })
}

fn symbol(character: char) -> Option<char> {
    // Write Symbol glyphs as escapes while emitting their Unicode values in SVG.
    Some(match character {
        'A' => '\u{391}',
        'B' => '\u{392}',
        'C' => '\u{3a7}',
        'D' => '\u{394}',
        'E' => '\u{395}',
        'F' => '\u{3a6}',
        'G' => '\u{393}',
        'H' => '\u{397}',
        'I' => '\u{399}',
        'J' => '\u{3d1}',
        'K' => '\u{39a}',
        'L' => '\u{39b}',
        'M' => '\u{39c}',
        'N' => '\u{39d}',
        'O' => '\u{39f}',
        'P' => '\u{3a0}',
        'Q' => '\u{398}',
        'R' => '\u{3a1}',
        'S' => '\u{3a3}',
        'T' => '\u{3a4}',
        'U' => '\u{3a5}',
        'V' => '\u{3c2}',
        'W' => '\u{3a9}',
        'X' => '\u{39e}',
        'Y' => '\u{3a8}',
        'Z' => '\u{396}',
        'a' => '\u{3b1}',
        'b' => '\u{3b2}',
        'c' => '\u{3c7}',
        'd' => '\u{3b4}',
        'e' => '\u{3b5}',
        'f' => '\u{3c6}',
        'g' => '\u{3b3}',
        'h' => '\u{3b7}',
        'i' => '\u{3b9}',
        'j' => '\u{3d5}',
        'k' => '\u{3ba}',
        'l' => '\u{3bb}',
        'm' => '\u{3bc}',
        'n' => '\u{3bd}',
        'o' => '\u{3bf}',
        'p' => '\u{3c0}',
        'q' => '\u{3b8}',
        'r' => '\u{3c1}',
        's' => '\u{3c3}',
        't' => '\u{3c4}',
        'u' => '\u{3c5}',
        'v' => '\u{3d6}',
        'w' => '\u{3c9}',
        'x' => '\u{3be}',
        'y' => '\u{3c8}',
        'z' => '\u{3b6}',
        _ => return None,
    })
}

fn font_name(name: &str) -> Option<String> {
    let font = name.trim();
    if font.is_empty() || font.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    if font.eq_ignore_ascii_case("symbol") {
        return Some("serif".into());
    }
    Some(font.to_owned())
}

fn push_span(output: &mut String, text: &mut String, style: &Style) {
    if text.is_empty() {
        return;
    }
    let mut attributes = String::new();
    if let Some(font) = &style.font {
        attributes.push_str(" font-family=\"");
        attributes.push_str(&xml(font));
        attributes.push('"');
    }
    if (style.scale - 1.).abs() > f64::EPSILON {
        attributes.push_str(&format!(" font-size=\"{:.6}em\"", style.scale));
    }
    if style.shift != 0. {
        attributes.push_str(&format!(" baseline-shift=\"{:.6}em\"", style.shift));
    }
    if style.oblique {
        attributes.push_str(" font-style=\"italic\"");
    }
    output.push_str("<tspan");
    output.push_str(&attributes);
    output.push('>');
    output.push_str(&xml(text));
    output.push_str("</tspan>");
    text.clear();
}

fn braced(input: &[char], cursor: &mut usize) -> Option<String> {
    if input.get(*cursor) != Some(&'{') {
        return None;
    }
    let start = *cursor + 1;
    let end = input[start..].iter().position(|&ch| ch == '}')? + start;
    *cursor = end + 1;
    Some(input[start..end].iter().collect())
}

fn number(input: Option<String>) -> Option<f64> {
    input?
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

/// Returns SVG span content for a Grace label.
///
/// Supported escapes include font selection, Symbol characters, scaling, baseline shifts,
/// oblique type, newlines, literal backslashes, and hexadecimal character lists.
pub(crate) fn svg_runs(label: &str) -> String {
    let input: Vec<_> = label.chars().collect();
    let mut output = String::new();
    let mut text = String::new();
    let mut style = Style::normal();
    let mut symbol_mode = false;
    let mut cursor = 0;

    while cursor < input.len() {
        let character = input[cursor];
        cursor += 1;
        if character != '\\' {
            text.push(if symbol_mode {
                symbol(character).unwrap_or(character)
            } else {
                character
            });
            continue;
        }
        let Some(code) = input.get(cursor).copied() else {
            text.push('\\');
            break;
        };
        cursor += 1;
        match code {
            '\\' => text.push('\\'),
            '"' => text.push('"'),
            'x' => {
                push_span(&mut output, &mut text, &style);
                style.font = Some("serif".into());
                symbol_mode = true;
            }
            'f' => {
                let Some(argument) = braced(&input, &mut cursor) else {
                    text.push_str("\\f");
                    continue;
                };
                push_span(&mut output, &mut text, &style);
                style.font = font_name(&argument);
                symbol_mode = argument.eq_ignore_ascii_case("symbol");
            }
            'z' | 'Z' => {
                let Some(factor) = number(braced(&input, &mut cursor)) else {
                    text.push('\\');
                    text.push(code);
                    continue;
                };
                push_span(&mut output, &mut text, &style);
                style.scale = if code == 'Z' {
                    factor
                } else {
                    style.scale * factor
                };
                if !style.scale.is_finite() || style.scale <= 0. {
                    style.scale = 1.;
                }
            }
            '+' => {
                push_span(&mut output, &mut text, &style);
                style.scale *= 1.19;
            }
            '-' => {
                push_span(&mut output, &mut text, &style);
                style.scale *= 0.84;
            }
            'v' => {
                let Some(shift) = number(braced(&input, &mut cursor)) else {
                    text.push_str("\\v");
                    continue;
                };
                push_span(&mut output, &mut text, &style);
                style.shift = shift;
            }
            's' => {
                push_span(&mut output, &mut text, &style);
                style.shift = -0.4;
                style.scale = 0.71;
            }
            'S' => {
                push_span(&mut output, &mut text, &style);
                style.shift = 0.6;
                style.scale = 0.71;
            }
            'N' => {
                push_span(&mut output, &mut text, &style);
                style.shift = 0.;
                style.scale = 1.;
            }
            'q' => {
                push_span(&mut output, &mut text, &style);
                style.oblique = true;
            }
            'Q' => {
                push_span(&mut output, &mut text, &style);
                style.oblique = false;
            }
            'n' => {
                push_span(&mut output, &mut text, &style);
                output.push_str("<tspan dy=\"1.2em\">&#10;</tspan>");
            }
            '#' => {
                let Some(hex) = braced(&input, &mut cursor) else {
                    text.push_str("\\#");
                    continue;
                };
                if hex.len() % 2 == 0 && hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
                    for index in (0..hex.len()).step_by(2) {
                        if let Ok(value) = u8::from_str_radix(&hex[index..index + 2], 16) {
                            text.push(char::from(value));
                        }
                    }
                } else {
                    text.push_str("\\#{");
                    text.push_str(&hex);
                    text.push('}');
                }
            }
            _ => {
                text.push('\\');
                text.push(code);
            }
        }
    }
    push_span(&mut output, &mut text, &style);
    output
}

#[cfg(test)]
mod tests {
    use super::svg_runs;

    #[test]
    fn escapes_xml_text_and_font_attributes() {
        assert_eq!(
            svg_runs(r#"<&\f{A"B}>"#),
            "<tspan>&lt;&amp;</tspan><tspan font-family=\"A&quot;B\">&gt;</tspan>"
        );
    }

    #[test]
    fn renders_classic_scientific_example() {
        let output = svg_runs(r"F\sX\N(\xe\f{})");
        assert!(output.contains("baseline-shift=\"-0.400000em\""));
        assert!(output.contains('\u{3b5}'));
        assert!(output.contains("font-family=\"serif\""));
    }

    #[test]
    fn parses_hex_and_keeps_unknown_escapes_readable() {
        assert_eq!(svg_runs(r"a\#{b7}b\wat"), "<tspan>a\u{b7}b\\wat</tspan>");
    }

    #[test]
    fn supports_absolute_and_relative_scaling() {
        let output = svg_runs(r"a\z{2}b\Z{3}c");
        assert!(output.contains("font-size=\"2.000000em\""));
        assert!(output.contains("font-size=\"3.000000em\""));
    }
}
