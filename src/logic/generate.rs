use rand::Rng;

use crate::generate::{Builder, Fragment};

/// A propositional question combines the five connectives over bound letters.
pub(crate) fn sample(builder: &mut Builder, count: u32) -> Fragment {
	formula(builder, count)
}

fn variable(builder: &mut Builder) -> Fragment {
	builder.variable(&["P", "Q", "R", "S"], |rng| {
		if rng.gen_bool(0.5) { "True" } else { "False" }.into()
	})
}

fn formula(builder: &mut Builder, count: u32) -> Fragment {
	if count == 0 {
		return variable(builder);
	}
	let expression: Fragment = if builder.rng.gen_bool(0.2) {
		formula(builder, count - 1).unary("¬", 90)
	} else {
		let left_count: u32 = builder.rng.gen_range(0..count);
		let left: Fragment = formula(builder, left_count);
		let right: Fragment = formula(builder, count - 1 - left_count);
		let op: &str = builder.choose(&["∧", "∨", "→", "↔"]);
		let precedence: u8 = match op {
			"∧" => 70,
			"∨" => 50,
			"→" => 30,
			_ => 10,
		};
		left.binary(op, precedence, right, op == "→")
	};
	if builder.rng.gen_bool(0.15) {
		expression.grouped()
	} else {
		expression
	}
}
