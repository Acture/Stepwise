use std::{
	collections::{BTreeMap, BTreeSet},
	fmt,
	ops::Range,
};

use boolean_expression::{BDD, Expr as BooleanExpr};

use crate::core::{Expr, ExprKind, NodeId, ParseError, UnaryOp, Value};

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
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Formula {
	Atom(String),
	Constant(bool),
	Not(Box<Self>),
	Binary(LogicOp, Box<Self>, Box<Self>),
}

impl Formula {
	pub fn negate(self) -> Self {
		Self::Not(Box::new(self))
	}
	pub fn binary(op: LogicOp, left: Self, right: Self) -> Self {
		Self::Binary(op, Box::new(left), Box::new(right))
	}

	pub fn atoms(&self) -> BTreeSet<String> {
		match self {
			Self::Atom(name) => BTreeSet::from([name.clone()]),
			Self::Constant(_) => BTreeSet::new(),
			Self::Not(child) => child.atoms(),
			Self::Binary(_, left, right) => left.atoms().union(&right.atoms()).cloned().collect(),
		}
	}

	fn boolean(&self) -> BooleanExpr<String> {
		match self {
			Self::Atom(name) => BooleanExpr::Terminal(name.clone()),
			Self::Constant(value) => BooleanExpr::Const(*value),
			Self::Not(child) => BooleanExpr::not(child.boolean()),
			Self::Binary(op, left, right) => {
				let a: BooleanExpr<String> = left.boolean();
				let b: BooleanExpr<String> = right.boolean();
				match op {
					LogicOp::And => BooleanExpr::and(a, b),
					LogicOp::Or => BooleanExpr::or(a, b),
					LogicOp::Implies => BooleanExpr::or(BooleanExpr::not(a), b),
					LogicOp::Iff => BooleanExpr::or(
						BooleanExpr::and(a.clone(), b.clone()),
						BooleanExpr::and(BooleanExpr::not(a), BooleanExpr::not(b)),
					),
				}
			}
		}
	}

	/// An independent semantic check, never a substitute for checking a named proof rule.
	pub fn equivalent(&self, other: &Self) -> Result<bool, ParseError> {
		if self.atoms().union(&other.atoms()).count() > 12 {
			return Err(ParseError("等价检查最多支持 12 个不同命题。".into()));
		}
		let mut bdd: BDD<String> = BDD::new();
		let left: boolean_expression::BDDFunc = bdd.from_expr(&self.boolean());
		let right: boolean_expression::BDDFunc = bdd.from_expr(&other.boolean());
		Ok(left == right)
	}
}

impl fmt::Display for Formula {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Atom(name) => write!(f, "{name}"),
			Self::Constant(true) => write!(f, "⊤"),
			Self::Constant(false) => write!(f, "⊥"),
			Self::Not(child) => write!(f, "¬{child}"),
			Self::Binary(op, left, right) => write!(f, "({left} {} {right})", op.symbol()),
		}
	}
}

