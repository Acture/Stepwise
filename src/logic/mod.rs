//! Propositional logic: symbol aliases, formula parsing, BDD equivalence, the evaluation
//! rules a student applies, and classical natural deduction. The shared teaching flow lives
//! in [`crate::core`] and reaches these rules through [`crate::core::Op`].

mod formula;
pub(crate) mod generate;
mod op;
pub mod proof;

pub(crate) use formula::parse_teaching_formula;
pub use formula::{Formula, parse_formula, parse_truth};
pub use op::{LogicOp, Op};

use crate::core::Value;

pub(crate) fn binding_explanation(name: &str, value: &Value) -> String {
	debug_assert!(
		matches!(value, Value::Bool(_)),
		"a proposition is a truth value"
	);
	format!("根据本题赋值，将所有同名命题 {name} 代入为 {value}。")
}

/// A propositional tree only ever holds truth values; anything else is a parser bug.
fn truth(value: &Value) -> bool {
	let Value::Bool(value) = value else {
		unreachable!("a proposition is bound to a truth value")
	};
	*value
}
