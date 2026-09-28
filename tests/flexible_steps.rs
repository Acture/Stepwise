use std::collections::BTreeMap;

use stepwise::{
	core::{FeedbackKind, Language, NextStep, NodeId, Session, Value},
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

/// The allowed steps as the displayed text they reduce, sorted so the test names a set.
fn allowed(session: &Session) -> Vec<String> {
	let (source, ranges) = session.render_with_ranges();
	let mut steps: Vec<String> = session
		.allowed_steps()
		.into_iter()
		.map(|step| source[ranges[&step.node_id].clone()].to_string())
		.collect();
	steps.sort();
	steps
}

/// What a student types for a step: its value, or the name of the exception it raises.
fn typed(step: &NextStep) -> String {
	match &step.outcome {
		Ok(value) => value.to_string(),
		Err(error) => error
			.name()
			.expect("a step raises a nameable exception")
			.into(),
	}
}

#[test]
fn any_occurrence_replaces_the_whole_binding_with_one_undoable_answer() {
	let bindings: BTreeMap<String, Value> = BTreeMap::from([
		("x".into(), parse_value("-2").unwrap()),
		("x1".into(), parse_value("3").unwrap()),
	]);
	let mut session: Session = python::session("x1 + x ** 2 + (x)", &bindings).unwrap();
	let last_x: NodeId = node(&session, "x");
	assert_eq!(session.replacement_ids(last_x).len(), 2);
	assert_eq!(session.submit(last_x, "-2.0").kind, FeedbackKind::WrongType);
	assert!(session.history().is_empty());
	answer(&mut session, "x", "-2");
	assert_eq!(session.render(), "x1 + (-2) ** 2 + (-2)");
	assert_eq!(session.history().len(), 1);
	assert_eq!(session.attempts().len(), 1);
	let restored: Session = python::session(session.source(), &bindings)
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
	)
	.unwrap();
	answer(&mut session, "P", "True");
	assert_eq!(session.render(), "Q ∨ (True ∧ True)");
	assert_eq!(session.attempts().len(), 1);
	assert!(session.undo());
	assert_eq!(session.render(), "Q ∨ (P ∧ P)");
}

/// Logic short-circuits the way Python does, and in the same session the side a short
/// circuit would skip may still be computed, before or after the left side decides.
#[test]
fn logic_may_compute_the_skippable_side_or_short_circuit_past_it() {
	let source: &str = "(False ∨ False) ∧ (True ∨ True)";
	let mut session: Session = logic::session(source, &BTreeMap::new()).unwrap();
	answer(&mut session, "True ∨ True", "True");
	assert_eq!(session.render(), "(False ∨ False) ∧ (True)");
	assert!(session.undo());
	answer(&mut session, "False ∨ False", "False");
	answer(&mut session, "(False)", "False");
	assert_eq!(session.render(), "False ∧ (True ∨ True)");
	assert_eq!(allowed(&session), ["False ∧ (True ∨ True)", "True ∨ True"]);
	let root: NodeId = session.root().id;
	assert!(session.submit(root, "False").accepted());
	assert_eq!(session.render(), "False");
	assert!(session.is_finished());
}

