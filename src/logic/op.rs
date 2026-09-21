use super::truth;
use crate::core::{EvaluationMode, Expr, Layout, Rules, Step, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LogicOp {
	And,
	Or,
	Implies,
	Iff,
}

impl LogicOp {
	pub fn symbol(self) -> &'static str {
		match self {
			Self::And => "∧",
			Self::Or => "∨",
			Self::Implies => "→",
			Self::Iff => "↔",
		}
	}

	pub fn apply(self, left: bool, right: bool) -> bool {
		match self {
			Self::And => left && right,
			Self::Or => left || right,
			Self::Implies => !left || right,
			Self::Iff => left == right,
		}
	}

	pub fn short_circuit(self, left: bool) -> Option<bool> {
		match (self, left) {
			(Self::And, false) => Some(false),
			(Self::Or, true) | (Self::Implies, false) => Some(true),
			_ => None,
		}
	}

	pub fn rule(self) -> &'static str {
		match self {
			Self::And => "合取：两边都真才为真。",
			Self::Or => "析取（相容或）：至少一边为真即为真。",
			Self::Implies => "实质蕴涵：仅当前件真、后件假时为假。",
			Self::Iff => "等价：两边真值相同时为真。",
		}
	}

	fn precedence(self) -> u8 {
		match self {
			Self::And => 70,
			Self::Or => 50,
			Self::Implies => 30,
			Self::Iff => 10,
		}
	}
}

/// Every propositional operation a student can select.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
	Not,
	Binary(LogicOp),
}

impl From<Op> for crate::core::Op {
	fn from(op: Op) -> Self {
		Self::Logic(op)
	}
}

impl Rules for Op {
	fn layout(&self) -> Layout {
		match self {
			Self::Not => Layout::Prefix("¬"),
			Self::Binary(op) => Layout::Infix(op.symbol()),
		}
	}

	fn precedence(&self) -> u8 {
		match self {
			Self::Not => 90,
			Self::Binary(op) => op.precedence(),
		}
	}

	fn step(&self, operands: &[Expr], mode: EvaluationMode) -> Step {
		match self {
			Self::Not => {
				let [operand] = operands else {
					unreachable!("a negation has one operand")
				};
				let Some(value) = operand.value() else {
					return Step::Operand(0);
				};
				Step::Apply {
					outcome: Ok(Value::Bool(!truth(value))),
					explanation: "否定：原命题真则结果假，原命题假则结果真。".into(),
					skipped_operands: Vec::new(),
				}
			}
			Self::Binary(op) => {
				let [antecedent, consequent] = operands else {
					unreachable!("a connective has two operands")
				};
				let Some(left) = antecedent.value() else {
					return Step::Operand(0);
				};
				let left: bool = truth(left);
				if mode == EvaluationMode::ShortCircuit
					&& let Some(value) = op.short_circuit(left)
				{
					return Step::Apply {
						outcome: Ok(Value::Bool(value)),
						explanation: format!(
							"短路求值：左侧真值已足以确定结果，右侧无需计算。{}",
							op.rule()
						),
						skipped_operands: vec![1],
					};
				}
				let Some(right) = consequent.value() else {
					return Step::Operand(1);
				};
				Step::Apply {
					outcome: Ok(Value::Bool(op.apply(left, truth(right)))),
					explanation: op.rule().into(),
					skipped_operands: Vec::new(),
				}
			}
		}
	}

	fn skips_operands(&self, mode: EvaluationMode) -> bool {
		// Equivalence always needs both sides; the other connectives may skip the right one.
		matches!(self, Self::Binary(op) if *op != LogicOp::Iff)
			&& mode == EvaluationMode::ShortCircuit
	}
}
