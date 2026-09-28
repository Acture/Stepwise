use std::collections::BTreeMap;

use stepwise::{
	core::{EvalError, Feedback, FeedbackKind, NodeId, Session, Value},
	python::{self, parse_expression, parse_value},
};

fn session(source: &str) -> Session {
	python::session(source, &BTreeMap::new()).unwrap()
}

#[test]
fn displayed_parentheses_survive_replacement_undo_and_replay() {
	let mut practice: Session = session("((2 + (3*4)))");
	assert_eq!(practice.render(), "((2 + (3*4)))");
	let multiplication: NodeId = node(&practice, "3 * 4");
	let (text, ranges) = practice.render_with_ranges();
	assert_eq!(&text[ranges[&multiplication].clone()], "3*4");
	assert!(practice.submit(multiplication, "12").accepted());
	assert_eq!(practice.render(), "((2 + (12)))");
	assert_eq!(practice.history()[0].before, "((2 + (3*4)))");
	let restored: Session = session(practice.source())
		.replay(practice.attempts())
		.unwrap();
	assert_eq!(restored.render(), practice.render());
	for (selected, answer, display) in [
		("(12)", "12", "((2 + 12))"),
		("2 + 12", "14", "((14))"),
		("(14)", "14", "(14)"),
		("(14)", "14", "14"),
	] {
		assert!(!practice.is_finished());
		assert!(
			practice
				.submit(node(&practice, selected), answer)
				.accepted()
		);
		assert_eq!(practice.render(), display);
	}
	assert!(practice.is_finished());
	for _ in 0..5 {
		assert!(practice.undo());
	}
	assert_eq!(practice.render(), "((2 + (3*4)))");
}

#[test]
fn displayed_sibling_spans_shift_after_each_replacement() {
	let mut practice: Session = session("(20+30) * (4+5)");
	let left: NodeId = node(&practice, "20 + 30");
	let right: NodeId = node(&practice, "4 + 5");
	assert!(practice.submit(left, "50").accepted());
	assert_eq!(practice.render(), "(50) * (4+5)");
	let (text, ranges) = practice.render_with_ranges();
	assert_eq!(&text[ranges[&right].clone()], "4+5");
	assert!(practice.submit(right, "9").accepted());
	assert_eq!(practice.render(), "(50) * (9)");
	assert!(practice.submit(node(&practice, "(50)"), "50").accepted());
	assert_eq!(practice.render(), "50 * (9)");
	assert!(practice.submit(node(&practice, "(9)"), "9").accepted());
	assert!(practice.submit(practice.root().id, "450").accepted());
	assert_eq!(practice.render(), "450");
}
fn node(session: &Session, expression: &str) -> NodeId {
	session
		.root()
		.rows()
		.into_iter()
		.find(|(_, node)| node.render() == expression)
		.unwrap_or_else(|| panic!("missing {expression} in {}", session.root().render()))
		.1
		.id
}

#[test]
fn strict_selection_and_no_skipping() {
	let mut practice: Session = session("(2 + 3) * (4 + 5)");
	let before: String = practice.root().render();
	assert!(practice.check_selection(node(&practice, "4 + 5")).is_ok());
	assert_eq!(
		practice.submit(practice.root().id, "45").kind,
		FeedbackKind::NeedsInner
	);
	assert_eq!(
		practice.submit(node(&practice, "2"), "2").kind,
		FeedbackKind::AlreadyValue
	);
	assert_eq!(
		practice.submit(node(&practice, "2 + 3"), "6").kind,
		FeedbackKind::WrongValue
	);
	assert_eq!(practice.root().render(), before);
	assert!(practice.history().is_empty());
	let left: NodeId = node(&practice, "2 + 3");
	let right: NodeId = node(&practice, "4 + 5");
	assert!(practice.submit(left, "5").accepted());
	assert_eq!(
		practice
			.root()
			.find(left)
			.unwrap()
			.value()
			.unwrap()
			.to_string(),
		"5"
	);
	assert!(practice.submit(node(&practice, "(5)"), "5").accepted());
	assert_eq!(practice.next_step().unwrap().node_id, right);
	assert!(practice.submit(right, "9").accepted());
	assert!(practice.submit(node(&practice, "(9)"), "9").accepted());
	assert!(practice.submit(practice.root().id, "45").accepted());
	assert!(practice.is_finished());
	assert_eq!(practice.history().len(), 5);
	assert!(practice.undo());
	assert_eq!(practice.root().render(), "5 * 9");
}

