use super::{BinaryOp, EvalError, Expr, ExprKind, NodeId, UnaryOp, Value};
use serde::{Deserialize, Serialize};

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

pub fn next_step(root: &Expr, mode: EvaluationMode) -> Option<NextStep> {
	let (outcome, explanation): (Outcome, String) = match &root.kind {
		ExprKind::Value(_) => return None,
		ExprKind::Variable(name, value) => (
			Ok(value.clone()),
			format!(
				"根据本题赋值，将所有同名变量 {name} 代入为 {value}（{}）。",
				value.type_name()
			),
		),
		ExprKind::Group(child) => {
			if let Some(step) = next_step(child, mode) {
				return Some(step);
			}
			(
				Ok(child.value().expect("group contents reduced").clone()),
				"括号内已经是一个值；去掉这一层分组，值和类型保持不变。".into(),
			)
		}
		ExprKind::Proposition(name, value) => (
			Ok(Value::Bool(*value)),
			format!(
				"根据本题赋值，将所有同名命题 {name} 代入为 {}。",
				if *value { "True" } else { "False" }
			),
		),
		ExprKind::Logic(op, left, right) => {
			if let Some(step) = next_step(left, mode) {
				return Some(step);
			}
			let left_value: bool = left.value().expect("logic left reduced").truthy();
			if mode == EvaluationMode::ShortCircuit
				&& let Some(value) = op.short_circuit(left_value)
			{
				return Some(NextStep {
					node_id: root.id,
					outcome: Ok(Value::Bool(value)),
					explanation: format!(
						"短路求值：左侧真值已足以确定结果，右侧无需计算。{}",
						op.rule()
					),
					skipped: vec![right.id],
				});
			}
			if let Some(step) = next_step(right, mode) {
				return Some(step);
			}
			(
				Ok(Value::Bool(op.apply(
					left_value,
					right.value().expect("logic right reduced").truthy(),
				))),
				op.rule().into(),
			)
		}
		ExprKind::Unary(op, operand) => {
			if let Some(step) = next_step(operand, mode) {
				return Some(step);
			}
			let value: &Value = operand.value().expect("operand is reduced");
			let reason: &str = match op {
				UnaryOp::Positive => "一元 + 保留数值；布尔值参加数值运算时会转成整数。",
				UnaryOp::Negative => "一元 - 改变数值的符号。",
				UnaryOp::Not => "not 根据操作数的真假值取反，结果一定是 bool。",
				UnaryOp::LogicalNot => "否定：原命题真则结果假，原命题假则结果真。",
			};
			(value.unary(*op), reason.into())
		}
		ExprKind::Binary(op, left, right) => {
			if let Some(step) = next_step(left, mode).or_else(|| next_step(right, mode)) {
				return Some(step);
			}
			let left: &Value = left.value().expect("left is reduced");
			let right: &Value = right.value().expect("right is reduced");
			let reason: &str = match op {
				BinaryOp::Add => "两个操作数都已求值，现在应用加法规则。",
				BinaryOp::Subtract => "两个操作数都已求值，现在应用减法规则。",
				BinaryOp::Multiply => "两个操作数都已求值，现在应用乘法规则。",
				BinaryOp::Divide => "Python 的 / 是真除法，整数相除也返回 float。",
				BinaryOp::FloorDivide => {
					"// 向负无穷取整，而不是朝零截断；有浮点操作数时返回 float。"
				}
				BinaryOp::Modulo => "% 的非零余数与除数同号，并与 // 的向下取整规则一致。",
				BinaryOp::Power => "** 应用乘方规则；负整数指数返回 float。",
			};
			(left.binary(*op, right), reason.into())
		}
		ExprKind::Compare(op, left, right) => {
			if let Some(step) = next_step(left, mode).or_else(|| next_step(right, mode)) {
				return Some(step);
			}
			(
				left.value()
					.expect("left is reduced")
					.compare(*op, right.value().expect("right is reduced")),
				"比较两个已经求值的操作数，返回 True 或 False。".into(),
			)
		}
		ExprKind::Bool(op, operands) => {
			if mode == EvaluationMode::Eager {
				if let Some(step) = operands.iter().find_map(|operand| next_step(operand, mode)) {
					return Some(step);
				}
				let value: &Value = operands
					.iter()
					.filter_map(Expr::value)
					.find(|value| op.stops(value))
					.unwrap_or_else(|| {
						operands
							.last()
							.and_then(Expr::value)
							.expect("nonempty operands")
					});
				return Some(NextStep {
					node_id: root.id,
					outcome: Ok(value.clone()),
					explanation: format!(
						"短路已关闭：所有操作数均已求值，再按 {} 的操作数返回规则得到 {}（{}）。这是教学变体，不是 Python 的执行策略。",
						op.symbol(),
						value,
						value.type_name()
					),
					skipped: Vec::new(),
				});
			}
			for (index, operand) in operands.iter().enumerate() {
				if let Some(step) = next_step(operand, mode) {
					return Some(step);
				}
				let value: &Value = operand.value().expect("operand is reduced");
				if op.stops(value) || index + 1 == operands.len() {
					let skipped: Vec<NodeId> =
						operands[index + 1..].iter().map(|node| node.id).collect();
					let explanation: String = if skipped.is_empty() {
						format!(
							"{} 返回最后求值的操作数 {}（{}），不会强制转成 bool。",
							op.symbol(),
							value,
							value.type_name()
						)
					} else {
						format!(
							"短路求值：{} 遇到{}值 {}，直接返回该操作数（{}），跳过后面的子树。",
							op.symbol(),
							if value.truthy() { "真" } else { "假" },
							value,
							value.type_name()
						)
					};
					return Some(NextStep {
						node_id: root.id,
						outcome: Ok(value.clone()),
						explanation,
						skipped,
					});
				}
			}
			unreachable!("parser produces nonempty boolean expressions")
		}
	};
	let explanation: String = match &outcome {
		Ok(_) => explanation,
		Err(error) => error.to_string(),
	};
	Some(NextStep {
		node_id: root.id,
		outcome,
		explanation,
		skipped: Vec::new(),
	})
}
