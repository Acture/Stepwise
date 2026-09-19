use std::collections::BTreeMap;

use num_traits::ToPrimitive;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::{
	core::{EvaluationMode, Expr, ParseError, Value, next_step},
	exercises::{Exercise, Language},
};

/// Version the seed protocol when changing the grammar, sampling or acceptance rules.
const PREFIX: &str = "random-v1-";

pub fn fresh_seed() -> u64 {
	rand::random()
}

/// Generate source and bindings only. The ordinary rule engine supplies all teaching steps.
pub fn generate(language: Language, seed: u64) -> Result<Exercise, ParseError> {
	let mut builder: Builder = Builder {
		rng: ChaCha8Rng::seed_from_u64(seed),
		bindings: BTreeMap::new(),
	};
	for _ in 0..128 {
		builder.bindings.clear();
		let count: u32 = builder.rng.gen_range(5..=8);
		let formula: Fragment = match language {
			Language::Python if builder.rng.gen_bool(0.35) => builder.python_boolean(count),
			Language::Python => builder.number(count),
			Language::Logic => builder.logic(count),
		};
		let exercise: Exercise = Exercise {
			id: format!(
				"{PREFIX}{}-{seed}",
				match language {
					Language::Python => "python",
					Language::Logic => "logic",
				}
			),
			title: "随机练习".into(),
			expression: formula.text,
			goal: "每次应用一条规则；n 下一道随机题，p 返回本次练习的上一题。".into(),
			language,
			bindings: builder.bindings.clone(),
		};
		if suitable(&exercise)? {
			return Ok(exercise);
		}
	}
	Err(ParseError(format!(
		"随机种子 {seed} 未能生成合适的题目，请换一个种子。"
	)))
}

/// The ID is a versioned seed, so the current random question resumes without a second store.
pub fn restore(id: &str) -> Result<Option<Exercise>, ParseError> {
	if !id.starts_with("random-") {
		return Ok(None);
	}
	let suffix: &str = id.strip_prefix(PREFIX).ok_or_else(|| {
		ParseError("无法恢复这个版本的随机题。请用 --random 开始新题，原进度保留。".into())
	})?;
	let (language, seed): (&str, &str) = suffix
		.split_once('-')
		.ok_or_else(|| ParseError("随机题编号缺少种子。".into()))?;
	let language: Language = match language {
		"python" => Language::Python,
		"logic" => Language::Logic,
		_ => return Err(ParseError("随机题语言无效。".into())),
	};
	let seed: u64 = seed
		.parse()
		.map_err(|_| ParseError("随机题种子无效。".into()))?;
	generate(language, seed).map(Some)
}

fn suitable(exercise: &Exercise) -> Result<bool, ParseError> {
	if exercise.expression.len() > 180 || exercise.bindings.is_empty() {
		return Ok(false);
	}
	for mode in [EvaluationMode::ShortCircuit, EvaluationMode::Eager] {
		// Count semantic reductions so UI completion shortcuts do not change persisted seeds.
		let mut root: Expr = exercise.session(mode)?.root().clone();
		let mut steps: usize = 0;
		while let Some(step) = next_step(&root, mode) {
			let Ok(value) = step.outcome else {
				return Ok(false);
			};
			let input: String = value.to_string();
			let manageable: bool = match &value {
				Value::Int(number) => number
					.to_i64()
					.is_some_and(|number| (-999..=999).contains(&number)),
				Value::Float(number) => number.abs() <= 999.0,
				Value::Bool(_) => true,
				Value::None => false,
			};
			if input.len() > 10 || !manageable {
				return Ok(false);
			}
			root.replace(step.node_id, &value);
			steps += 1;
			if steps > 40 {
				return Ok(false);
			}
		}
		if steps < 6 {
			return Ok(false);
		}
	}
	Ok(true)
}

struct Fragment {
	text: String,
	precedence: u8,
}

