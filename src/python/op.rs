use super::value;
use crate::core::{Expr, Layout, Outcome, Rules, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
	Positive,
	Negative,
	Not,
}

impl UnaryOp {
	pub fn symbol(self) -> &'static str {
		match self {
			Self::Positive => "+",
			Self::Negative => "-",
			Self::Not => "not ",
		}
	}

	fn rule(self) -> &'static str {
		match self {
			Self::Positive => "一元 + 保留数值；布尔值参加数值运算时会转成整数。",
			Self::Negative => "一元 - 改变数值的符号。",
			Self::Not => "not 根据操作数的真假值取反，结果一定是 bool。",
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
	Add,
	Subtract,
	Multiply,
	Divide,
	FloorDivide,
	Modulo,
	Power,
}

impl BinaryOp {
	pub fn symbol(self) -> &'static str {
		match self {
			Self::Add => "+",
			Self::Subtract => "-",
			Self::Multiply => "*",
			Self::Divide => "/",
			Self::FloorDivide => "//",
			Self::Modulo => "%",
			Self::Power => "**",
		}
	}

	fn rule(self) -> &'static str {
		match self {
			Self::Add => "两个操作数都已求值，现在应用加法规则。",
			Self::Subtract => "两个操作数都已求值，现在应用减法规则。",
			Self::Multiply => "两个操作数都已求值，现在应用乘法规则。",
			Self::Divide => "Python 的 / 是真除法，整数相除也返回 float。",
			Self::FloorDivide => "// 向负无穷取整，而不是朝零截断；有浮点操作数时返回 float。",
			Self::Modulo => "% 的非零余数与除数同号，并与 // 的向下取整规则一致。",
			Self::Power => "** 应用乘方规则；负整数指数返回 float。",
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompareOp {
	Equal,
	NotEqual,
	Less,
	LessEqual,
	Greater,
	GreaterEqual,
}

impl CompareOp {
	pub fn symbol(self) -> &'static str {
		match self {
			Self::Equal => "==",
			Self::NotEqual => "!=",
			Self::Less => "<",
			Self::LessEqual => "<=",
			Self::Greater => ">",
			Self::GreaterEqual => ">=",
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolOp {
	And,
	Or,
}

impl BoolOp {
	pub fn symbol(self) -> &'static str {
		match self {
			Self::And => "and",
			Self::Or => "or",
		}
	}

	/// `and` stops on a false operand, `or` on a true one; the operand itself is the result.
	fn stops(self, value: &Value) -> bool {
		match self {
			Self::And => !value::truthy(value),
			Self::Or => value::truthy(value),
		}
	}

	/// The operand the scan stops at is the result: the first that decides, or the last.
	/// `None` while an operand the scan reaches is still unreduced.
	fn apply(self, operands: &[Expr]) -> Option<(Outcome, String)> {
		for (index, operand) in operands.iter().enumerate() {
			let value: &Value = operand.value()?;
			if self.stops(value) || index + 1 == operands.len() {
				let explanation: String = if index + 1 == operands.len() {
					format!(
						"{} 返回最后求值的操作数 {}（{}），不会强制转成 bool。",
						self.symbol(),
						value,
						value.type_name()
					)
				} else {
					// Later operands that already hold values may have been computed by the
					// student, so say plainly that Python itself would not compute them.
					let rest: &str = if operands[index + 1..]
						.iter()
						.all(|operand| operand.value().is_some())
					{
						"；后面的操作数不影响结果，Python 执行时不会计算它们。"
					} else {
						"，跳过后面的子树。"
					};
					format!(
						"短路求值：{} 遇到{}值 {}，直接返回该操作数（{}）{rest}",
						self.symbol(),
						if value::truthy(value) { "真" } else { "假" },
						value,
						value.type_name()
					)
				};
				return Some((Ok(value.clone()), explanation));
			}
		}
		unreachable!("parser produces nonempty boolean expressions")
	}
}

/// Every Python operation a student can select.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
	Unary(UnaryOp),
	Binary(BinaryOp),
	Compare(CompareOp),
	Bool(BoolOp),
}

impl From<Op> for crate::core::Op {
	fn from(op: Op) -> Self {
		Self::Python(op)
	}
}

/// Both values of a two-operand operation, or `None` while either is still unreduced.
fn pair(operands: &[Expr]) -> Option<(&Value, &Value)> {
	let [left, right] = operands else {
		unreachable!("a binary operation has two operands")
	};
	left.value().zip(right.value())
}

impl Rules for Op {
	fn layout(&self) -> Layout {
		match self {
			Self::Unary(op) => Layout::Prefix(op.symbol()),
			Self::Binary(op) => Layout::Infix(op.symbol()),
			Self::Compare(op) => Layout::Infix(op.symbol()),
			Self::Bool(op) => Layout::Infix(op.symbol()),
		}
	}

	fn precedence(&self) -> u8 {
		match self {
			Self::Binary(BinaryOp::Power) => 80,
			Self::Unary(UnaryOp::Positive | UnaryOp::Negative) => 70,
			Self::Binary(
				BinaryOp::Multiply | BinaryOp::Divide | BinaryOp::FloorDivide | BinaryOp::Modulo,
			) => 60,
			Self::Binary(BinaryOp::Add | BinaryOp::Subtract) => 50,
			Self::Compare(_) => 40,
			Self::Unary(UnaryOp::Not) => 30,
			Self::Bool(BoolOp::And) => 20,
			Self::Bool(BoolOp::Or) => 10,
		}
	}

	fn apply(&self, operands: &[Expr]) -> Option<(Outcome, String)> {
		match self {
			Self::Unary(op) => {
				let [operand] = operands else {
					unreachable!("a unary operation has one operand")
				};
				Some((value::unary(operand.value()?, *op), op.rule().into()))
			}
			Self::Binary(op) => {
				let (left, right) = pair(operands)?;
				Some((value::binary(left, *op, right), op.rule().into()))
			}
			Self::Compare(op) => {
				let (left, right) = pair(operands)?;
				Some((
					value::compare(left, *op, right),
					"比较两个已经求值的操作数，返回 True 或 False。".into(),
				))
			}
			Self::Bool(op) => op.apply(operands),
		}
	}

	fn short_circuits(&self) -> bool {
		matches!(self, Self::Bool(_))
	}

	fn negative_brackets(&self, index: usize) -> Option<&'static str> {
		(matches!(self, Self::Binary(BinaryOp::Power)) && index == 0)
			.then_some(" 负数作为幂的底数时，显示保留必要括号以免改变含义；该值已完成本步。")
	}

	/// A final minus sign over an unsigned number already spells a complete numeric answer.
	/// Do not collapse groups, inner negations, bool conversion, or double negatives.
	fn completed_value(&self, operands: &[Expr]) -> Option<Value> {
		if *self != Self::Unary(UnaryOp::Negative) {
			return None;
		}
		let [operand] = operands else {
			return None;
		};
		let Some(value @ (Value::Int(_) | Value::Float(_))) = operand.value() else {
			return None;
		};
		(!value.to_string().starts_with('-')).then(|| {
			value::unary(value, UnaryOp::Negative)
				.expect("negating a checked finite number stays in range")
		})
	}
}