/// One session holds both routes: the short circuit answers with its own type, and the
/// operand it skips may still be computed, raising only because the student chose it.
#[test]
fn short_circuit_types_and_a_chosen_exception_are_distinct() {
	let mut practice: Session = session("False and (3 / 0 > 1)");
	assert_eq!(practice.submit(0, "0").kind, FeedbackKind::WrongType);
	let division: NodeId = node(&practice, "3 / 0");
	let feedback: Feedback = practice.submit(division, "ZeroDivisionError");
	assert!(feedback.accepted(), "{}", feedback.message);
	let error: &EvalError = practice.terminal_error().unwrap();
	assert_eq!(error.name(), Some("ZeroDivisionError"));
	assert_eq!(
		feedback.message,
		format!(
			"正确。{error} 这个异常来自你选择计算的子式 3 / 0；Python 执行原式时不会在这里引发它。求值在这里终止。"
		)
	);
	assert!(practice.is_finished());
	assert!(practice.allowed_steps().is_empty());
	assert_eq!(practice.submit(0, "False").kind, FeedbackKind::Finished);
	assert!(practice.undo());
	assert!(!practice.is_finished());
	assert!(practice.submit(0, "False").accepted());
	assert!(practice.terminal_error().is_none());
	assert_eq!(practice.history().len(), 1);
	let undecided: Session = session("True and (3 / 0 > 1)");
	assert_eq!(
		undecided.check_attempt(0, "False").kind,
		FeedbackKind::NeedsInner
	);
}

/// The exception wording says whether Python itself would raise there, running the question
/// on in its own order from the state the student reached.
#[test]
fn a_chosen_exception_says_whether_python_raises_it_there() {
	for (source, selected, origin) in [
		("True and (3 / 0 > 1)", "3 / 0", ""),
		(
			"(1 / 0) + (2 % 0)",
			"2 % 0",
			"这个异常来自你选择计算的子式 2 % 0；Python 执行原式时不会在这里引发它。",
		),
		// A limit of this program stops the reference first, so nothing is said of Python.
		(
			"(2 ** 99999999999999) + (1 / 0)",
			"1 / 0",
			"这个异常来自你选择计算的子式 1 / 0。",
		),
	] {
		let mut practice: Session = session(source);
		let feedback: Feedback = practice.submit(node(&practice, selected), "ZeroDivisionError");
		assert!(feedback.accepted(), "{source}: {}", feedback.message);
		let error: &EvalError = practice.terminal_error().unwrap();
		assert_eq!(
			feedback.message,
			format!("正确。{error} {origin}求值在这里终止。"),
			"{source}"
		);
	}
}

/// Computing the operand a short circuit skips changes neither its result nor its type. While
/// part of it is still unreduced the short circuit skips that subtree; once all of it is a
/// value the wording says Python itself would not have computed it, so the extra evaluation
/// is never passed off as Python's own order.
#[test]
fn a_computed_operand_leaves_the_short_circuit_result_and_names_the_extra_evaluation() {
	let skipping: &str =
		"正确。短路求值：or 遇到真值 True，直接返回该操作数（bool），跳过后面的子树。";
	let mut direct: Session = session("True or (1 + 1)");
	assert_eq!(direct.submit(0, "True").message, skipping);

	let mut practice: Session = session("True or (1 + 1)");
	assert!(practice.submit(node(&practice, "1 + 1"), "2").accepted());
	assert_eq!(practice.render(), "True or (2)");
	assert_eq!(practice.check_attempt(0, "True").message, skipping);
	assert!(practice.submit(node(&practice, "(2)"), "2").accepted());
	assert_eq!(practice.render(), "True or 2");
	assert_eq!(practice.submit(0, "2").kind, FeedbackKind::WrongType);
	let feedback: Feedback = practice.submit(0, "True");
	assert!(feedback.accepted());
	assert_eq!(
		feedback.message,
		"正确。短路求值：or 遇到真值 True，直接返回该操作数（bool）；后面的操作数不影响结果，Python 执行时不会计算它们。"
	);
}

