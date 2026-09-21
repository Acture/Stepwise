use rustpython_parser::{Mode, Parse, Tok, ast, ast::Ranged, lexer};
use std::{
	collections::{BTreeMap, BTreeSet},
	ops::Range,
};

use super::{BinaryOp, BoolOp, CompareOp, Op, UnaryOp, value};
use crate::core::{Expr, ExprKind, Language, NodeId, ParseError, Value};

pub fn parse_expression(source: &str) -> Result<Expr, ParseError> {
	parse_bound_expression(source, &BTreeMap::new())
}

pub fn parse_bound_expression(
	source: &str,
	bindings: &BTreeMap<String, Value>,
) -> Result<Expr, ParseError> {
	let source: &str = source.trim();
	if source.len() > 2048 {
		return Err(ParseError("表达式最多 2048 字节。".into()));
	}
	let mut nesting: usize = 0;
	for character in source.chars() {
		match character {
			'(' | '[' | '{' => nesting += 1,
			')' | ']' | '}' => nesting = nesting.saturating_sub(1),
			_ => {}
		}
		if nesting > 32 {
			return Err(ParseError("括号最多嵌套 32 层。".into()));
		}
	}
	let parsed: ast::Expr = ast::Expr::parse(source.trim(), "<expression>")
		.map_err(|error| ParseError(format!("Python 语法错误：{error}")))?;
	let mut next_id: NodeId = 0;
	let mut root: Expr = convert(parsed, bindings, &mut next_id, 0)?;
	let names: BTreeSet<&str> = root
		.rows()
		.into_iter()
		.filter_map(|(_, node)| match &node.kind {
			ExprKind::Binding { name, .. } => Some(name.as_str()),
			_ => None,
		})
		.collect();
	if let Some(name) = bindings.keys().find(|name| !names.contains(name.as_str())) {
		return Err(ParseError(format!("表达式中没有变量 {name}。")));
	}
	add_groups(source, &mut root, &mut next_id)?;
	if root.rows().iter().any(|(depth, _)| *depth > 32) {
		return Err(ParseError("包含括号的表达式最多嵌套 32 层。".into()));
	}
	Ok(root)
}

fn operation(op: Op, operands: Vec<Expr>) -> ExprKind {
	ExprKind::Operation(op.into(), operands)
}

fn convert(
	parsed: ast::Expr,
	bindings: &BTreeMap<String, Value>,
	next_id: &mut NodeId,
	depth: usize,
) -> Result<Expr, ParseError> {
	if depth > 32 || *next_id >= 128 {
		return Err(ParseError("首版最多支持 128 个节点、32 层表达式。".into()));
	}
	let id: NodeId = *next_id;
	*next_id += 1;
	let source_range: std::ops::Range<usize> =
		parsed.range().start().to_usize()..parsed.range().end().to_usize();
	let kind: ExprKind = match parsed {
		ast::Expr::Name(node) => {
			let name: String = node.id.to_string();
			let value: Value = value::checked(
				bindings
					.get(&name)
					.ok_or_else(|| {
						ParseError(format!("未给变量 {name} 赋值；使用 --assign {name}=3。"))
					})?
					.clone(),
			)
			.map_err(|error| ParseError(error.to_string()))?;
			ExprKind::Binding {
				language: Language::Python,
				name,
				value,
			}
		}
		ast::Expr::Constant(node) => {
			let value: Value = match node.value {
				ast::Constant::Bool(value) => Value::Bool(value),
				ast::Constant::Int(value) => Value::Int(value),
				ast::Constant::Float(value) => Value::Float(value),
				ast::Constant::None => Value::None,
				_ => return Err(ParseError(
					"首版仅支持整数、有限浮点数、True、False 和 None；暂不支持字符串、复数或容器。"
						.into(),
				)),
			};
			ExprKind::Value(value::checked(value).map_err(|error| ParseError(error.to_string()))?)
		}
		ast::Expr::UnaryOp(node) => {
			let op: UnaryOp = match node.op {
				ast::UnaryOp::UAdd => UnaryOp::Positive,
				ast::UnaryOp::USub => UnaryOp::Negative,
				ast::UnaryOp::Not => UnaryOp::Not,
				_ => return Err(ParseError("暂不支持按位运算。".into())),
			};
			operation(
				Op::Unary(op),
				vec![convert(*node.operand, bindings, next_id, depth + 1)?],
			)
		}
		ast::Expr::BinOp(node) => {
			let op: BinaryOp = match node.op {
				ast::Operator::Add => BinaryOp::Add,
				ast::Operator::Sub => BinaryOp::Subtract,
				ast::Operator::Mult => BinaryOp::Multiply,
				ast::Operator::Div => BinaryOp::Divide,
				ast::Operator::FloorDiv => BinaryOp::FloorDivide,
				ast::Operator::Mod => BinaryOp::Modulo,
				ast::Operator::Pow => BinaryOp::Power,
				_ => {
					return Err(ParseError(
						"首版支持 +、-、*、/、//、%、**，暂不支持其他运算符。".into(),
					));
				}
			};
			operation(
				Op::Binary(op),
				vec![
					convert(*node.left, bindings, next_id, depth + 1)?,
					convert(*node.right, bindings, next_id, depth + 1)?,
				],
			)
		}
		ast::Expr::BoolOp(node) => {
			let op: BoolOp = match node.op {
				ast::BoolOp::And => BoolOp::And,
				ast::BoolOp::Or => BoolOp::Or,
			};
			let values: Vec<Expr> = node
				.values
				.into_iter()
				.map(|value| convert(value, bindings, next_id, depth + 1))
				.collect::<Result<_, _>>()?;
			operation(Op::Bool(op), values)
		}
		ast::Expr::Compare(node) => {
			if node.ops.len() != 1 {
				return Err(ParseError(
					"首版暂不支持链式比较（如 1 < 2 < 3）；请使用单个比较。".into(),
				));
			}
			let op: CompareOp = match node.ops[0] {
				ast::CmpOp::Eq => CompareOp::Equal,
				ast::CmpOp::NotEq => CompareOp::NotEqual,
				ast::CmpOp::Lt => CompareOp::Less,
				ast::CmpOp::LtE => CompareOp::LessEqual,
				ast::CmpOp::Gt => CompareOp::Greater,
				ast::CmpOp::GtE => CompareOp::GreaterEqual,
				_ => return Err(ParseError("暂不支持 is、in 或其否定形式。".into())),
			};
			let right: ast::Expr = node
				.comparators
				.into_iter()
				.next()
				.expect("parser supplies a comparator");
			operation(
				Op::Compare(op),
				vec![
					convert(*node.left, bindings, next_id, depth + 1)?,
					convert(right, bindings, next_id, depth + 1)?,
				],
			)
		}
		_ => {
			return Err(ParseError(
				"首版仅支持纯表达式；函数调用、赋值、条件表达式和容器暂不支持。".into(),
			));
		}
	};
	Ok(Expr {
		id,
		source_range,
		kind,
	})
}