impl Fragment {
	fn atom(text: String) -> Self {
		Self {
			text,
			precedence: 100,
		}
	}
	fn grouped(self) -> Self {
		Self::atom(format!("({})", self.text))
	}
	fn operand(self, minimum: u8) -> String {
		if self.precedence < minimum {
			self.grouped().text
		} else {
			self.text
		}
	}
	fn binary(self, symbol: &str, precedence: u8, right: Self, right_associative: bool) -> Self {
		let left: String = self.operand(precedence + u8::from(right_associative));
		let right: String = right.operand(precedence + u8::from(!right_associative));
		Self {
			text: format!("{left} {symbol} {right}"),
			precedence,
		}
	}
	fn unary(self, symbol: &str, precedence: u8) -> Self {
		Self {
			text: format!("{symbol}{}", self.operand(precedence)),
			precedence,
		}
	}
}

struct Builder {
	rng: ChaCha8Rng,
	bindings: BTreeMap<String, String>,
}

impl Builder {
	fn choose<'a>(&mut self, choices: &'a [&'a str]) -> &'a str {
		choices[self.rng.gen_range(0..choices.len() as u32) as usize]
	}
	fn variable(&mut self, logical: bool) -> Fragment {
		let name: &str = self.choose(if logical {
			&["P", "Q", "R", "S"]
		} else {
			&["x", "y", "z"]
		});
		if !self.bindings.contains_key(name) {
			let literal: String = if logical {
				if self.rng.gen_bool(0.5) {
					"True"
				} else {
					"False"
				}
				.into()
			} else {
				self.rng.gen_range(-5..=9).to_string()
			};
			self.bindings.insert(name.into(), literal);
		}
		Fragment::atom(name.into())
	}
	fn number(&mut self, count: u32) -> Fragment {
		if count == 0 {
			return if self.bindings.is_empty() || self.rng.gen_bool(0.6) {
				self.variable(false)
			} else {
				Fragment::atom(self.rng.gen_range(1..=9).to_string())
			};
		}
		let expression: Fragment = if self.rng.gen_bool(0.15) {
			self.number(count - 1).unary("-", 70)
		} else {
			let op: &str = self.choose(&["+", "-", "*", "//", "%", "/", "**"]);
			if matches!(op, "//" | "%" | "/" | "**") {
				// Positive literal divisors avoid accidental error exercises; powers stay small.
				let right: Fragment = Fragment::atom(if op == "**" {
					self.rng.gen_range(2..=3).to_string()
				} else {
					self.choose(&["2", "4"]).into()
				});
				self.number(count - 1).binary(
					op,
					if op == "**" { 80 } else { 60 },
					right,
					op == "**",
				)
			} else {
				let left_count: u32 = self.rng.gen_range(0..count);
				let left: Fragment = self.number(left_count);
				let right: Fragment = self.number(count - 1 - left_count);
				left.binary(op, if op == "*" { 60 } else { 50 }, right, false)
			}
		};
		if self.rng.gen_bool(0.15) {
			expression.grouped()
		} else {
			expression
		}
	}
	fn comparison(&mut self, count: u32) -> Fragment {
		let left_count: u32 = self.rng.gen_range(0..=count);
		let left: Fragment = self.number(left_count);
		let right: Fragment = self.number(count - left_count);
		let op: &str = self.choose(&["<", "<=", ">", ">=", "==", "!="]);
		left.binary(op, 40, right, false)
	}
	fn python_boolean(&mut self, count: u32) -> Fragment {
		let left: Fragment = self.comparison(count / 2);
		let mut right: Fragment = self.comparison(count - count / 2);
		if self.rng.gen_bool(0.5) {
			right = right.unary("not ", 30);
		}
		let op: &str = self.choose(&["and", "or"]);
		left.binary(op, if op == "and" { 20 } else { 10 }, right, false)
	}
	fn logic(&mut self, count: u32) -> Fragment {
		if count == 0 {
			return self.variable(true);
		}
		let expression: Fragment = if self.rng.gen_bool(0.2) {
			self.logic(count - 1).unary("¬", 90)
		} else {
			let left_count: u32 = self.rng.gen_range(0..count);
			let left: Fragment = self.logic(left_count);
			let right: Fragment = self.logic(count - 1 - left_count);
			let op: &str = self.choose(&["∧", "∨", "→", "↔"]);
			let precedence: u8 = match op {
				"∧" => 70,
				"∨" => 50,
				"→" => 30,
				_ => 10,
			};
			left.binary(op, precedence, right, op == "→")
		};
		if self.rng.gen_bool(0.15) {
			expression.grouped()
		} else {
			expression
		}
	}
}
