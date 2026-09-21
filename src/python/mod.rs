//! Python: expression parsing, operator rules, type semantics and the explanations a
//! student reads. Everything here is Python's own; the shared teaching flow lives in
//! [`crate::core`] and reaches these rules through [`crate::core::Op`].

pub(crate) mod generate;
mod op;
mod parse;
mod value;

pub use op::{BinaryOp, BoolOp, CompareOp, Op, UnaryOp};
pub use parse::{parse_bound_expression, parse_expression, parse_value};

use crate::core::Value;

pub(crate) fn binding_explanation(name: &str, value: &Value) -> String {
	format!(
		"根据本题赋值，将所有同名变量 {name} 代入为 {value}（{}）。",
		value.type_name()
	)
}
