use serde::Deserialize;

use super::{EvaluationMode, Expr, Outcome, ParseError, Value};

/// The teaching languages. Core owns the shared flow; each language owns its own
/// parsing, operator rules and explanations, reached only through this interface.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Language {
	Python,
	Logic,
}

impl Language {
	pub fn label(self) -> &'static str {
		match self {
			Self::Python => "Python 运算练习",
			Self::Logic => "命题逻辑",
		}
	}

	/// Part of the progress key and of a random question's ID; changing it separates
	/// already saved progress and must stay paired with [`Language::from_key`].
	pub fn key(self) -> &'static str {
		match self {
			Self::Python => "python",
			Self::Logic => "logic",
		}
	}

	pub fn from_key(key: &str) -> Option<Self> {
		[Self::Python, Self::Logic]
			.into_iter()
			.find(|language| language.key() == key)
	}

	/// A typed answer is a literal of this language, never an expression to evaluate.
	pub fn parse_answer(self, input: &str) -> Result<Value, ParseError> {
		match self {
			Self::Python => crate::python::parse_value(input),
			Self::Logic => crate::logic::parse_truth(input).map(Value::Bool),
		}
	}

	/// What a binding literal of this language must look like, for a question file that got
	/// one wrong. The language owns the words, as it owns the literals.
	pub fn literal_form(self) -> &'static str {
		match self {
			Self::Python => crate::python::LITERAL_FORM,
			Self::Logic => crate::logic::LITERAL_FORM,
		}
	}

	pub(crate) fn binding_explanation(self, name: &str, value: &Value) -> String {
		match self {
			Self::Python => crate::python::binding_explanation(name, value),
			Self::Logic => crate::logic::binding_explanation(name, value),
		}
	}
}

/// Where a rendered operation puts its symbol. Core owns spacing and bracketing, the
/// language owns the symbol and its position; neither consults precedence to render.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
	Prefix(&'static str),
	Infix(&'static str),
}

/// What a language decides about one operation node whose operands are already in the tree.
pub enum Step {
	/// Reduce this operand first. A language only names an operand that is not yet a value.
	Operand(usize),
	/// The operation applies here. `skipped_operands` holds the POSITIONS in `operands`
	/// whose subtrees must not be evaluated; core turns them into node identifiers.
	Apply {
		outcome: Outcome,
		explanation: String,
		skipped_operands: Vec<usize>,
	},
}

/// The rules a language supplies for its own operations. Core never reads inside an operator.
pub(crate) trait Rules {
	fn layout(&self) -> Layout;

	/// Ranks competing operations inside one parenthesis scope; only compared within a language.
	fn precedence(&self) -> u8;

	/// Called with exactly the operands this operator's parser built for it, so an
	/// implementation may destructure its own arity.
	fn step(&self, operands: &[Expr], mode: EvaluationMode) -> Step;

	/// True when a later operand may still be skipped, so only the deciding operand is
	/// selectable. Every operator answers this: a wrong `false` would let a student
	/// evaluate a branch the rules say to skip.
	fn skips_operands(&self, mode: EvaluationMode) -> bool;

	/// Set when a negative value shown at this operand needs brackets to keep its meaning,
	/// carrying the note the student sees about the brackets that stay visible.
	fn negative_brackets(&self, _index: usize) -> Option<&'static str> {
		None
	}

	/// A whole expression that already spells its answer, finished without another student
	/// step. Applied by [`super::Session`] at the root only, never during descent, so the
	/// number of semantic reductions a question needs stays unchanged.
	fn completed_value(&self, _operands: &[Expr]) -> Option<Value> {
		None
	}
}

/// The one place core names a language: a closed pair, not a plugin registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
	Python(crate::python::Op),
	Logic(crate::logic::Op),
}

impl Op {
	pub(crate) fn rules(&self) -> &dyn Rules {
		match self {
			Self::Python(op) => op,
			Self::Logic(op) => op,
		}
	}
}
