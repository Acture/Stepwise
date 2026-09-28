use std::collections::{BTreeMap, BTreeSet};

use num_traits::ToPrimitive;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use stepwise::{
	core::{Expr, ExprKind, Feedback, FeedbackKind, Language, NextStep, Session, Value},
	exercises::{self, Exercise},
	generate, logic,
	python::{self, parse_value},
};

fn bindings(entries: &[(&str, &str)]) -> BTreeMap<String, Value> {
	entries
		.iter()
		.map(|(name, value)| ((*name).into(), parse_value(value).unwrap()))
		.collect()
}

fn selected(session: &Session) -> String {
	let (text, ranges) = session.render_with_ranges();
	text[ranges[&session.next_step().unwrap().node_id].clone()].into()
}

fn answer(session: &mut Session, input: &str) {
	let id: usize = session.next_step().unwrap().node_id;
	let feedback: stepwise::core::Feedback = session.submit(id, input);
	assert!(
		feedback.accepted(),
		"{}: {}",
		session.render(),
		feedback.message
	);
}

/// The allowed step whose selection reads `unit` on screen, whichever route it lies on.
fn allowed(session: &Session, unit: &str) -> Option<usize> {
	let (text, ranges) = session.render_with_ranges();
	session
		.allowed_steps()
		.into_iter()
		.map(|step| step.node_id)
		.find(|id| text[ranges[id].clone()] == *unit)
}

/// Answer the allowed step that reads `unit`, not necessarily the one a hint names.
fn take(session: &mut Session, unit: &str, input: &str) -> Feedback {
	let id: usize = allowed(session, unit)
		.unwrap_or_else(|| panic!("{unit} is not allowed in {}", session.render()));
	let feedback: Feedback = session.submit(id, input);
	assert!(
		feedback.accepted(),
		"{}: {}",
		session.render(),
		feedback.message
	);
	feedback
}

/// An operation applied before every operand is a value: the step skips what is left.
fn short_circuits(session: &Session, step: &NextStep) -> bool {
	matches!(
		&session.root().find(step.node_id).unwrap().kind,
		ExprKind::Operation(_, operands) if operands.iter().any(|operand| operand.value().is_none())
	)
}

#[test]
fn a_recommended_path_respects_dependencies_without_enforcing_variable_order() {
	let values: BTreeMap<String, Value> = bindings(&[("x", "2"), ("y", "3"), ("z", "4")]);
	let mut session: Session = python::session("x + y * z", &values).unwrap();
	for (unit, input) in [
		("x", "2"),
		("y", "3"),
		("z", "4"),
		("3 * 4", "12"),
		("2 + 12", "14"),
	] {
		assert_eq!(selected(&session), unit);
		if unit != "2 + 12" {
			assert_eq!(
				session.submit(session.root().id, "14").kind,
				FeedbackKind::NeedsInner
			);
		}
		answer(&mut session, input);
		let restored: Session = python::session(session.source(), &values)
			.unwrap()
			.replay(session.attempts())
			.unwrap();
		assert_eq!(restored.render(), session.render());
		assert_eq!(restored.root(), session.root());
	}
	assert_eq!(session.render(), "14");
}

#[test]
fn exponent_associativity_does_not_reverse_variable_read_order() {
	let mut session: Session = python::session(
		"-a ** b ** c",
		&bindings(&[("a", "2"), ("b", "3"), ("c", "2")]),
	)
	.unwrap();
	for (unit, input) in [
		("a", "2"),
		("b", "3"),
		("c", "2"),
		("3 ** 2", "9"),
		("2 ** 9", "512"),
	] {
		assert_eq!(selected(&session), unit);
		answer(&mut session, input);
	}
	assert!(session.is_finished());
	assert_eq!(session.render(), "-512");
}

#[test]
fn negative_power_bases_and_word_operators_keep_their_display_meaning() {
	let mut session: Session = python::session("(x) ** 2", &bindings(&[("x", "-2")])).unwrap();
	for (input, display) in [("-2", "(-2) ** 2"), ("-2", "(-2) ** 2"), ("4", "4")] {
		answer(&mut session, input);
		assert_eq!(session.render(), display);
	}
	let mut session: Session = python::session("not(False)", &BTreeMap::new()).unwrap();
	answer(&mut session, "False");
	assert_eq!(session.render(), "not False");
	answer(&mut session, "True");
	assert!(session.is_finished());
}

#[test]
fn variable_types_and_bindings_are_checked_before_starting() {
	for (value, wrong) in [
		("False", "0"),
		("2.0", "2"),
		("-0.0", "0.0"),
		("None", "False"),
	] {
		let mut session: Session = python::session("x", &bindings(&[("x", value)])).unwrap();
		assert!(!session.submit(session.root().id, wrong).accepted());
		assert!(session.history().is_empty());
		answer(&mut session, value);
	}
	assert!(python::session("x + 1", &BTreeMap::new()).is_err());
	assert!(python::session("x + 1", &bindings(&[("x", "2"), ("typo", "3")])).is_err());
	assert!(
		python::session(
			"x",
			&BTreeMap::from([("x".into(), Value::Float(f64::INFINITY))])
		)
		.is_err()
	);
}

