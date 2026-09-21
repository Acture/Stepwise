use std::{collections::BTreeMap, ops::Range};

use super::{Language, Layout, Op, Value};

pub type NodeId = usize;

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
	Value(Value),
	/// A name the question already bound: a Python variable or a proposition letter.
	Binding {
		language: Language,
		name: String,
		value: Value,
	},
	Group(Box<Expr>),
	Operation(Op, Vec<Expr>),
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
			ExprKind::Value(_) | ExprKind::Binding { .. } => Vec::new(),
			ExprKind::Group(child) => vec![child],
			ExprKind::Operation(_, operands) => operands.iter().collect(),
		}
	}

	pub(crate) fn children_mut(&mut self) -> Vec<&mut Self> {
		match &mut self.kind {
			ExprKind::Value(_) | ExprKind::Binding { .. } => Vec::new(),
			ExprKind::Group(child) => vec![child],
			ExprKind::Operation(_, operands) => operands.iter_mut().collect(),
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
			&& !matches!(self.kind, ExprKind::Binding { .. } | ExprKind::Group(_))
			&& self
				.value()
				.is_none_or(|value| value.to_string().starts_with('-'));
		if parentheses {
			text.push('(');
		}
		match &self.kind {
			ExprKind::Value(value) => text.push_str(&value.to_string()),
			ExprKind::Binding { name, .. } => text.push_str(name),
			ExprKind::Group(child) => {
				text.push('(');
				child.write(text, ranges, false);
				text.push(')');
			}
			ExprKind::Operation(op, operands) => match op.rules().layout() {
				Layout::Prefix(symbol) => {
					text.push_str(symbol);
					for operand in operands {
						operand.write(text, ranges, true);
					}
				}
				Layout::Infix(symbol) => {
					for (index, operand) in operands.iter().enumerate() {
						if index > 0 {
							text.push_str(&format!(" {symbol} "));
						}
						operand.write(text, ranges, true);
					}
				}
			},
		}
		if parentheses {
			text.push(')');
		}
		ranges.insert(self.id, start..text.len());
	}
}
