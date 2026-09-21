use std::collections::BTreeMap;

use stepwise::{
	core::{EvaluationMode, FeedbackKind, Language, NodeId, Session, Value},
	generate, logic,
	python::{self, parse_value},
};

fn node(session: &Session, text: &str) -> NodeId {
	let (source, ranges) = session.render_with_ranges();
	session
		.root()
		.rows()
		.into_iter()
		.rev()
		.find(|(_, node)| &source[ranges[&node.id].clone()] == text)
		.unwrap()
		.1
		.id
}

fn answer(session: &mut Session, text: &str, input: &str) {
	let feedback: stepwise::core::Feedback = session.submit(node(session, text), input);
	assert!(feedback.accepted(), "{text}: {}", feedback.message);
}

#[test]
fn any_occurrence_replaces_the_whole_binding_with_one_undoable_answer() {
	let bindings: BTreeMap<String, Value> = BTreeMap::from([
		("x".into(), parse_value("-2").unwrap()),
		("x1".into(), parse_value("3").unwrap()),
	]);
	let mut session: Session =
		python::session("x1 + x ** 2 + (x)", &bindings, EvaluationMode::ShortCircuit).unwrap();
	let last_x: NodeId = node(&session, "x");
	assert_eq!(session.replacement_ids(last_x).len(), 2);
	assert_eq!(session.submit(last_x, "-2.0").kind, FeedbackKind::WrongType);
	assert!(session.history().is_empty());
	answer(&mut session, "x", "-2");
	assert_eq!(session.render(), "x1 + (-2) ** 2 + (-2)");
	assert_eq!(session.history().len(), 1);
	assert_eq!(session.attempts().len(), 1);
	let restored: Session = python::session(session.source(), &bindings, session.mode())
		.unwrap()
		.replay(session.attempts())
		.unwrap();
	assert_eq!(restored.root(), session.root());
	assert_eq!(restored.render(), session.render());
	assert!(session.undo());
	assert_eq!(session.render(), "x1 + x ** 2 + (x)");
	assert_eq!(session.replacement_ids(last_x).len(), 2);
}

#[test]
fn propositions_also_substitute_together_before_other_variables() {
	let mut session: Session = logic::session(
		"Q ∨ (P ∧ P)",
		&BTreeMap::from([("Q".into(), false), ("P".into(), true)]),
		EvaluationMode::Eager,
	)
	.unwrap();
	answer(&mut session, "P", "True");
	assert_eq!(session.render(), "Q ∨ (True ∧ True)");
	assert_eq!(session.attempts().len(), 1);
	assert!(session.undo());
	assert_eq!(session.render(), "Q ∨ (P ∧ P)");
}

/// Short circuit restricts logic the same way it restricts Python: the right disjunction
/// may still be skipped, so it cannot be computed before the conjunction decides.
#[test]
fn logic_short_circuit_defers_the_skippable_side_but_eager_does_not() {
	let source: &str = "(False ∨ False) ∧ (True ∨ True)";
	let mut guarded: Session =
		logic::session(source, &BTreeMap::new(), EvaluationMode::ShortCircuit).unwrap();
	let right: NodeId = node(&guarded, "True ∨ True");
	assert_eq!(guarded.submit(right, "True").kind, FeedbackKind::OutOfOrder);
	answer(&mut guarded, "False ∨ False", "False");
	assert_eq!(guarded.render(), "(False) ∧ (True ∨ True)");
	let mut eager: Session =
		logic::session(source, &BTreeMap::new(), EvaluationMode::Eager).unwrap();
	let right: NodeId = node(&eager, "True ∨ True");
	assert!(eager.submit(right, "True").accepted());
}

