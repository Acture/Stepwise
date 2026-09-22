//! Propositional logic: symbol aliases, formula parsing, BDD equivalence, the evaluation
//! rules a student applies, and classical natural deduction. The shared teaching flow lives
//! in [`crate::core`] and reaches these rules through [`crate::core::Op`].

mod formula;
pub(crate) mod generate;
mod op;
pub mod proof;

pub use formula::{Formula, parse_formula, parse_truth};

/// What a propositional binding literal looks like, for a question file that wrote something
/// else. Logic owns these words, as it owns the truth values.
pub const LITERAL_FORM: &str = "真值 True / False（也接受 true/false、T / F、真 / 假）";
pub use op::{LogicOp, Op};

use std::collections::BTreeMap;

use crate::core::{EvaluationMode, Expr, Language, ParseError, Session, Value};

/// Start a propositional practice session. The progress key embeds this module's own name
/// and the Debug rendering of logic's truth-value bindings; both must stay byte-identical.
pub fn session(
	source: &str,
	bindings: &BTreeMap<String, bool>,
	mode: EvaluationMode,
) -> Result<Session, ParseError> {
	let source: String = source.trim().into();
	let root: Expr = formula::parse_teaching_formula(&source, bindings)?;
	let context: String = format!("logic\n{bindings:?}\n{source}");
	Ok(Session::new(Language::Logic, source, root, context, mode))
}

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
