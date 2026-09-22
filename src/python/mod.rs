//! Python: expression parsing, operator rules, type semantics and the explanations a
//! student reads. Everything here is Python's own; the shared teaching flow lives in
//! [`crate::core`] and reaches these rules through [`crate::core::Op`].

pub(crate) mod generate;
mod op;
mod parse;
mod value;

pub use op::{BinaryOp, BoolOp, CompareOp, Op, UnaryOp};
pub use parse::{parse_expression, parse_value};

/// What a Python binding literal looks like, for a question file that wrote something else.
/// Python owns these words, as it owns the literals.
pub const LITERAL_FORM: &str = "Python 源码字面量，例如 2、-0.0、True、None";

use std::collections::BTreeMap;

use crate::core::{EvaluationMode, Expr, Language, ParseError, Session, Value};

/// Start a Python practice session. The progress key embeds this module's own name and
/// the Debug rendering of Python's typed bindings; both must stay byte-identical.
pub fn session(
	source: &str,
	bindings: &BTreeMap<String, Value>,
	mode: EvaluationMode,
) -> Result<Session, ParseError> {
	let source: String = source.trim().into();
	let root: Expr = parse::parse_bound_expression(&source, bindings)?;
	let context: String = format!("python\n{bindings:?}\n{source}");
	Ok(Session::new(Language::Python, source, root, context, mode))
}

pub(crate) fn binding_explanation(name: &str, value: &Value) -> String {
	format!(
		"根据本题赋值，将所有同名变量 {name} 代入为 {value}（{}）。",
		value.type_name()
	)
}