/// Python's semantic AST drops grouping. Recover explicit pairs from lexer spans, inside out.
fn add_groups(source: &str, root: &mut Expr, next_id: &mut NodeId) -> Result<(), ParseError> {
	fn wrap(root: &mut Expr, inner: &Range<usize>, outer: &Range<usize>, id: NodeId) -> bool {
		if &root.source_range == inner {
			let child: Expr = root.clone();
			*root = Expr {
				id,
				source_range: outer.clone(),
				kind: ExprKind::Group(Box::new(child)),
			};
			return true;
		}
		root.children_mut()
			.into_iter()
			.any(|child| wrap(child, inner, outer, id))
	}
	let tokens: Vec<(Tok, Range<usize>)> = lexer::lex(source, Mode::Expression)
		.map(|token| {
			token.map(|(kind, range)| (kind, range.start().to_usize()..range.end().to_usize()))
		})
		.collect::<Result<Vec<_>, _>>()
		.map_err(|error| ParseError(format!("Python 词法错误：{error:?}")))?
		.into_iter()
		.filter(|(token, _)| !matches!(token, Tok::Newline | Tok::EndOfFile))
		.collect();
	let mut stack: Vec<usize> = Vec::new();
	for (index, (token, range)) in tokens.iter().enumerate() {
		match token {
			Tok::Lpar => stack.push(index),
			Tok::Rpar => {
				let start: usize = stack.pop().expect("parser validated parentheses");
				let inner: Range<usize> = tokens[start + 1].1.start..tokens[index - 1].1.end;
				let outer: Range<usize> = tokens[start].1.start..range.end;
				if *next_id >= 128 {
					return Err(ParseError("包含括号的表达式最多 128 个节点。".into()));
				}
				if !wrap(root, &inner, &outer, *next_id) {
					return Err(ParseError("无法匹配分组括号。".into()));
				}
				*next_id += 1;
			}
			_ => {}
		}
	}
	Ok(())
}

/// Answers are literals, optionally signed. Never evaluate a student's expression as an answer.
pub fn parse_value(input: &str) -> Result<Value, ParseError> {
	let expression: Expr = parse_expression(input)?;
	let signed: Option<(UnaryOp, ExprKind)> = match expression.kind {
		ExprKind::Value(value) => return Ok(value),
		ExprKind::Operation(crate::core::Op::Python(Op::Unary(op)), operands)
			if matches!(op, UnaryOp::Positive | UnaryOp::Negative) =>
		{
			operands
				.into_iter()
				.next()
				.map(|operand| (op, operand.kind))
		}
		_ => None,
	};
	match signed {
		Some((op, ExprKind::Value(value @ (Value::Int(_) | Value::Float(_))))) => {
			value::unary(&value, op).map_err(|error| ParseError(error.to_string()))
		}
		Some(_) => Err(ParseError("请输入一个值，不要填写待计算的表达式。".into())),
		None => Err(ParseError(
			"请输入一个值（例如 -3、2.0、False），不要填写待计算的表达式。".into(),
		)),
	}
}