/// Every step open to a student, for one state: the short circuit wherever the operands
/// already decide it, the operands it would skip, and any mix of the two, within precedence.
#[test]
fn allowed_steps_open_both_the_short_circuit_and_the_operands_it_skips() {
	for (source, expected) in [
		(
			"False and (2 + 3 > 1)",
			vec!["2 + 3", "False and (2 + 3 > 1)"],
		),
		// The short circuit is not held back by higher precedence inside what it skips.
		("False and 2 + 3 > 1", vec!["2 + 3", "False and 2 + 3 > 1"]),
		// Continue at `or`, short-circuit at `and`: a mix neither fixed route takes.
		(
			"True or (False and 4 > 5)",
			vec!["4 > 5", "False and 4 > 5", "True or (False and 4 > 5)"],
		),
		(
			"(False and (4 > 5)) or ((1 + 1) == 2)",
			vec!["1 + 1", "4 > 5", "False and (4 > 5)"],
		),
		// Undecided, `and` needs its right side, so only the work inside it is open.
		("True and (2 + 3 > 1)", vec!["2 + 3"]),
		// Precedence still holds outside the operand a short circuit skips.
		("2 * 3 > 1 or False and True", vec!["2 * 3"]),
	] {
		let session: Session = python::session(source, &BTreeMap::new()).unwrap();
		assert_eq!(allowed(&session), expected, "{source}");
		let hint: NodeId = session.next_step().unwrap().node_id;
		assert!(
			session
				.allowed_steps()
				.iter()
				.any(|step| step.node_id == hint),
			"{source}"
		);
	}
	let mut session: Session = python::session("False and (2 + 3 > 1)", &BTreeMap::new()).unwrap();
	answer(&mut session, "2 + 3", "5");
	assert_eq!(allowed(&session), ["5 > 1", "False and (5 > 1)"]);
	let root: NodeId = session.root().id;
	assert!(session.submit(root, "False").accepted());
	assert!(session.allowed_steps().is_empty());
	let session: Session = python::session("True and (2 + 3 > 1)", &BTreeMap::new()).unwrap();
	let feedback: stepwise::core::Feedback = session.check_attempt(session.root().id, "True");
	assert_eq!(feedback.kind, FeedbackKind::NeedsInner);
	assert_eq!(
		feedback.message,
		"这一步不能跳过内部运算。请先计算 2 + 3，再回到当前表达式。"
	);
	let session: Session =
		python::session("2 * 3 > 1 or False and True", &BTreeMap::new()).unwrap();
	let feedback: stepwise::core::Feedback =
		session.check_attempt(node(&session, "False and True"), "False");
	assert_eq!(feedback.kind, FeedbackKind::OutOfOrder);
	assert_eq!(
		feedback.message,
		"这里目前还不能计算，请先处理 2 * 3。同优先级的独立子式可以任选。"
	);
}

#[test]
fn equal_precedence_can_start_on_the_right_without_changing_association() {
	let mut session: Session =
		logic::session("(True ∧ False) ↔ (True ∧ True)", &BTreeMap::new()).unwrap();
	answer(&mut session, "True ∧ True", "True");
	assert_eq!(session.render(), "(True ∧ False) ↔ (True)");
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
			logic::session(source, &BTreeMap::new()).unwrap()
		} else {
			python::session(source, &BTreeMap::new()).unwrap()
		};
		answer(&mut session, first, input);
		assert_eq!(session.render(), expected);
	}
	let mut session: Session = python::session("20 - 5 - 2", &BTreeMap::new()).unwrap();
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
	let mut session: Session = python::session("1 + 2 + 3 * 4", &BTreeMap::new()).unwrap();
	let feedback: stepwise::core::Feedback = session.submit(node(&session, "1 + 2"), "3");
	assert_eq!(feedback.kind, FeedbackKind::OutOfOrder);
	assert_eq!(
		feedback.message,
		"这里目前还不能计算，请先处理 3 * 4。同优先级的独立子式可以任选。"
	);
	answer(&mut session, "3 * 4", "12");
	answer(&mut session, "1 + 2", "3");
	answer(&mut session, "3 + 12", "15");
	let mut session: Session = python::session("2 * 3 + (4 + 5)", &BTreeMap::new()).unwrap();
	assert!(session.check_selection(node(&session, "2 * 3")).is_ok());
	answer(&mut session, "4 + 5", "9");
	assert_eq!(
		session.submit(session.root().id, "15").kind,
		FeedbackKind::NeedsInner
	);
}

