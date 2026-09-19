use std::{collections::BTreeMap, ops::Range};

use super::Value;

pub type NodeId = usize;

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
	Positive,
	Negative,
	Not,
	LogicalNot,
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

	pub fn stops(self, value: &Value) -> bool {
		match self {
			Self::And => !value.truthy(),
			Self::Or => value.truthy(),
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

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
	Value(Value),
	Variable(String, Value),
	Group(Box<Expr>),
	Proposition(String, bool),
	Logic(crate::logic::LogicOp, Box<Expr>, Box<Expr>),
	Unary(UnaryOp, Box<Expr>),
	Binary(BinaryOp, Box<Expr>, Box<Expr>),
	Compare(CompareOp, Box<Expr>, Box<Expr>),
	Bool(BoolOp, Vec<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
	pub id: NodeId,
	/// Trimmed source byte range; never changes on reduction.
	pub source_range: Range<usize>,
	pub kind: ExprKind,
}

impl Expr {
	pub fn value(&self) -> Option<&Value> {
		match &self.kind {
			ExprKind::Value(value) => Some(value),
			_ => None,
		}
	}

	pub fn children(&self) -> Vec<&Self> {
		match &self.kind {
			ExprKind::Value(_) | ExprKind::Variable(_, _) | ExprKind::Proposition(_, _) => {
				Vec::new()
			}
			ExprKind::Unary(_, operand) | ExprKind::Group(operand) => vec![operand],
			ExprKind::Binary(_, left, right)
			| ExprKind::Compare(_, left, right)
			| ExprKind::Logic(_, left, right) => {
				vec![left, right]
			}
			ExprKind::Bool(_, values) => values.iter().collect(),
		}
	}

	pub(super) fn children_mut(&mut self) -> Vec<&mut Self> {
		match &mut self.kind {
			ExprKind::Value(_) | ExprKind::Variable(_, _) | ExprKind::Proposition(_, _) => {
				Vec::new()
			}
			ExprKind::Unary(_, operand) | ExprKind::Group(operand) => vec![operand],
			ExprKind::Binary(_, left, right)
			| ExprKind::Compare(_, left, right)
			| ExprKind::Logic(_, left, right) => vec![left, right],
			ExprKind::Bool(_, values) => values.iter_mut().collect(),
		}
	}

	pub fn find(&self, id: NodeId) -> Option<&Self> {
		if self.id == id {
			Some(self)
		} else {
			self.children().into_iter().find_map(|child| child.find(id))
		}
	}

	pub(crate) fn replace(&mut self, id: NodeId, value: &Value) -> bool {
		if self.id == id {
			self.kind = ExprKind::Value(value.clone());
			return true;
		}
		self.children_mut()
			.into_iter()
			.any(|child| child.replace(id, value))
	}

	pub fn rows(&self) -> Vec<(usize, &Self)> {
		fn visit<'a>(node: &'a Expr, depth: usize, rows: &mut Vec<(usize, &'a Expr)>) {
			rows.push((depth, node));
			for child in node.children() {
				visit(child, depth + 1, rows);
			}
		}
		let mut rows: Vec<(usize, &Self)> = Vec::new();
		visit(self, 0, &mut rows);
		rows
	}

	pub fn render(&self) -> String {
		self.render_with_ranges().0
	}

	/// Parenthesize compound children so a reduced negative value never changes precedence.
	pub fn render_with_ranges(&self) -> (String, BTreeMap<NodeId, Range<usize>>) {
		let mut text: String = String::new();
		let mut ranges: BTreeMap<NodeId, Range<usize>> = BTreeMap::new();
		self.write(&mut text, &mut ranges, false);
		(text, ranges)
	}

	fn write(&self, text: &mut String, ranges: &mut BTreeMap<NodeId, Range<usize>>, nested: bool) {
		let start: usize = text.len();
		let parentheses: bool = nested
			&& !matches!(
				self.kind,
				ExprKind::Proposition(_, _) | ExprKind::Variable(_, _) | ExprKind::Group(_)
			) && self
			.value()
			.is_none_or(|value| value.to_string().starts_with('-'));
		if parentheses {
			text.push('(');
		}
		match &self.kind {
			ExprKind::Value(value) => text.push_str(&value.to_string()),
			ExprKind::Proposition(name, _) | ExprKind::Variable(name, _) => text.push_str(name),
			ExprKind::Group(child) => {
				text.push('(');
				child.write(text, ranges, false);
				text.push(')');
			}
			ExprKind::Logic(op, left, right) => {
				left.write(text, ranges, true);
				text.push_str(&format!(" {} ", op.symbol()));
				right.write(text, ranges, true);
			}
			ExprKind::Unary(op, operand) => {
				text.push_str(match op {
					UnaryOp::Positive => "+",
					UnaryOp::Negative => "-",
					UnaryOp::Not => "not ",
					UnaryOp::LogicalNot => "¬",
				});
				operand.write(text, ranges, true);
			}
			ExprKind::Binary(op, left, right) => {
				left.write(text, ranges, true);
				text.push_str(&format!(" {} ", op.symbol()));
				right.write(text, ranges, true);
			}
			ExprKind::Compare(op, left, right) => {
				left.write(text, ranges, true);
				text.push_str(&format!(" {} ", op.symbol()));
				right.write(text, ranges, true);
			}
			ExprKind::Bool(op, values) => {
				for (index, child) in values.iter().enumerate() {
					if index > 0 {
						text.push_str(&format!(" {} ", op.symbol()));
					}
					child.write(text, ranges, true);
				}
			}
		}
		if parentheses {
			text.push(')');
		}
		ranges.insert(self.id, start..text.len());
	}
}
