//! A small, deterministic scalar expression language for generated data.
//!
//! The parser deliberately owns no browser or JavaScript state.  Callers supply
//! the variables available to a formula and evaluate the resulting expression
//! for each sample point.

use crate::Diagnostic;

const MAX_FORMULA_BYTES: usize = 2048;
const MAX_NODES: usize = 256;
const MAX_DEPTH: usize = 64;

#[derive(Debug)]
pub(crate) struct Expression {
    tree: Node,
    variables: usize,
}

#[derive(Debug)]
enum Node {
    Number(f64),
    Variable(usize),
    Unary(Unary, Box<Node>),
    Binary(Binary, Box<Node>, Box<Node>),
    Conditional(Box<Node>, Box<Node>, Box<Node>),
    Function(Function, Vec<Node>),
}

#[derive(Debug, Clone, Copy)]
enum Unary {
    Plus,
    Minus,
}

#[derive(Debug, Clone, Copy)]
enum Binary {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Remainder,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}

#[derive(Debug, Clone, Copy)]
enum Function {
    Abs,
    Acos,
    Acosh,
    Asin,
    Asinh,
    Atan,
    Atan2,
    Atanh,
    Ceil,
    Cos,
    Cosh,
    Exp,
    Floor,
    Ln,
    Log10,
    Max,
    Min,
    Pow,
    Rint,
    Sign,
    Sin,
    Sinh,
    Sqr,
    Sqrt,
    Tan,
    Tanh,
}

impl Function {
    fn parse(name: &str) -> Option<(Self, usize)> {
        let function = match name {
            "abs" => (Self::Abs, 1),
            "acos" => (Self::Acos, 1),
            "acosh" => (Self::Acosh, 1),
            "asin" => (Self::Asin, 1),
            "asinh" => (Self::Asinh, 1),
            "atan" => (Self::Atan, 1),
            "atan2" => (Self::Atan2, 2),
            "atanh" => (Self::Atanh, 1),
            "ceil" => (Self::Ceil, 1),
            "cos" => (Self::Cos, 1),
            "cosh" => (Self::Cosh, 1),
            "exp" => (Self::Exp, 1),
            "floor" => (Self::Floor, 1),
            "ln" | "log" => (Self::Ln, 1),
            "log10" => (Self::Log10, 1),
            "max" | "maxof" => (Self::Max, 2),
            "min" | "minof" => (Self::Min, 2),
            "pow" => (Self::Pow, 2),
            "rint" | "round" => (Self::Rint, 1),
            "sgn" | "sign" => (Self::Sign, 1),
            "sin" => (Self::Sin, 1),
            "sinh" => (Self::Sinh, 1),
            "sqr" => (Self::Sqr, 1),
            "sqrt" => (Self::Sqrt, 1),
            "tan" => (Self::Tan, 1),
            "tanh" => (Self::Tanh, 1),
            _ => return None,
        };
        Some(function)
    }

    fn evaluate(self, values: &[f64]) -> f64 {
        match self {
            Self::Abs => values[0].abs(),
            Self::Acos => values[0].acos(),
            Self::Acosh => values[0].acosh(),
            Self::Asin => values[0].asin(),
            Self::Asinh => values[0].asinh(),
            Self::Atan => values[0].atan(),
            Self::Atan2 => values[0].atan2(values[1]),
            Self::Atanh => values[0].atanh(),
            Self::Ceil => values[0].ceil(),
            Self::Cos => values[0].cos(),
            Self::Cosh => values[0].cosh(),
            Self::Exp => values[0].exp(),
            Self::Floor => values[0].floor(),
            Self::Ln => values[0].ln(),
            Self::Log10 => values[0].log10(),
            Self::Max => values[0].max(values[1]),
            Self::Min => values[0].min(values[1]),
            Self::Pow => values[0].powf(values[1]),
            Self::Rint => values[0].round(),
            Self::Sign => values[0].signum(),
            Self::Sin => values[0].sin(),
            Self::Sinh => values[0].sinh(),
            Self::Sqr => values[0] * values[0],
            Self::Sqrt => values[0].sqrt(),
            Self::Tan => values[0].tan(),
            Self::Tanh => values[0].tanh(),
        }
    }
}

