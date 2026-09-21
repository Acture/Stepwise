use std::collections::BTreeMap;

use stepwise::{
	core::{EvaluationMode, FeedbackKind, Session, Value},
	exercises::{self, Exercise},
	logic,
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

#[test]
fn a_recommended_path_respects_dependencies_without_enforcing_variable_order() {
	let values: BTreeMap<String, Value> = bindings(&[("x", "2"), ("y", "3"), ("z", "4")]);
	let mut session: Session =
		python::session("x + y * z", &values, EvaluationMode::ShortCircuit).unwrap();
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
		let restored: Session = python::session(session.source(), &values, session.mode())
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
		EvaluationMode::ShortCircuit,
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
	let mut session: Session = python::session(
		"(x) ** 2",
		&bindings(&[("x", "-2")]),
		EvaluationMode::ShortCircuit,
	)
	.unwrap();
	for (input, display) in [("-2", "(-2) ** 2"), ("-2", "(-2) ** 2"), ("4", "4")] {
		answer(&mut session, input);
		assert_eq!(session.render(), display);
	}
	let mut session: Session =
		python::session("not(False)", &BTreeMap::new(), EvaluationMode::ShortCircuit).unwrap();
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
		let mut session: Session = python::session(
			"x",
			&bindings(&[("x", value)]),
			EvaluationMode::ShortCircuit,
		)
		.unwrap();
		assert!(!session.submit(session.root().id, wrong).accepted());
		assert!(session.history().is_empty());
		answer(&mut session, value);
	}
	assert!(python::session("x + 1", &BTreeMap::new(), EvaluationMode::ShortCircuit).is_err());
	assert!(
		python::session(
			"x + 1",
			&bindings(&[("x", "2"), ("typo", "3")]),
			EvaluationMode::ShortCircuit
		)
		.is_err()
	);
	assert!(
		python::session(
			"x",
			&BTreeMap::from([("x".into(), Value::Float(f64::INFINITY))]),
			EvaluationMode::ShortCircuit
		)
		.is_err()
	);
}

#[test]
fn short_circuit_skips_variable_reads_and_groups_but_eager_does_not() {
	let source: &str = "flag and ((x / zero > limit))";
	let values: BTreeMap<String, Value> =
		bindings(&[("flag", "False"), ("x", "3"), ("zero", "0"), ("limit", "1")]);
	let mut short: Session =
		python::session(source, &values, EvaluationMode::ShortCircuit).unwrap();
	answer(&mut short, "False");
	let x: usize = short
		.root()
		.rows()
		.into_iter()
		.find(|(_, node)| node.render() == "x")
		.unwrap()
		.1
		.id;
	assert!(short.check_selection(x).is_ok());
	assert!(short.final_binary_step().is_none());
	answer(&mut short, "False");
	assert_eq!(short.history().len(), 2);
	let mut eager: Session = python::session(source, &values, EvaluationMode::Eager).unwrap();
	for input in ["False", "3", "0", "ZeroDivisionError"] {
		answer(&mut eager, input);
	}
	assert!(eager.is_finished());
	assert!(eager.terminal_error().is_some());
}

#[test]
fn all_five_logic_precedences_are_generated_from_source() {
	let mut session: Session = logic::session(
		"¬P ∧ Q ∨ R → S ↔ U",
		&BTreeMap::from([
			("P".into(), true),
			("Q".into(), true),
			("R".into(), false),
			("S".into(), false),
			("U".into(), true),
		]),
		EvaluationMode::Eager,
	)
	.unwrap();
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
		EvaluationMode::Eager,
	)
	.unwrap();
	for (unit, input, display) in [
		("P", "False", "((False)) => Q"),
		("(False)", "False", "(False) => Q"),
		("(False)", "False", "False => Q"),
		("Q", "True", "False => True"),
		("False => True", "True", "True"),
	] {
		assert_eq!(selected(&session), unit);
		answer(&mut session, input);
		assert_eq!(session.render(), display);
	}
	assert!(session.is_finished());
}

#[test]
fn long_exercises_use_the_same_rules_in_both_modes() {
	let exercises: Vec<Exercise> = exercises::builtin().unwrap();
	for (id, expected) in [
		("long-arithmetic", "-0.5"),
		("long-power", "-504.0"),
		("long-python-logic", "True"),
		("long-logic", "False"),
	] {
		let exercise: &Exercise = exercises.iter().find(|exercise| exercise.id == id).unwrap();
		for mode in [EvaluationMode::ShortCircuit, EvaluationMode::Eager] {
			let mut session: Session = exercise.session(mode).unwrap();
			while let Some(step) = session.next_step() {
				answer(&mut session, &step.outcome.unwrap().to_string());
				assert!(session.history().len() < 128);
			}
			assert_eq!(session.render(), expected, "{id} {mode:?}");
			assert!(session.history().len() > 10);
		}
	}
}

#[test]
fn only_a_whole_binary_pair_can_automatically_enter_a_blank() {
	for source in ["2 + 3", "False and True", "2 > 1", "-2 ** 2"] {
		let session: Session =
			python::session(source, &BTreeMap::new(), EvaluationMode::ShortCircuit).unwrap();
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
		let session: Session =
			python::session(source, &BTreeMap::new(), EvaluationMode::ShortCircuit).unwrap();
		assert!(session.final_binary_step().is_none(), "{source}");
	}
}
