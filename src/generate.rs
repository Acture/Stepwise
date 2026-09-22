use std::collections::BTreeMap;

use num_traits::ToPrimitive;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::{
	core::{EvaluationMode, Expr, Language, ParseError, Value, next_step},
	exercises::Exercise,
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
			Language::Python => crate::python::generate::sample(&mut builder, count),
			Language::Logic => crate::logic::generate::sample(&mut builder, count),
		};
		let exercise: Exercise = Exercise {
			// A generated question belongs to no set; its seeded ID is its whole identity.
			set: String::new(),
			name: format!("{PREFIX}{}-{seed}", language.key()),
			title: "随机练习".into(),
			expression: formula.text,
			language,
			bindings: builder.bindings.clone(),
			evaluation: None,
			note: None,
		};
		if suitable(&exercise)? {
			return Ok(exercise);
		}
	}
	Err(ParseError(format!(
		"随机种子 {seed} 未能生成合适的题目，请换一个种子。"
	)))
}

/// The name is a versioned seed, so the current random question resumes without a second store.
pub fn restore(name: &str) -> Result<Option<Exercise>, ParseError> {
	if !name.starts_with("random-") {
		return Ok(None);
	}
	let suffix: &str = name.strip_prefix(PREFIX).ok_or_else(|| {
		ParseError("无法恢复这个版本的随机题。请用 --random 开始新题，原进度保留。".into())
	})?;
	let (language, seed): (&str, &str) = suffix
		.split_once('-')
		.ok_or_else(|| ParseError("随机题编号缺少种子。".into()))?;
	let language: Language =
		Language::from_key(language).ok_or_else(|| ParseError("随机题语言无效。".into()))?;
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

/// Source text plus the precedence it binds at, so a language only brackets what it must.
pub(crate) struct Fragment {
	pub(crate) text: String,
	precedence: u8,
}

impl Fragment {
	pub(crate) fn atom(text: String) -> Self {
		Self {
			text,
			precedence: 100,
		}
	}
	pub(crate) fn grouped(self) -> Self {
		Self::atom(format!("({})", self.text))
	}
	fn operand(self, minimum: u8) -> String {
		if self.precedence < minimum {
			self.grouped().text
		} else {
			self.text
		}
	}
	pub(crate) fn binary(
		self,
		symbol: &str,
		precedence: u8,
		right: Self,
		right_associative: bool,
	) -> Self {
		let left: String = self.operand(precedence + u8::from(right_associative));
		let right: String = right.operand(precedence + u8::from(!right_associative));
		Self {
			text: format!("{left} {symbol} {right}"),
			precedence,
		}
	}
	pub(crate) fn unary(self, symbol: &str, precedence: u8) -> Self {
		Self {
			text: format!("{symbol}{}", self.operand(precedence)),
			precedence,
		}
	}
}

/// The seeded draw sequence shared by both grammars; each language owns its own shapes.
pub(crate) struct Builder {
	pub(crate) rng: ChaCha8Rng,
	pub(crate) bindings: BTreeMap<String, String>,
}

impl Builder {
	pub(crate) fn choose<'a>(&mut self, choices: &'a [&'a str]) -> &'a str {
		choices[self.rng.gen_range(0..choices.len() as u32) as usize]
	}

	/// Draw a name, then draw its literal only the first time that name appears.
	pub(crate) fn variable<'a>(
		&mut self,
		names: &'a [&'a str],
		literal: impl FnOnce(&mut ChaCha8Rng) -> String,
	) -> Fragment {
		let name: &str = self.choose(names);
		if !self.bindings.contains_key(name) {
			let literal: String = literal(&mut self.rng);
			self.bindings.insert(name.into(), literal);
		}
		Fragment::atom(name.into())
	}
}