#[test]
fn equal_precedence_can_start_on_the_right_without_changing_association() {
	for mode in [EvaluationMode::Eager, EvaluationMode::ShortCircuit] {
		let mut session: Session =
			logic::session("(True ∧ False) ↔ (True ∧ True)", &BTreeMap::new(), mode).unwrap();
		answer(&mut session, "True ∧ True", "True");
		assert_eq!(session.render(), "(True ∧ False) ↔ (True)");
	}
	for (source, first, input, expected) in [
		("(2 + 3) * (4 + 5)", "4 + 5", "9", "(2 + 3) * (9)"),
		("20 // 4 + 3 * 2", "3 * 2", "6", "20 // 4 + 6"),
		(
			"True ∧ False ∨ True ∧ True",
			"True ∧ True",
			"True",
			"True ∧ False ∨ True",
		),
	] {
		let mut session: Session = if source.contains('∧') {
			logic::session(source, &BTreeMap::new(), EvaluationMode::Eager).unwrap()
		} else {
			python::session(source, &BTreeMap::new(), EvaluationMode::ShortCircuit).unwrap()
		};
		answer(&mut session, first, input);
		assert_eq!(session.render(), expected);
	}
	let mut session: Session =
		python::session("20 - 5 - 2", &BTreeMap::new(), EvaluationMode::ShortCircuit).unwrap();
	assert_eq!(
		session.submit(session.root().id, "13").kind,
		FeedbackKind::NeedsInner
	);
	answer(&mut session, "20 - 5", "15");
	answer(&mut session, "15 - 2", "13");
	assert_eq!(session.render(), "13");
}

#[test]
fn lower_precedence_and_unfinished_groups_still_cannot_be_skipped() {
	let mut session: Session = python::session(
		"1 + 2 + 3 * 4",
		&BTreeMap::new(),
		EvaluationMode::ShortCircuit,
	)
	.unwrap();
	assert_eq!(
		session.submit(node(&session, "1 + 2"), "3").kind,
		FeedbackKind::OutOfOrder
	);
	answer(&mut session, "3 * 4", "12");
	answer(&mut session, "1 + 2", "3");
	answer(&mut session, "3 + 12", "15");
	let mut session: Session = python::session(
		"2 * 3 + (4 + 5)",
		&BTreeMap::new(),
		EvaluationMode::ShortCircuit,
	)
	.unwrap();
	assert!(session.check_selection(node(&session, "2 * 3")).is_ok());
	answer(&mut session, "4 + 5", "9");
	assert_eq!(
		session.submit(session.root().id, "15").kind,
		FeedbackKind::NeedsInner
	);
}

#[test]
fn substitution_does_not_execute_a_short_circuited_branch() {
	let bindings: BTreeMap<String, Value> = BTreeMap::from([
		("flag".into(), Value::Bool(false)),
		("x".into(), parse_value("0").unwrap()),
	]);
	let mut session: Session = python::session(
		"flag and (1 / x > 1)",
		&bindings,
		EvaluationMode::ShortCircuit,
	)
	.unwrap();
	answer(&mut session, "x", "0");
	assert!(session.check_selection(node(&session, "1 / 0")).is_err());
	answer(&mut session, "flag", "False");
	assert_eq!(
		session
			.submit(node(&session, "1 / 0"), "ZeroDivisionError")
			.kind,
		FeedbackKind::Skipped
	);
	let root: NodeId = session.root().id;
	assert!(session.submit(root, "False").accepted());
	assert!(session.terminal_error().is_none());
}

#[test]
fn alternate_choices_finish_seeded_questions_with_the_same_values_and_replay() {
	for language in [Language::Python, Language::Logic] {
		for mode in [EvaluationMode::ShortCircuit, EvaluationMode::Eager] {
			for seed in 0..16 {
				let exercise: stepwise::exercises::Exercise =
					generate::generate(language, seed).unwrap();
				let mut recommended: Session = exercise.session(mode).unwrap();
				while let Some(step) = recommended.next_step() {
					assert!(
						recommended
							.submit(step.node_id, &step.outcome.unwrap().to_string())
							.accepted()
					);
				}
				let mut alternative: Session = exercise.session(mode).unwrap();
				while !alternative.is_finished() {
					let selected: NodeId = alternative
						.root()
						.rows()
						.into_iter()
						.rev()
						.find(|(_, node)| alternative.check_selection(node.id).is_ok())
						.unwrap()
						.1
						.id;
					let step: stepwise::core::NextStep =
						stepwise::core::next_step(alternative.root().find(selected).unwrap(), mode)
							.unwrap();
					assert!(
						alternative
							.submit(selected, &step.outcome.unwrap().to_string())
							.accepted()
					);
					assert!(alternative.attempts().len() <= 40);
				}
				assert!(
					recommended
						.root()
						.value()
						.unwrap()
						.same_answer(alternative.root().value().unwrap())
				);
				let restored: Session = exercise
					.session(mode)
					.unwrap()
					.replay(alternative.attempts())
					.unwrap();
				assert_eq!(restored.render(), alternative.render());
				assert_eq!(restored.root(), alternative.root());
			}
		}
	}
}
