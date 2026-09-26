use serde::{Deserialize, Serialize};

use super::{EvalError, Expr, ExprKind, NodeId, Step, Value};

/// How a question is evaluated. Both languages open with short circuit on; eager evaluation
/// is the teaching variant a student switches to, or a question asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvaluationMode {
	#[default]
	ShortCircuit,
	Eager,
}

impl EvaluationMode {
	pub fn label(self) -> &'static str {
		match self {
			Self::ShortCircuit => "短路开启",
			Self::Eager => "短路关闭 · 全部求值",
		}
	}

	pub fn key(self) -> &'static str {
		match self {
			Self::ShortCircuit => "short-circuit",
			Self::Eager => "eager",
		}
	}

	pub fn toggled(self) -> Self {
		match self {
			Self::ShortCircuit => Self::Eager,
			Self::Eager => Self::ShortCircuit,
		}
	}
}

pub type Outcome = Result<Value, EvalError>;

#[derive(Clone, Debug, PartialEq)]
pub struct NextStep {
	pub node_id: NodeId,
	pub outcome: Outcome,
	pub explanation: String,
	/// Descendants in these subtrees must never be evaluated.
	pub skipped: Vec<NodeId>,
}

/// One rule application, chosen by descending until a node can reduce on its own.
/// Bindings, grouping and the descent itself are shared; every operator rule comes
/// from the language that owns the operator.
pub fn next_step(root: &Expr, mode: EvaluationMode) -> Option<NextStep> {
	let (outcome, explanation, skipped): (Outcome, String, Vec<NodeId>) = match &root.kind {
		ExprKind::Value(_) => return None,
		ExprKind::Binding {
			language,
			name,
			value,
		} => (
			Ok(value.clone()),
			language.binding_explanation(name, value),
			Vec::new(),
		),
		ExprKind::Group(child) => {
			if let Some(step) = next_step(child, mode) {
				return Some(step);
			}
			(
				Ok(child.value().expect("group contents reduced").clone()),
				"括号内已经是一个值；去掉这一层分组，值和类型保持不变。".into(),
				Vec::new(),
			)
		}
		ExprKind::Operation(op, operands) => match op.rules().step(operands, mode) {
			Step::Operand(index) => {
				return Some(
					next_step(&operands[index], mode)
						.expect("a language names only an unreduced operand"),
				);
			}
			Step::Apply {
				outcome,
				explanation,
				skipped_operands,
			} => (
				outcome,
				explanation,
				skipped_operands
					.into_iter()
					.map(|index| operands[index].id)
					.collect(),
			),
		},
	};
	let explanation: String = match &outcome {
		Ok(_) => explanation,
		Err(error) => error.to_string(),
	};
	Some(NextStep {
		node_id: root.id,
		outcome,
		explanation,
		skipped,
	})
}
