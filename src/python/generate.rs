use rand::Rng;

use crate::generate::{Builder, Fragment};

/// A Python question is arithmetic, or a boolean pair of comparisons.
pub(crate) fn sample(builder: &mut Builder, count: u32) -> Fragment {
	if builder.rng.gen_bool(0.35) {
		boolean(builder, count)
	} else {
		number(builder, count)
	}
}

fn variable(builder: &mut Builder) -> Fragment {
	builder.variable(&["x", "y", "z"], |rng| rng.gen_range(-5..=9).to_string())
}

fn number(builder: &mut Builder, count: u32) -> Fragment {
	if count == 0 {
		return if builder.bindings.is_empty() || builder.rng.gen_bool(0.6) {
			variable(builder)
		} else {
			Fragment::atom(builder.rng.gen_range(1..=9).to_string())
		};
	}
	let expression: Fragment = if builder.rng.gen_bool(0.15) {
		number(builder, count - 1).unary("-", 70)
	} else {
		let op: &str = builder.choose(&["+", "-", "*", "//", "%", "/", "**"]);
		if matches!(op, "//" | "%" | "/" | "**") {
			// Positive literal divisors avoid accidental error exercises; powers stay small.
			let right: Fragment = Fragment::atom(if op == "**" {
				builder.rng.gen_range(2..=3).to_string()
			} else {
				builder.choose(&["2", "4"]).into()
			});
			number(builder, count - 1).binary(
				op,
				if op == "**" { 80 } else { 60 },
				right,
				op == "**",
			)
		} else {
			let left_count: u32 = builder.rng.gen_range(0..count);
			let left: Fragment = number(builder, left_count);
			let right: Fragment = number(builder, count - 1 - left_count);
			left.binary(op, if op == "*" { 60 } else { 50 }, right, false)
		}
	};
	if builder.rng.gen_bool(0.15) {
		expression.grouped()
	} else {
		expression
	}
}

fn comparison(builder: &mut Builder, count: u32) -> Fragment {
	let left_count: u32 = builder.rng.gen_range(0..=count);
	let left: Fragment = number(builder, left_count);
	let right: Fragment = number(builder, count - left_count);
	let op: &str = builder.choose(&["<", "<=", ">", ">=", "==", "!="]);
	left.binary(op, 40, right, false)
}

fn boolean(builder: &mut Builder, count: u32) -> Fragment {
	let left: Fragment = comparison(builder, count / 2);
	let mut right: Fragment = comparison(builder, count - count / 2);
	if builder.rng.gen_bool(0.5) {
		right = right.unary("not ", 30);
	}
	let op: &str = builder.choose(&["and", "or"]);
	left.binary(op, if op == "and" { 20 } else { 10 }, right, false)
}