#[test]
fn exact_types_and_signed_zero() {
	let mut practice: Session = session("6 / 3");
	assert_eq!(practice.submit(0, "2").kind, FeedbackKind::WrongType);
	assert_eq!(practice.submit(0, "1 + 1").kind, FeedbackKind::InvalidInput);
	assert!(practice.submit(0, "2.0").accepted());
	assert!(
		!parse_value("-0.0")
			.unwrap()
			.same_answer(&parse_value("0.0").unwrap())
	);
	assert_ne!(parse_value("True").unwrap(), parse_value("1").unwrap());
}

#[test]
fn precedence_and_rules_have_expected_step_sequences() {
	for (source, expected) in [
		("-2 ** 2", vec!["2 ** 2"]),
		("--10", vec!["-10", "-(-10)"]),
		("-True", vec!["-True"]),
		("+10", vec!["+10"]),
		("-(10)", vec!["(10)"]),
		("(-10)", vec!["-10", "(-10)"]),
		("(-2) ** 2", vec!["-2", "(-2)", "(-2) ** 2"]),
		("2 ** 3 ** 2", vec!["3 ** 2", "2 ** 9"]),
		("-7 // 3", vec!["-7", "(-7) // 3"]),
		("True or (1 / 0)", vec!["True or (1 / 0)"]),
		(
			"(0 or 5) and (2 + 3)",
			vec!["0 or 5", "(5)", "2 + 3", "(5)", "5 and 5"],
		),
	] {
		let mut practice: Session = session(source);
		let mut seen: Vec<String> = Vec::new();
		while let Some(step) = practice.next_step() {
			seen.push(practice.root().find(step.node_id).unwrap().render());
			let value: Value = step.outcome.unwrap();
			assert!(practice.submit(step.node_id, &value.to_string()).accepted());
		}
		assert_eq!(seen, expected, "{source}");
	}
}

#[test]
fn final_negative_literals_are_values_without_recorded_answers() {
	for source in ["-10", "-10.5", "-0", "-0.0", "- 10"] {
		let mut practice: Session = session(source);
		assert!(practice.is_finished(), "{source}");
		assert!(practice.next_step().is_none());
		assert!(practice.allowed_steps().is_empty());
		assert!(practice.history().is_empty());
		assert!(practice.attempts().is_empty());
		assert!(
			practice
				.root()
				.value()
				.unwrap()
				.same_answer(&parse_value(source).unwrap())
		);
		assert!(!practice.undo());
	}
}

#[test]
fn final_negative_completion_keeps_validation_and_undo_at_the_last_student_step() {
	let mut practice: Session = session("-2 ** 2");
	let power: NodeId = practice.next_step().unwrap().node_id;
	assert_eq!(practice.submit(power, "5").kind, FeedbackKind::WrongValue);
	assert!(!practice.is_finished());
	assert!(practice.submit(power, "4").accepted());
	assert_eq!(practice.render(), "-4");
	assert!(practice.is_finished());
	assert_eq!(practice.attempts().len(), 1);
	let restored: Session = session(practice.source())
		.replay(practice.attempts())
		.unwrap();
	assert_eq!(restored.root(), practice.root());
	assert_eq!(restored.render(), practice.render());
	assert!(restored.is_finished());
	assert!(practice.undo());
	assert_eq!(practice.render(), "-2 ** 2");
	assert_eq!(practice.next_step().unwrap().node_id, power);
	assert!(practice.submit(power, "4").accepted());
	assert!(practice.is_finished());
}

#[test]
fn rejects_unsupported_source_and_resource_limits() {
	for source in [
		"x + 1",
		"print(1)",
		"[1, 2]",
		"1 < 2 < 3",
		"'hi'",
		"1j",
		"1e999",
		"1 if True else 2",
	] {
		assert!(parse_expression(source).is_err(), "{source}");
	}
	assert!(parse_expression(&format!("{}1{}", "(".repeat(33), ")".repeat(33))).is_err());
	let mut practice: Session = session("2 ** 99999999999999");
	assert_eq!(practice.submit(0, "0").kind, FeedbackKind::Unsupported);
	assert!(!practice.is_finished());
}

#[test]
fn replay_preserves_ids_and_rejects_invalid_attempts() {
	let mut practice: Session = session("2 + 3 * 4");
	let multiplication: NodeId = node(&practice, "3 * 4");
	assert!(practice.submit(multiplication, "12").accepted());
	let resumed: Session = session(practice.source())
		.replay(practice.attempts())
		.unwrap();
	assert_eq!(resumed.root(), practice.root());
	assert!(session("2 + 5").replay(practice.attempts()).is_err());
}