impl Expression {
    pub(crate) fn parse(input: &str, variables: &[&str]) -> Result<Self, Diagnostic> {
        if input.trim().is_empty() {
            return Err(Diagnostic::new("Enter a formula."));
        }
        if input.len() > MAX_FORMULA_BYTES || !input.is_ascii() {
            return Err(Diagnostic::new(
                "Formula must be ASCII and at most 2048 characters.",
            ));
        }
        let names: Vec<String> = variables
            .iter()
            .map(|name| name.to_ascii_lowercase())
            .collect();
        if names.iter().any(|name| !is_identifier(name))
            || names
                .iter()
                .enumerate()
                .any(|(index, name)| names[..index].contains(name))
        {
            return Err(Diagnostic::new(
                "Formula variables must have unique valid names.",
            ));
        }
        let mut parser = Parser {
            input: input.as_bytes().to_vec(),
            position: 0,
            variables: names,
            nodes: 0,
            depth: 0,
        };
        let tree = parser.expression()?;
        parser.skip_space();
        if parser.position != parser.input.len() {
            return Err(parser.error("Unexpected text in formula."));
        }
        Ok(Self {
            tree,
            variables: variables.len(),
        })
    }

    pub(crate) fn evaluate(&self, values: &[f64]) -> Result<f64, Diagnostic> {
        if values.len() != self.variables {
            return Err(Diagnostic::new("Formula received invalid variable values."));
        }
        let value = self.tree.evaluate(values)?;
        value
            .is_finite()
            .then_some(value)
            .ok_or_else(|| Diagnostic::new("Formula produced a non-finite value."))
    }
}

impl Node {
    fn evaluate(&self, variables: &[f64]) -> Result<f64, Diagnostic> {
        let value = match self {
            Self::Number(value) => *value,
            Self::Variable(index) => variables[*index],
            Self::Unary(operator, value) => match operator {
                Unary::Plus => value.evaluate(variables)?,
                Unary::Minus => -value.evaluate(variables)?,
            },
            Self::Binary(operator, left, right) => {
                let left = left.evaluate(variables)?;
                if matches!(operator, Binary::And) {
                    if !truthy(left) {
                        0.
                    } else {
                        boolean(truthy(right.evaluate(variables)?))
                    }
                } else if matches!(operator, Binary::Or) {
                    if truthy(left) {
                        1.
                    } else {
                        boolean(truthy(right.evaluate(variables)?))
                    }
                } else {
                    let right = right.evaluate(variables)?;
                    match operator {
                        Binary::Add => left + right,
                        Binary::Subtract => left - right,
                        Binary::Multiply => left * right,
                        Binary::Divide => left / right,
                        Binary::Power => left.powf(right),
                        Binary::Remainder => left % right,
                        Binary::Equal => boolean(left == right),
                        Binary::NotEqual => boolean(left != right),
                        Binary::Less => boolean(left < right),
                        Binary::LessEqual => boolean(left <= right),
                        Binary::Greater => boolean(left > right),
                        Binary::GreaterEqual => boolean(left >= right),
                        Binary::And | Binary::Or => unreachable!("handled before right evaluation"),
                    }
                }
            }
            Self::Conditional(condition, yes, no) => {
                if truthy(condition.evaluate(variables)?) {
                    yes.evaluate(variables)?
                } else {
                    no.evaluate(variables)?
                }
            }
            Self::Function(function, arguments) => {
                let values: Result<Vec<_>, _> = arguments
                    .iter()
                    .map(|argument| argument.evaluate(variables))
                    .collect();
                function.evaluate(&values?)
            }
        };
        value
            .is_finite()
            .then_some(value)
            .ok_or_else(|| Diagnostic::new("Formula produced a non-finite value."))
    }
}

fn truthy(value: f64) -> bool {
    value != 0.
}

fn boolean(value: bool) -> f64 {
    if value { 1. } else { 0. }
}

struct Parser {
    input: Vec<u8>,
    position: usize,
    variables: Vec<String>,
    nodes: usize,
    depth: usize,
}

impl Parser {
    fn expression(&mut self) -> Result<Node, Diagnostic> {
        self.with_depth(Self::conditional)
    }

    fn conditional(&mut self) -> Result<Node, Diagnostic> {
        let condition = self.logical_or()?;
        if !self.consume(b'?') {
            return Ok(condition);
        }
        let yes = self.expression()?;
        if !self.consume(b':') {
            return Err(self.error("Expected ':' in conditional formula."));
        }
        let no = self.conditional()?;
        self.node(Node::Conditional(
            Box::new(condition),
            Box::new(yes),
            Box::new(no),
        ))
    }

    fn logical_or(&mut self) -> Result<Node, Diagnostic> {
        let mut node = self.logical_and()?;
        while self.consume_bytes(b"||") {
            let right = self.logical_and()?;
            node = self.node(Node::Binary(Binary::Or, Box::new(node), Box::new(right)))?;
        }
        Ok(node)
    }

    fn logical_and(&mut self) -> Result<Node, Diagnostic> {
        let mut node = self.comparison()?;
        while self.consume_bytes(b"&&") {
            let right = self.comparison()?;
            node = self.node(Node::Binary(Binary::And, Box::new(node), Box::new(right)))?;
        }
        Ok(node)
    }