pub fn parse_truth(input: &str) -> Result<bool, ParseError> {
	match input.trim() {
		"True" | "true" | "T" | "⊤" | "真" => Ok(true),
		"False" | "false" | "F" | "⊥" | "假" => Ok(false),
		_ => Err(ParseError(
			"请输入真值 True / False（也接受 T / F、真 / 假）；0 和 1 不是本模式的答案。".into(),
		)),
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
	Atom(String),
	Constant(bool),
	Not,
	Op(LogicOp),
	Left,
	Right,
}

fn tokenize(source: &str) -> Result<Vec<(Token, Range<usize>)>, ParseError> {
	if source.len() > 2048 {
		return Err(ParseError("公式最多 2048 字节。".into()));
	}
	let mut rest: &str = source;
	let mut tokens: Vec<(Token, Range<usize>)> = Vec::new();
	while !rest.is_empty() {
		rest = rest.trim_start();
		if rest.is_empty() {
			break;
		}
		let start: usize = source.len() - rest.len();
		let operators: [(&str, Token); 26] = [
			("⇒", Token::Op(LogicOp::Implies)),
			("⊃", Token::Op(LogicOp::Implies)),
			("⇔", Token::Op(LogicOp::Iff)),
			("≡", Token::Op(LogicOp::Iff)),
			("<->", Token::Op(LogicOp::Iff)),
			("<=>", Token::Op(LogicOp::Iff)),
			("->", Token::Op(LogicOp::Implies)),
			("=>", Token::Op(LogicOp::Implies)),
			("&&", Token::Op(LogicOp::And)),
			("||", Token::Op(LogicOp::Or)),
			("↔", Token::Op(LogicOp::Iff)),
			("→", Token::Op(LogicOp::Implies)),
			("∧", Token::Op(LogicOp::And)),
			("∨", Token::Op(LogicOp::Or)),
			("&", Token::Op(LogicOp::And)),
			("|", Token::Op(LogicOp::Or)),
			("¬", Token::Not),
			("~", Token::Not),
			("!", Token::Not),
			("(", Token::Left),
			(")", Token::Right),
			("⊤", Token::Constant(true)),
			("⊥", Token::Constant(false)),
			("真", Token::Constant(true)),
			("假", Token::Constant(false)),
			("∼", Token::Not),
		];
		if let Some((symbol, token)) = operators
			.into_iter()
			.find(|(symbol, _)| rest.starts_with(symbol))
		{
			tokens.push((token, start..start + symbol.len()));
			rest = &rest[symbol.len()..];
		} else {
			let length: usize = rest
				.char_indices()
				.take_while(|(_, character)| character.is_ascii_alphanumeric() || *character == '_')
				.map(|(index, character)| index + character.len_utf8())
				.last()
				.unwrap_or(0);
			if length == 0
				|| !rest
					.chars()
					.next()
					.is_some_and(|character| character.is_ascii_alphabetic())
			{
				return Err(ParseError(format!(
					"无法识别的逻辑符号：{}",
					rest.chars().next().unwrap()
				)));
			}
			let name: &str = &rest[..length];
			tokens.push((
				match name {
					"True" | "true" | "T" => Token::Constant(true),
					"False" | "false" | "F" => Token::Constant(false),
					"not" => Token::Not,
					"and" => Token::Op(LogicOp::And),
					"or" => Token::Op(LogicOp::Or),
					_ => Token::Atom(name.into()),
				},
				start..start + length,
			));
			rest = &rest[length..];
		}
		if tokens.len() > 128 {
			return Err(ParseError("公式最多 128 个词法单元。".into()));
		}
	}
	Ok(tokens)
}

/// Concrete syntax keeps grouping for teaching; proof formulas erase only grouping.
struct Syntax {
	kind: SyntaxKind,
	range: Range<usize>,
}

enum SyntaxKind {
	Atom(String),
	Constant(bool),
	Not(Box<Syntax>),
	Group(Box<Syntax>),
	Binary(LogicOp, Box<Syntax>, Box<Syntax>),
}

impl Syntax {
	fn formula(&self) -> Formula {
		match &self.kind {
			SyntaxKind::Atom(name) => Formula::Atom(name.clone()),
			SyntaxKind::Constant(value) => Formula::Constant(*value),
			SyntaxKind::Not(child) => child.formula().negate(),
			SyntaxKind::Group(child) => child.formula(),
			SyntaxKind::Binary(op, left, right) => {
				Formula::binary(*op, left.formula(), right.formula())
			}
		}
	}

	fn teaching_tree(
		&self,
		bindings: &BTreeMap<String, bool>,
		next: &mut NodeId,
	) -> Result<Expr, ParseError> {
		let id: NodeId = *next;
		*next += 1;
		let kind: ExprKind = match &self.kind {
			SyntaxKind::Atom(name) => ExprKind::Proposition(
				name.clone(),
				*bindings.get(name).ok_or_else(|| {
					ParseError(format!(
						"未给命题 {name} 赋值；使用 --assign {name}=true 或 {name}=false。"
					))
				})?,
			),
			SyntaxKind::Constant(value) => ExprKind::Value(Value::Bool(*value)),
			SyntaxKind::Not(child) => ExprKind::Unary(
				UnaryOp::LogicalNot,
				Box::new(child.teaching_tree(bindings, next)?),
			),
			SyntaxKind::Group(child) => {
				ExprKind::Group(Box::new(child.teaching_tree(bindings, next)?))
			}
			SyntaxKind::Binary(op, left, right) => ExprKind::Logic(
				*op,
				Box::new(left.teaching_tree(bindings, next)?),
				Box::new(right.teaching_tree(bindings, next)?),
			),
		};
		Ok(Expr {
			id,
			source_range: self.range.clone(),
			kind,
		})
	}
}

pub fn parse_formula(source: &str) -> Result<Formula, ParseError> {
	Ok(parse_syntax(source)?.formula())
}

pub(crate) fn parse_teaching_formula(
	source: &str,
	bindings: &BTreeMap<String, bool>,
) -> Result<Expr, ParseError> {
	let syntax: Syntax = parse_syntax(source)?;
	let atoms: BTreeSet<String> = syntax.formula().atoms();
	if let Some(name) = bindings.keys().find(|name| !atoms.contains(*name)) {
		return Err(ParseError(format!("公式中没有命题 {name}。")));
	}
	syntax.teaching_tree(bindings, &mut 0)
}

fn parse_syntax(source: &str) -> Result<Syntax, ParseError> {
	fn expression(
		tokens: &[(Token, Range<usize>)],
		position: &mut usize,
		minimum: u8,
		depth: usize,
	) -> Result<Syntax, ParseError> {
		if depth > 32 {
			return Err(ParseError("公式最多嵌套 32 层。".into()));
		}
		let (token, span): &(Token, Range<usize>) = tokens
			.get(*position)
			.ok_or_else(|| ParseError("这里需要一个命题或子公式。".into()))?;
		*position += 1;
		let kind: SyntaxKind = match token {
			Token::Atom(name) => SyntaxKind::Atom(name.clone()),
			Token::Constant(value) => SyntaxKind::Constant(*value),
			Token::Not => SyntaxKind::Not(Box::new(expression(tokens, position, 9, depth + 1)?)),
			Token::Left => {
				let inside: Syntax = expression(tokens, position, 0, depth + 1)?;
				if !matches!(tokens.get(*position), Some((Token::Right, _))) {
					return Err(ParseError("缺少右括号。".into()));
				}
				*position += 1;
				SyntaxKind::Group(Box::new(inside))
			}
			_ => return Err(ParseError("这里需要一个命题或左括号。".into())),
		};
		let mut left: Syntax = Syntax {
			kind,
			range: span.start..tokens[*position - 1].1.end,
		};
		while let Some((Token::Op(op), _)) = tokens.get(*position) {
			let (left_binding, right_binding): (u8, u8) = match op {
				LogicOp::Iff => (1, 2),
				LogicOp::Implies => (3, 3),
				LogicOp::Or => (5, 6),
				LogicOp::And => (7, 8),
			};
			if left_binding < minimum {
				break;
			}
			*position += 1;
			let right: Syntax = expression(tokens, position, right_binding, depth + 1)?;
			let range: Range<usize> = left.range.start..right.range.end;
			left = Syntax {
				kind: SyntaxKind::Binary(*op, Box::new(left), Box::new(right)),
				range,
			};
		}
		Ok(left)
	}
	let tokens: Vec<(Token, Range<usize>)> = tokenize(source)?;
	let mut position: usize = 0;
	let formula: Syntax = expression(&tokens, &mut position, 0, 0)?;
	if position != tokens.len() {
		return Err(ParseError("公式后有多余内容或括号。".into()));
	}
	Ok(formula)
}