#[test]
fn one_session_may_short_circuit_or_compute_the_operand_it_would_skip() {
	let values: BTreeMap<String, Value> =
		bindings(&[("flag", "False"), ("x", "3"), ("zero", "0"), ("limit", "1")]);
	let mut session: Session = python::session("flag and ((x / zero > limit))", &values).unwrap();
	answer(&mut session, "False");
	// The hint short-circuits, and every read inside the skipped operand stays open beside it.
	assert_eq!(selected(&session), "False and ((x / zero > limit))");
	for unit in ["x", "zero", "limit"] {
		assert!(allowed(&session, unit).is_some(), "{unit}");
	}
	assert!(session.final_binary_step().is_none());
	take(&mut session, "x", "3");
	take(&mut session, "zero", "0");
	// Computing part of the operand leaves the short circuit available.
	assert_eq!(selected(&session), "False and ((3 / 0 > limit))");
	let raised: Feedback = take(&mut session, "3 / 0", "ZeroDivisionError");
	assert!(
		raised.message.ends_with(
			"这个异常来自你选择计算的子式 3 / 0；Python 执行原式时不会在这里引发它。求值在这里终止。"
		),
		"{}",
		raised.message
	);
	assert!(session.is_finished());
	assert!(session.terminal_error().is_some());
	assert_eq!(
		session.submit(session.root().id, "False").kind,
		FeedbackKind::Finished
	);
	assert!(session.undo());
	take(&mut session, "False and ((3 / 0 > limit))", "False");
	assert_eq!(session.render(), "False");
	assert!(session.terminal_error().is_none());
	assert_eq!(session.history().len(), 4);
}

#[test]
fn all_five_logic_precedences_are_generated_from_source() {
	let truth: BTreeMap<String, bool> = BTreeMap::from([
		("P".into(), true),
		("Q".into(), true),
		("R".into(), false),
		("S".into(), false),
		("U".into(), true),
	]);
	// Computing every operand, including the Q and S that a short circuit would skip.
	let mut session: Session = logic::session("¬P ∧ Q ∨ R → S ↔ U", &truth).unwrap();
	for (unit, input) in [
		("P", "True"),
		("¬True", "False"),
		("Q", "True"),
		("False ∧ True", "False"),
		("R", "False"),
		("False ∨ False", "False"),
		("S", "False"),
		("False → False", "True"),
		("U", "True"),
		("True ↔ True", "True"),
	] {
		take(&mut session, unit, input);
	}
	assert!(session.is_finished());
	// The same session constructor, following the hints, short-circuits where it can.
	let mut session: Session = logic::session("¬P ∧ Q ∨ R → S ↔ U", &truth).unwrap();
	for (unit, input) in [
		("P", "True"),
		("¬True", "False"),
		("False ∧ Q", "False"),
		("R", "False"),
		("False ∨ False", "False"),
		("False → S", "True"),
		("U", "True"),
		("True ↔ True", "True"),
	] {
		assert_eq!(selected(&session), unit);
		answer(&mut session, input);
	}
	assert!(session.is_finished());
}

#[test]
fn logic_groups_preserve_aliases_and_require_separate_steps() {
	let mut session: Session = logic::session(
		"((P)) => Q",
		&BTreeMap::from([("P".into(), false), ("Q".into(), true)]),
	)
	.unwrap();
	for (unit, input, display) in [
		("P", "False", "((False)) => Q"),
		("(False)", "False", "(False) => Q"),
		("(False)", "False", "False => Q"),
		("Q", "True", "False => True"),
		("False => True", "True", "True"),
	] {
		// Reading Q is allowed although the hint already short-circuits the alias.
		let hint: &str = if unit == "Q" { "False => Q" } else { unit };
		assert_eq!(selected(&session), hint);
		take(&mut session, unit, input);
		assert_eq!(session.render(), display);
	}
	assert!(session.is_finished());
}

#[test]
fn long_exercises_reach_one_answer_by_short_circuit_or_by_computing_everything() {
	let exercises: Vec<Exercise> = exercises::builtin().unwrap().exercises().cloned().collect();
	for (id, expected) in [
		("long-arithmetic", "-0.5"),
		("long-power", "-504.0"),
		("long-python-logic", "True"),
		("long-logic", "False"),
	] {
		let exercise: &Exercise = exercises
			.iter()
			.find(|exercise| exercise.name == id)
			.unwrap();
		let mut lengths: Vec<usize> = Vec::new();
		for skip in [true, false] {
			let mut session: Session = exercise.session().unwrap();
			while !session.is_finished() {
				let step: NextStep = if skip {
					session.next_step().unwrap()
				} else {
					session
						.allowed_steps()
						.into_iter()
						.find(|step| !short_circuits(&session, step))
						.expect("computing every operand is always allowed")
				};
				let feedback: Feedback =
					session.submit(step.node_id, &step.outcome.unwrap().to_string());
				assert!(feedback.accepted(), "{id}: {}", feedback.message);
				assert!(session.history().len() < 128);
			}
			assert_eq!(session.render(), expected, "{id} skip={skip}");
			assert!(session.history().len() > 10);
			lengths.push(session.history().len());
		}
		assert!(lengths[0] <= lengths[1], "{id}: {lengths:?}");
	}
}