    fn comparison(&mut self) -> Result<Node, Diagnostic> {
        let left = self.sum()?;
        let operator = if self.consume_bytes(b"<=") || self.consume_keyword("le") {
            Some(Binary::LessEqual)
        } else if self.consume_bytes(b">=") || self.consume_keyword("ge") {
            Some(Binary::GreaterEqual)
        } else if self.consume_bytes(b"==") || self.consume_keyword("eq") {
            Some(Binary::Equal)
        } else if self.consume_bytes(b"!=") || self.consume_keyword("ne") {
            Some(Binary::NotEqual)
        } else if self.consume(b'<') || self.consume_keyword("lt") {
            Some(Binary::Less)
        } else if self.consume(b'>') || self.consume_keyword("gt") {
            Some(Binary::Greater)
        } else {
            None
        };
        let Some(operator) = operator else {
            return Ok(left);
        };
        let right = self.sum()?;
        self.node(Node::Binary(operator, Box::new(left), Box::new(right)))
    }

    fn sum(&mut self) -> Result<Node, Diagnostic> {
        let mut node = self.product()?;
        loop {
            let operator = if self.consume(b'+') {
                Some(Binary::Add)
            } else if self.consume(b'-') {
                Some(Binary::Subtract)
            } else {
                None
            };
            let Some(operator) = operator else {
                return Ok(node);
            };
            let right = self.product()?;
            node = self.node(Node::Binary(operator, Box::new(node), Box::new(right)))?;
        }
    }

    fn product(&mut self) -> Result<Node, Diagnostic> {
        let mut node = self.unary()?;
        loop {
            let operator = if self.consume(b'*') {
                Some(Binary::Multiply)
            } else if self.consume(b'/') {
                Some(Binary::Divide)
            } else if self.consume(b'%') {
                Some(Binary::Remainder)
            } else {
                None
            };
            let Some(operator) = operator else {
                return Ok(node);
            };
            let right = self.unary()?;
            node = self.node(Node::Binary(operator, Box::new(node), Box::new(right)))?;
        }
    }

    fn unary(&mut self) -> Result<Node, Diagnostic> {
        if self.consume(b'+') {
            let value = self.unary()?;
            return self.node(Node::Unary(Unary::Plus, Box::new(value)));
        }
        if self.consume(b'-') {
            let value = self.unary()?;
            return self.node(Node::Unary(Unary::Minus, Box::new(value)));
        }
        self.power()
    }

    fn power(&mut self) -> Result<Node, Diagnostic> {
        let left = self.atom()?;
        if self.consume(b'^') {
            let right = self.unary()?;
            return self.node(Node::Binary(Binary::Power, Box::new(left), Box::new(right)));
        }
        Ok(left)
    }

    fn atom(&mut self) -> Result<Node, Diagnostic> {
        self.skip_space();
        if self.consume(b'(') {
            let node = self.expression()?;
            if !self.consume(b')') {
                return Err(self.error("Expected ')' in formula."));
            }
            return Ok(node);
        }
        if self
            .peek()
            .is_some_and(|byte| byte.is_ascii_digit() || byte == b'.')
        {
            return self.number();
        }
        if self.peek().is_some_and(is_identifier_start) {
            return self.identifier();
        }
        Err(self.error("Expected a number, variable, function, or '('."))
    }