/// Substituting a variable inside the branch a short circuit skips does not run that branch.
/// Running it is the student's own choice: its exception ends the question, undo takes it
/// back, and the short circuit still finishes with an ordinary value.
#[test]
fn substitution_does_not_execute_a_short_circuited_branch() {
	let bindings: BTreeMap<String, Value> = BTreeMap::from([
		("flag".into(), Value::Bool(false)),
		("x".into(), parse_value("0").unwrap()),
	]);
	let mut session: Session = python::session("flag and (1 / x > 1)", &bindings).unwrap();
	answer(&mut session, "x", "0");
	assert!(session.terminal_error().is_none());
	answer(&mut session, "flag", "False");
	assert!(session.terminal_error().is_none());
	let division: NodeId = node(&session, "1 / 0");
	assert!(session.check_selection(division).is_ok());
	assert!(
		session.submit(division, "ZeroDivisionError").accepted(),
		"the operand a short circuit skips may still be computed"
	);
	assert_eq!(
		session.terminal_error().unwrap().name(),
		Some("ZeroDivisionError")
	);
	assert!(session.undo());
	let root: NodeId = session.root().id;
	assert!(session.submit(root, "False").accepted());
	assert!(session.terminal_error().is_none());
	assert_eq!(session.render(), "False");
}

/// A hint names one route; a student may take any mix of short circuits and computed
/// operands instead. Whatever is chosen from the allowed steps, a seeded question finishes
/// with the reference walk's typed value, takes no fewer answers than it and no more than
/// generation bounds, and replays to the same state.
#[test]
fn alternate_choices_finish_seeded_questions_with_the_same_values_and_replay() {
	type Choose = fn(u64, &Session, &[NextStep]) -> usize;
	let choices: [(&str, Choose); 3] = [
		// The deepest step first computes what a short circuit would skip wherever it can.
		("deepest", |_, session, steps| {
			let rows: Vec<(usize, &stepwise::core::Expr)> = session.root().rows();
			(0..steps.len())
				.max_by_key(|&index| {
					rows.iter()
						.find(|(_, node)| node.id == steps[index].node_id)
						.unwrap()
						.0
				})
				.unwrap()
		}),
		("last", |_, _, steps| steps.len() - 1),
		("mixed", |seed, session, steps| {
			(seed as usize * 7 + session.attempts().len() * 3) % steps.len()
		}),
	];
	for language in [Language::Python, Language::Logic] {
		for seed in 0..64 {
			let exercise: stepwise::exercises::Exercise =
				generate::generate(language, seed).unwrap();
			let mut recommended: Session = exercise.session().unwrap();
			while let Some(step) = recommended.next_step() {
				assert!(recommended.submit(step.node_id, &typed(&step)).accepted());
			}
			assert!(recommended.terminal_error().is_none());
			for (policy, choose) in choices {
				let mut alternative: Session = exercise.session().unwrap();
				while !alternative.is_finished() {
					let steps: Vec<NextStep> = alternative.allowed_steps();
					let hint: NodeId = alternative.next_step().unwrap().node_id;
					assert!(steps.iter().any(|step| step.node_id == hint));
					let step: &NextStep = &steps[choose(seed, &alternative, &steps)];
					let feedback: stepwise::core::Feedback =
						alternative.submit(step.node_id, &typed(step));
					assert!(
						feedback.accepted(),
						"{language:?} {seed} {policy}: {}",
						feedback.message
					);
					assert!(alternative.attempts().len() <= 40);
				}
				assert!(alternative.allowed_steps().is_empty());
				assert!(
					alternative.terminal_error().is_none(),
					"{language:?} {seed} {policy}"
				);
				assert!(
					recommended
						.root()
						.value()
						.unwrap()
						.same_answer(alternative.root().value().unwrap()),
					"{language:?} {seed} {policy}"
				);
				assert!(recommended.attempts().len() <= alternative.attempts().len());
				let restored: Session = exercise
					.session()
					.unwrap()
					.replay(alternative.attempts())
					.unwrap();
				assert_eq!(restored.render(), alternative.render());
				assert_eq!(restored.root(), alternative.root());
			}
		}
	}
}