/// Where one walk through a generated question ended, and how it got there.
struct Walk {
	value: Value,
	/// It computed inside an operand that a short circuit standing open at the time skips.
	continued: bool,
	/// It took a short circuit after computing inside the operand that short circuit skips.
	resumed: bool,
}

/// Each allowed step is one a student can work out by hand: never an exception or `None`,
/// a short answer, and a small number.
fn assert_manageable(step: &NextStep, context: &str) {
	let value: &Value = step
		.outcome
		.as_ref()
		.unwrap_or_else(|error| panic!("{context}: {error}"));
	let small: bool = match value {
		Value::Int(number) => number
			.to_i64()
			.is_some_and(|number| (-999..=999).contains(&number)),
		Value::Float(number) => number.abs() <= 999.0,
		Value::Bool(_) => true,
		Value::None => false,
	};
	assert!(small && value.to_string().len() <= 10, "{context}: {value}");
}

/// Works a generated question to its end, taking whichever allowed step `choose` picks, and
/// checks every step allowed at every state the walk passes through.
fn walk(exercise: &Exercise, mut choose: impl FnMut(&Session, &[NextStep]) -> NextStep) -> Walk {
	let mut session: Session = exercise.session().unwrap();
	// Short circuits that stood open while the walk computed inside what they skip.
	let mut passed: BTreeSet<usize> = BTreeSet::new();
	let mut resumed: bool = false;
	while !session.is_finished() {
		let context: String = format!("{} at {}", exercise.expression, session.render());
		let allowed: Vec<NextStep> = session.allowed_steps();
		for step in &allowed {
			assert_manageable(step, &context);
		}
		let hint: NextStep = session.next_step().unwrap();
		assert!(
			allowed.contains(&hint),
			"{context}: the hint is not allowed"
		);
		let step: NextStep = choose(&session, &allowed);
		if short_circuits(&session, &step) {
			resumed |= passed.contains(&step.node_id);
		}
		passed.extend(
			allowed
				.iter()
				.filter(|open| open.node_id != step.node_id && short_circuits(&session, open))
				.filter(|open| {
					let skipped: &Expr = session.root().find(open.node_id).unwrap();
					skipped.find(step.node_id).is_some()
				})
				.map(|open| open.node_id),
		);
		let group: bool = matches!(
			session.root().find(step.node_id).unwrap().kind,
			ExprKind::Group(_)
		);
		let feedback: Feedback = if group {
			session.remove_group(step.node_id)
		} else {
			session.submit(step.node_id, &step.outcome.unwrap().to_string())
		};
		assert!(feedback.accepted(), "{context}: {}", feedback.message);
		assert!(session.history().len() <= 40, "{context}");
	}
	assert!(session.terminal_error().is_none());
	assert!(session.allowed_steps().is_empty());
	assert!(session.next_step().is_none());
	Walk {
		value: session.root().value().unwrap().clone(),
		continued: !passed.is_empty(),
		resumed,
	}
}

#[test]
fn every_mix_of_short_circuit_and_continue_in_a_random_question_stays_manageable() {
	const WALKS: u64 = 6;
	for language in [Language::Python, Language::Logic] {
		let (mut continued, mut resumed): (usize, usize) = (0, 0);
		for seed in (0..128).chain([u64::MAX]) {
			let exercise: Exercise = generate::generate(language, seed).unwrap();
			let reference: Walk = walk(&exercise, |session, _| session.next_step().unwrap());
			for index in 0..WALKS {
				let mut rng: ChaCha8Rng = ChaCha8Rng::seed_from_u64(seed);
				rng.set_stream(index);
				let random: Walk = walk(&exercise, |_, allowed| {
					allowed[rng.gen_range(0..allowed.len())].clone()
				});
				assert!(
					random.value.same_answer(&reference.value)
						&& random.value.type_name() == reference.value.type_name(),
					"{} walk {index}: {} instead of {}",
					exercise.expression,
					random.value,
					reference.value
				);
				continued += usize::from(random.continued);
				resumed += usize::from(random.resumed);
			}
		}
		// The walks leave the route a hint follows: they compute what a short circuit skips,
		// and some short-circuit after computing part of it.
		assert!(
			continued > 0 && resumed > 0,
			"{language:?}: {continued} {resumed}"
		);
	}
}

#[test]
fn only_a_whole_binary_pair_can_automatically_enter_a_blank() {
	for source in ["2 + 3", "False and True", "2 > 1", "-2 ** 2"] {
		let session: Session = python::session(source, &BTreeMap::new()).unwrap();
		assert_eq!(
			session.final_binary_step().is_some(),
			source != "-2 ** 2",
			"{source}"
		);
	}
	for source in [
		"(2 + 3)",
		"2 + (3)",
		"False and (1 / 0)",
		"1 + 2 + 3",
		"((3))",
	] {
		let session: Session = python::session(source, &BTreeMap::new()).unwrap();
		assert!(session.final_binary_step().is_none(), "{source}");
	}
}