    fn number(&mut self) -> Result<Node, Diagnostic> {
        self.skip_space();
        let start = self.position;
        let mut digits = 0;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.position += 1;
            digits += 1;
        }
        if self.peek() == Some(b'.') {
            self.position += 1;
            while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                self.position += 1;
                digits += 1;
            }
        }
        if digits == 0 {
            return Err(self.error("Invalid number in formula."));
        }
        if self
            .peek()
            .is_some_and(|byte| matches!(byte, b'e' | b'E' | b'd' | b'D'))
        {
            self.position += 1;
            if self.peek().is_some_and(|byte| matches!(byte, b'+' | b'-')) {
                self.position += 1;
            }
            let exponent = self.position;
            while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                self.position += 1;
            }
            if exponent == self.position {
                return Err(self.error("Invalid exponent in formula."));
            }
        }
        let text = std::str::from_utf8(&self.input[start..self.position])
            .expect("formula was checked as ASCII")
            .replace(['d', 'D'], "e");
        let value = text
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .ok_or_else(|| self.error("Invalid number in formula."))?;
        self.node(Node::Number(value))
    }

    fn identifier(&mut self) -> Result<Node, Diagnostic> {
        self.skip_space();
        let start = self.position;
        self.position += 1;
        while self.peek().is_some_and(is_identifier_continue) {
            self.position += 1;
        }
        let name = std::str::from_utf8(&self.input[start..self.position])
            .expect("formula was checked as ASCII")
            .to_ascii_lowercase();
        if self.consume(b'(') {
            let (function, arity) = Function::parse(&name)
                .ok_or_else(|| self.error(&format!("Unknown formula function '{name}'.")))?;
            let mut arguments = Vec::new();
            if !self.consume(b')') {
                loop {
                    arguments.push(self.expression()?);
                    if self.consume(b')') {
                        break;
                    }
                    if !self.consume(b',') {
                        return Err(self.error("Expected ',' or ')' in function call."));
                    }
                }
            }
            if arguments.len() != arity {
                return Err(self.error(&format!("Function '{name}' needs {arity} arguments.")));
            }
            return self.node(Node::Function(function, arguments));
        }
        if name == "pi" {
            return self.node(Node::Number(std::f64::consts::PI));
        }
        if name == "e" {
            return self.node(Node::Number(std::f64::consts::E));
        }
        let index = self
            .variables
            .iter()
            .position(|variable| variable == &name)
            .ok_or_else(|| self.error(&format!("Unknown formula variable '{name}'.")))?;
        self.node(Node::Variable(index))
    }

    fn with_depth(
        &mut self,
        parser: fn(&mut Self) -> Result<Node, Diagnostic>,
    ) -> Result<Node, Diagnostic> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.error("Formula nesting is too deep."));
        }
        let result = parser(self);
        self.depth -= 1;
        result
    }

    fn node(&mut self, node: Node) -> Result<Node, Diagnostic> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err(self.error("Formula is too complex."));
        }
        Ok(node)
    }

    fn consume(&mut self, expected: u8) -> bool {
        self.skip_space();
        if self.peek() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn consume_bytes(&mut self, expected: &[u8]) -> bool {
        self.skip_space();
        if self.input[self.position..].starts_with(expected) {
            self.position += expected.len();
            true
        } else {
            false
        }
    }

    fn consume_keyword(&mut self, expected: &str) -> bool {
        self.skip_space();
        let end = self.position + expected.len();
        let Some(candidate) = self.input.get(self.position..end) else {
            return false;
        };
        if !candidate.eq_ignore_ascii_case(expected.as_bytes())
            || self
                .input
                .get(end)
                .copied()
                .is_some_and(is_identifier_continue)
        {
            return false;
        }
        self.position = end;
        true
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.position).copied()
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.position += 1;
        }
    }

    fn error(&self, message: &str) -> Diagnostic {
        Diagnostic::new(format!("{message} At character {}.", self.position + 1))
    }
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'$' || byte == b'_'
}

fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit() || byte == b'.'
}

fn is_identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(is_identifier_start) && bytes.all(is_identifier_continue)
}

#[cfg(test)]
mod tests {
    use super::Expression;

    fn evaluate(formula: &str, values: &[f64]) -> f64 {
        Expression::parse(formula, &["$t", "x"])
            .unwrap()
            .evaluate(values)
            .unwrap()
    }

    #[test]
    fn evaluates_a_sine_wave_formula() {
        assert!(
            (evaluate("sin($T) + cos(x)", &[std::f64::consts::FRAC_PI_2, 0.]) - 2.).abs() < 1e-12
        );
    }

    #[test]
    fn preserves_mathematical_power_precedence() {
        assert_eq!(evaluate("-2^2", &[0., 0.]), -4.);
        assert_eq!(evaluate("2^-2", &[0., 0.]), 0.25);
        assert_eq!(evaluate("2^3^2", &[0., 0.]), 512.);
    }

    #[test]
    fn accepts_grace_style_numbers_and_scalar_functions() {
        assert_eq!(evaluate("sqr(3d0) + maxof(2, min(5, 4))", &[0., 0.]), 13.);
    }

    #[test]
    fn evaluates_comparisons_with_symbolic_and_grace_operators() {
        assert_eq!(evaluate("x >= 3", &[0., 3.]), 1.);
        assert_eq!(evaluate("x LT 3", &[0., 3.]), 0.);
        assert_eq!(evaluate("$t ne x", &[2., 2.]), 0.);
    }

    #[test]
    fn short_circuits_boolean_conditions_and_formula_branches() {
        assert_eq!(evaluate("0 && 1 / 0", &[0., 0.]), 0.);
        assert_eq!(evaluate("1 || 1 / 0", &[0., 0.]), 1.);
        assert_eq!(evaluate("x > 0 ? sqrt(x) : 0", &[0., -1.]), 0.);
        assert_eq!(evaluate("x ? 2 : 3 ? 4 : 5", &[0., 0.]), 4.);
    }

    #[test]
    fn rejects_unknown_or_non_finite_expressions() {
        assert!(Expression::parse("sine($t)", &["$t"]).is_err());
        assert!(
            Expression::parse("1 / 0", &["$t"])
                .unwrap()
                .evaluate(&[0.])
                .is_err()
        );
    }
}
