//! Short circuit or continue, with no mode to switch (P-741). Wherever the operands already
//! decide an `and`, `or`, `∧`, `∨` or `→`, one session lets the student take the short circuit
//! or keep computing the operands it would skip, and still take it after computing part of
//! one. Every scenario runs through the public API only: the core session each language
//! builds, and the app layer a front end drives.

use std::collections::BTreeMap;

use stepwise::{
	app::{Course, Lesson, Practice, ProofPractice, Report, Task, resume_in_set},
	core::{Feedback, FeedbackKind, Language, NextStep, NodeId, RecordedAttempt, Session, Value},
	exercises::{self, Exercise, Question, QuestionSet},
	generate, logic,
	progress::Progress,
	python::{self, parse_value},
};

const AND: &str = "False and (2 + 3 > 1)";
const OR: &str = "True or (2 + 3 > 1)";

fn python(source: &str) -> Session {
	python::session(source, &BTreeMap::new()).unwrap()
}

fn python_with(source: &str, bindings: &[(&str, &str)]) -> Session {
	let bindings: BTreeMap<String, Value> = bindings
		.iter()
		.map(|(name, literal)| ((*name).into(), parse_value(literal).unwrap()))
		.collect();
	python::session(source, &bindings).unwrap()
}

fn logic_with(source: &str, bindings: &[(&str, bool)]) -> Session {
	let bindings: BTreeMap<String, bool> = bindings
		.iter()
		.map(|(name, value)| ((*name).into(), *value))
		.collect();
	logic::session(source, &bindings).unwrap()
}

/// The displayed text of a node, exactly as the student sees it.
fn shown(session: &Session, node_id: NodeId) -> String {
	let (text, ranges) = session.render_with_ranges();
	text[ranges[&node_id].clone()].into()
}

/// Every node displayed as `text`, in display order.
fn occurrences(session: &Session, text: &str) -> Vec<NodeId> {
	session
		.root()
		.rows()
		.into_iter()
		.map(|(_, node)| node.id)
		.filter(|node_id| shown(session, *node_id) == text)
		.collect()
}

/// The one node displayed as `text`. Two would leave the test guessing, so that fails too.
fn node(session: &Session, text: &str) -> NodeId {
	let found: Vec<NodeId> = occurrences(session, text);
	assert_eq!(found.len(), 1, "{text} in {}", session.render());
	found[0]
}

/// Every step the student may submit now, as the displayed text of its node, sorted.
fn allowed(session: &Session) -> Vec<String> {
	let mut texts: Vec<String> = session
		.allowed_steps()
		.iter()
		.map(|step| shown(session, step.node_id))
		.collect();
	texts.sort();
	texts
}

fn texts(expected: &[&str]) -> Vec<String> {
	let mut texts: Vec<String> = expected.iter().map(|text| (*text).into()).collect();
	texts.sort();
	texts
}

/// Answer at the node displayed as `text`; `None` takes a finished bracket off with no answer.
fn take(session: &mut Session, text: &str, input: Option<&str>) -> Feedback {
	let node_id: NodeId = node(session, text);
	match input {
		Some(input) => session.submit(node_id, input),
		None => session.remove_group(node_id),
	}
}

/// One accepted student step, and the expression it leaves on screen.
fn step(session: &mut Session, text: &str, input: Option<&str>, after: &str) {
	let feedback: Feedback = take(session, text, input);
	assert!(
		feedback.accepted(),
		"{text} ← {input:?}: {}",
		feedback.message
	);
	assert_eq!(session.render(), after);
}

/// Everything a rejected answer must leave exactly as it was: the expression on screen, the
/// history, the recorded attempts and the steps still allowed.
type Snapshot = (String, usize, Vec<RecordedAttempt>, Vec<String>);

fn snapshot(session: &Session) -> Snapshot {
	(
		session.render().into(),
		session.history().len(),
		session.attempts().to_vec(),
		allowed(session),
	)
}

/// A rejected answer of this kind that leaves the session exactly where it was.
fn rejected(session: &mut Session, text: &str, input: &str, kind: FeedbackKind) -> Feedback {
	let before: Snapshot = snapshot(session);
	let feedback: Feedback = take(session, text, Some(input));
	assert_eq!(
		feedback.kind, kind,
		"{text} ← {input}: {}",
		feedback.message
	);
	assert_eq!(snapshot(session), before);
	assert!(session.terminal_error().is_none());
	feedback
}

/// Finished on an ordinary value: this very value, of this very type, and nothing left.
fn finished_with(session: &Session, literal: &str, type_name: &str) {
	assert!(session.is_finished());
	assert!(session.terminal_error().is_none());
	assert!(session.allowed_steps().is_empty());
	assert_eq!(session.next_step(), None);
	let value: &Value = session.root().value().expect("finished on a value");
	assert_eq!(value, &parse_value(literal).unwrap());
	assert_eq!(value.type_name(), type_name);
	assert_eq!(session.render(), literal);
}

/// A Python question on its own, as a custom expression is: it belongs to no set.
fn exercise(name: &str, expression: &str) -> Exercise {
	Exercise {
		set: String::new(),
		name: name.into(),
		title: name.into(),
		language: Language::Python,
		expression: expression.into(),
		note: None,
		bindings: BTreeMap::new(),
	}
}

/// The sentence the teaching rules worded; a reason in its place is the failure.
fn taught(report: &Report) -> &str {
	match report {
		Report::Taught { message, .. } => message,
		other => panic!("expected a sentence from the teaching rules, got {other:?}"),
	}
}

/// Select a node as a click does and type the answer into the blank it opens.
fn answer(practice: &mut Practice, text: &str, input: &str) {
	let node_id: NodeId = node(practice.session(), text);
	assert!(!practice.select(node_id));
	assert_eq!(practice.draft(), Some(node_id), "{:?}", practice.report());
	practice.paste(input);
	assert!(
		practice.submit(),
		"{text} ← {input}: {:?}",
		practice.report()
	);
	assert!(practice.report().good());
}

fn evaluating(lesson: &Lesson) -> &Practice {
	match lesson.task() {
		Task::Evaluation(practice) => practice,
		Task::Proof(practice) => panic!("expected an evaluation, got {}", practice.question().name),
	}
}

fn evaluating_mut(lesson: &mut Lesson) -> &mut Practice {
	match lesson.task_mut() {
		Task::Evaluation(practice) => practice,
		Task::Proof(practice) => panic!("expected an evaluation, got {}", practice.question().name),
	}
}

fn proving(lesson: &Lesson) -> &ProofPractice {
	match lesson.task() {
		Task::Proof(practice) => practice,
		Task::Evaluation(practice) => panic!("expected a proof, got {}", practice.question().name),
	}
}

#[test]
fn a_decided_and_short_circuits_computes_part_or_computes_all_to_the_same_bool() {
	let initial: Session = python(AND);
	assert_eq!(allowed(&initial), texts(&[AND, "2 + 3"]));

	let mut short: Session = initial.clone();
	step(&mut short, AND, Some("False"), "False");
	assert_eq!(short.history().len(), 1);
	finished_with(&short, "False", "bool");

	// Computing part of an operand does not commit to computing all of it.
	let mut part: Session = initial.clone();
	step(&mut part, "2 + 3", Some("5"), "False and (5 > 1)");
	assert_eq!(allowed(&part), texts(&["False and (5 > 1)", "5 > 1"]));
	step(&mut part, "False and (5 > 1)", Some("False"), "False");
	assert_eq!(part.history().len(), 2);
	finished_with(&part, "False", "bool");

	let mut all: Session = initial.clone();
	step(&mut all, "2 + 3", Some("5"), "False and (5 > 1)");
	step(&mut all, "5 > 1", Some("True"), "False and (True)");
	assert_eq!(allowed(&all), texts(&["False and (True)", "(True)"]));
	step(&mut all, "(True)", None, "False and True");
	step(&mut all, "False and True", Some("False"), "False");
	assert_eq!(all.history().len(), 4);
	assert_eq!(all.attempts()[2].input, None);
	finished_with(&all, "False", "bool");
}

#[test]
fn a_decided_or_short_circuits_computes_part_or_computes_all_to_the_same_bool() {
	let initial: Session = python(OR);
	assert_eq!(allowed(&initial), texts(&[OR, "2 + 3"]));

	let mut short: Session = initial.clone();
	step(&mut short, OR, Some("True"), "True");
	assert_eq!(short.history().len(), 1);
	finished_with(&short, "True", "bool");

	let mut part: Session = initial.clone();
	step(&mut part, "2 + 3", Some("5"), "True or (5 > 1)");
	assert_eq!(allowed(&part), texts(&["True or (5 > 1)", "5 > 1"]));
	step(&mut part, "True or (5 > 1)", Some("True"), "True");
	assert_eq!(part.history().len(), 2);
	finished_with(&part, "True", "bool");

	let mut all: Session = initial.clone();
	step(&mut all, "2 + 3", Some("5"), "True or (5 > 1)");
	step(&mut all, "5 > 1", Some("True"), "True or (True)");
	step(&mut all, "(True)", None, "True or True");
	step(&mut all, "True or True", Some("True"), "True");
	assert_eq!(all.history().len(), 4);
	finished_with(&all, "True", "bool");
}

#[test]
fn logic_connectives_short_circuit_or_continue_and_equivalence_never_does() {
	let bindings: [(&str, bool); 3] = [("P", false), ("Q", true), ("R", false)];
	let mut initial: Session = logic_with("P ∧ (Q ∨ R)", &bindings);
	assert_eq!(allowed(&initial), texts(&["P", "Q", "R"]));
	let undecided: Feedback = rejected(
		&mut initial,
		"P ∧ (Q ∨ R)",
		"False",
		FeedbackKind::NeedsInner,
	);
	assert_eq!(
		undecided.message,
		"这一步不能跳过内部运算。请先计算 P，再回到当前表达式。"
	);
	step(&mut initial, "P", Some("False"), "False ∧ (Q ∨ R)");
	assert_eq!(allowed(&initial), texts(&["False ∧ (Q ∨ R)", "Q", "R"]));
	let mut short: Session = initial.clone();
	step(&mut short, "False ∧ (Q ∨ R)", Some("False"), "False");
	finished_with(&short, "False", "bool");
	let mut continued: Session = initial;
	step(&mut continued, "Q", Some("True"), "False ∧ (True ∨ R)");
	step(&mut continued, "R", Some("False"), "False ∧ (True ∨ False)");
	assert_eq!(
		allowed(&continued),
		texts(&["False ∧ (True ∨ False)", "True ∨ False"])
	);
	step(
		&mut continued,
		"True ∨ False",
		Some("True"),
		"False ∧ (True)",
	);
	step(&mut continued, "(True)", None, "False ∧ True");
	step(&mut continued, "False ∧ True", Some("False"), "False");
	finished_with(&continued, "False", "bool");

	let bindings: [(&str, bool); 3] = [("P", true), ("Q", true), ("R", false)];
	let mut initial: Session = logic_with("P ∨ (Q ∧ R)", &bindings);
	step(&mut initial, "P", Some("True"), "True ∨ (Q ∧ R)");
	assert_eq!(allowed(&initial), texts(&["True ∨ (Q ∧ R)", "Q", "R"]));
	let mut short: Session = initial.clone();
	step(&mut short, "True ∨ (Q ∧ R)", Some("True"), "True");
	finished_with(&short, "True", "bool");
	let mut continued: Session = initial;
	step(&mut continued, "Q", Some("True"), "True ∨ (True ∧ R)");
	step(&mut continued, "R", Some("False"), "True ∨ (True ∧ False)");
	step(
		&mut continued,
		"True ∧ False",
		Some("False"),
		"True ∨ (False)",
	);
	step(&mut continued, "(False)", None, "True ∨ False");
	step(&mut continued, "True ∨ False", Some("True"), "True");
	finished_with(&continued, "True", "bool");

	let bindings: [(&str, bool); 3] = [("P", false), ("Q", true), ("R", false)];
	let mut initial: Session = logic_with("P → (Q ∧ R)", &bindings);
	step(&mut initial, "P", Some("False"), "False → (Q ∧ R)");
	assert_eq!(allowed(&initial), texts(&["False → (Q ∧ R)", "Q", "R"]));
	let mut short: Session = initial.clone();
	step(&mut short, "False → (Q ∧ R)", Some("True"), "True");
	finished_with(&short, "True", "bool");
	let mut continued: Session = initial;
	step(&mut continued, "Q", Some("True"), "False → (True ∧ R)");
	step(&mut continued, "R", Some("False"), "False → (True ∧ False)");
	step(
		&mut continued,
		"True ∧ False",
		Some("False"),
		"False → (False)",
	);
	step(&mut continued, "(False)", None, "False → False");
	step(&mut continued, "False → False", Some("True"), "True");
	finished_with(&continued, "True", "bool");

	// Equivalence needs both sides whatever the left one is.
	let mut iff: Session = logic_with("P ↔ (Q ∧ R)", &bindings);
	step(&mut iff, "P", Some("False"), "False ↔ (Q ∧ R)");
	assert_eq!(allowed(&iff), texts(&["Q", "R"]));
	for input in ["True", "False"] {
		let feedback: Feedback =
			rejected(&mut iff, "False ↔ (Q ∧ R)", input, FeedbackKind::NeedsInner);
		assert_eq!(
			feedback.message,
			"这一步不能跳过内部运算。请先计算 Q，再回到当前表达式。"
		);
	}
	step(&mut iff, "Q", Some("True"), "False ↔ (True ∧ R)");
	step(&mut iff, "R", Some("False"), "False ↔ (True ∧ False)");
	assert_eq!(allowed(&iff), texts(&["True ∧ False"]));
	rejected(
		&mut iff,
		"False ↔ (True ∧ False)",
		"True",
		FeedbackKind::NeedsInner,
	);
	step(&mut iff, "True ∧ False", Some("False"), "False ↔ (False)");
	step(&mut iff, "(False)", None, "False ↔ False");
	step(&mut iff, "False ↔ False", Some("True"), "True");
	finished_with(&iff, "True", "bool");
}

#[test]
fn an_undecided_and_names_its_operand_and_no_answer_there_advances() {
	let source: &str = "True and (2 + 3 > 1)";
	let mut session: Session = python(source);
	assert_eq!(allowed(&session), texts(&["2 + 3"]));
	let root: NodeId = session.root().id;
	let selection: Feedback = session.check_selection(root).unwrap_err();
	assert_eq!(selection.kind, FeedbackKind::NeedsInner);
	assert_eq!(
		selection.message,
		"这一步不能跳过内部运算。请先计算 2 + 3，再回到当前表达式。"
	);
	for input in ["True", "False"] {
		let feedback: Feedback = rejected(&mut session, source, input, FeedbackKind::NeedsInner);
		assert_eq!(feedback.message, selection.message);
	}
	assert_eq!(session.render(), source);
	assert!(session.history().is_empty());
	assert!(session.attempts().is_empty());

	step(&mut session, "2 + 3", Some("5"), "True and (5 > 1)");
	assert_eq!(allowed(&session), texts(&["5 > 1"]));
	step(&mut session, "5 > 1", Some("True"), "True and (True)");
	step(&mut session, "(True)", None, "True and True");
	step(&mut session, "True and True", Some("True"), "True");
	finished_with(&session, "True", "bool");
}

#[test]
fn a_short_circuit_returns_the_deciding_operand_with_its_own_type() {
	let initial: Session = python("0 and (2 + 3)");
	assert_eq!(allowed(&initial), texts(&["0 and (2 + 3)", "2 + 3"]));
	let mut short: Session = initial.clone();
	let feedback: Feedback = rejected(
		&mut short,
		"0 and (2 + 3)",
		"False",
		FeedbackKind::WrongType,
	);
	assert!(
		feedback
			.message
			.starts_with("这里应返回 int，你填写的是 bool。"),
		"{}",
		feedback.message
	);
	step(&mut short, "0 and (2 + 3)", Some("0"), "0");
	finished_with(&short, "0", "int");
	let mut continued: Session = initial;
	step(&mut continued, "2 + 3", Some("5"), "0 and (5)");
	step(&mut continued, "(5)", None, "0 and 5");
	rejected(&mut continued, "0 and 5", "False", FeedbackKind::WrongType);
	step(&mut continued, "0 and 5", Some("0"), "0");
	finished_with(&continued, "0", "int");

	let initial: Session = python("3 or (2 + 3)");
	assert_eq!(allowed(&initial), texts(&["3 or (2 + 3)", "2 + 3"]));
	let mut short: Session = initial.clone();
	let feedback: Feedback = rejected(&mut short, "3 or (2 + 3)", "True", FeedbackKind::WrongType);
	assert!(
		feedback
			.message
			.starts_with("这里应返回 int，你填写的是 bool。"),
		"{}",
		feedback.message
	);
	step(&mut short, "3 or (2 + 3)", Some("3"), "3");
	finished_with(&short, "3", "int");
	let mut continued: Session = initial;
	step(&mut continued, "2 + 3", Some("5"), "3 or (5)");
	step(&mut continued, "(5)", None, "3 or 5");
	rejected(&mut continued, "3 or 5", "True", FeedbackKind::WrongType);
	step(&mut continued, "3 or 5", Some("3"), "3");
	finished_with(&continued, "3", "int");
}

#[test]
fn precedence_holds_except_that_a_short_circuit_skips_the_work_in_its_operand() {
	let source: &str = "False and 2 + 3 > 1";
	let initial: Session = python(source);
	assert_eq!(allowed(&initial), texts(&[source, "2 + 3"]));
	let mut short: Session = initial.clone();
	step(&mut short, source, Some("False"), "False");
	finished_with(&short, "False", "bool");
	let mut continued: Session = initial;
	step(&mut continued, "2 + 3", Some("5"), "False and 5 > 1");
	assert_eq!(allowed(&continued), texts(&["False and 5 > 1", "5 > 1"]));

	let mut session: Session = python("1 - 2 + 3 * 4");
	assert_eq!(allowed(&session), texts(&["3 * 4"]));
	let feedback: Feedback = rejected(&mut session, "1 - 2", "-1", FeedbackKind::OutOfOrder);
	assert_eq!(
		feedback.message,
		"这里目前还不能计算，请先处理 3 * 4。同优先级的独立子式可以任选。"
	);
	step(&mut session, "3 * 4", Some("12"), "1 - 2 + 12");
	assert_eq!(allowed(&session), texts(&["1 - 2"]));

	let source: &str = "1 + 2 == 3 or False and 4 > 5";
	let mut session: Session = python(source);
	assert_eq!(allowed(&session), texts(&["1 + 2"]));
	for (text, input) in [("False and 4 > 5", "False"), ("4 > 5", "False")] {
		let feedback: Feedback = rejected(&mut session, text, input, FeedbackKind::OutOfOrder);
		assert_eq!(
			feedback.message,
			"这里目前还不能计算，请先处理 1 + 2。同优先级的独立子式可以任选。"
		);
	}
	step(
		&mut session,
		"1 + 2",
		Some("3"),
		"3 == 3 or False and 4 > 5",
	);
	assert_eq!(allowed(&session), texts(&["3 == 3", "4 > 5"]));
	step(
		&mut session,
		"3 == 3",
		Some("True"),
		"True or False and 4 > 5",
	);
	assert_eq!(
		allowed(&session),
		texts(&["True or False and 4 > 5", "False and 4 > 5", "4 > 5"])
	);
	step(
		&mut session,
		"True or False and 4 > 5",
		Some("True"),
		"True",
	);
	finished_with(&session, "True", "bool");

	// Association still decides which power goes first.
	let mut session: Session = python("2 ** 3 ** 2");
	assert_eq!(allowed(&session), texts(&["3 ** 2"]));
	let feedback: Feedback = rejected(&mut session, "2 ** 3 ** 2", "64", FeedbackKind::NeedsInner);
	assert_eq!(
		feedback.message,
		"这一步不能跳过内部运算。请先计算 3 ** 2，再回到当前表达式。"
	);
	step(&mut session, "3 ** 2", Some("9"), "2 ** 9");
	step(&mut session, "2 ** 9", Some("512"), "512");
	finished_with(&session, "512", "int");
}

#[test]
fn a_short_circuit_inside_a_continued_operand_mixes_both_paths() {
	let source: &str = "True or (False and 4 > 5)";
	let mut session: Session = python(source);
	assert_eq!(
		allowed(&session),
		texts(&[source, "False and 4 > 5", "4 > 5"])
	);
	// The hint keeps naming the language's own order: the outer short circuit.
	assert_eq!(session.next_step().unwrap().node_id, session.root().id);
	step(
		&mut session,
		"False and 4 > 5",
		Some("False"),
		"True or (False)",
	);
	assert_eq!(allowed(&session), texts(&["True or (False)", "(False)"]));
	step(&mut session, "True or (False)", Some("True"), "True");
	assert_eq!(session.history().len(), 2);
	finished_with(&session, "True", "bool");
}

#[test]
fn independent_operands_finish_the_same_in_any_order() {
	let source: &str = "(False and (4 > 5)) or ((1 + 1) == 2)";
	let initial: Session = python(source);
	assert_eq!(
		allowed(&initial),
		texts(&["False and (4 > 5)", "4 > 5", "1 + 1"])
	);
	type Order<'a> = &'a [(&'a str, Option<&'a str>, &'a str)];
	let orders: [Order; 3] = [
		// Short-circuit the left side at once, then the right side decides.
		&[
			(
				"False and (4 > 5)",
				Some("False"),
				"(False) or ((1 + 1) == 2)",
			),
			("(False)", None, "False or ((1 + 1) == 2)"),
			("1 + 1", Some("2"), "False or ((2) == 2)"),
			("(2)", None, "False or (2 == 2)"),
			("2 == 2", Some("True"), "False or (True)"),
			("(True)", None, "False or True"),
			("False or True", Some("True"), "True"),
		],
		// The right side first, then every operand on the left.
		&[
			("1 + 1", Some("2"), "(False and (4 > 5)) or ((2) == 2)"),
			("(2)", None, "(False and (4 > 5)) or (2 == 2)"),
			("2 == 2", Some("True"), "(False and (4 > 5)) or (True)"),
			("(True)", None, "(False and (4 > 5)) or True"),
			("4 > 5", Some("False"), "(False and (False)) or True"),
			("(False)", None, "(False and False) or True"),
			("False and False", Some("False"), "(False) or True"),
			("(False)", None, "False or True"),
			("False or True", Some("True"), "True"),
		],
		// Interleaved, and the short circuit taken after its operand was computed.
		&[
			(
				"4 > 5",
				Some("False"),
				"(False and (False)) or ((1 + 1) == 2)",
			),
			("1 + 1", Some("2"), "(False and (False)) or ((2) == 2)"),
			("False and (False)", Some("False"), "(False) or ((2) == 2)"),
			("(False)", None, "False or ((2) == 2)"),
			("(2)", None, "False or (2 == 2)"),
			("2 == 2", Some("True"), "False or (True)"),
			("(True)", None, "False or True"),
			("False or True", Some("True"), "True"),
		],
	];
	let mut recorded: Vec<Vec<RecordedAttempt>> = Vec::new();
	for order in orders {
		let mut session: Session = initial.clone();
		for (text, input, after) in order {
			step(&mut session, text, *input, after);
		}
		assert_eq!(session.history().len(), order.len());
		finished_with(&session, "True", "bool");
		assert!(!recorded.contains(&session.attempts().to_vec()));
		recorded.push(session.attempts().to_vec());
	}
}

#[test]
fn wrong_answers_never_advance_on_either_path() {
	let mut session: Session = python(AND);
	rejected(&mut session, AND, "True", FeedbackKind::WrongValue);
	rejected(&mut session, AND, "0", FeedbackKind::WrongType);
	rejected(&mut session, "2 + 3", "6", FeedbackKind::WrongValue);
	rejected(&mut session, "2 + 3", "5.0", FeedbackKind::WrongType);
	rejected(&mut session, "2 + 3", "five", FeedbackKind::InvalidInput);
	assert_eq!(session.render(), AND);
	assert!(session.history().is_empty());
	assert!(session.attempts().is_empty());

	step(&mut session, "2 + 3", Some("5"), "False and (5 > 1)");
	rejected(
		&mut session,
		"False and (5 > 1)",
		"True",
		FeedbackKind::WrongValue,
	);
	rejected(&mut session, "5 > 1", "False", FeedbackKind::WrongValue);
	rejected(&mut session, "5 > 1", "1", FeedbackKind::WrongType);
	assert_eq!(session.render(), "False and (5 > 1)");
	assert_eq!(session.history().len(), 1);
	assert_eq!(session.attempts().len(), 1);
	assert_eq!(allowed(&session), texts(&["False and (5 > 1)", "5 > 1"]));

	let mut session: Session =
		logic_with("P ∧ (Q ∨ R)", &[("P", false), ("Q", true), ("R", false)]);
	rejected(&mut session, "P", "True", FeedbackKind::WrongValue);
	step(&mut session, "P", Some("False"), "False ∧ (Q ∨ R)");
	rejected(
		&mut session,
		"False ∧ (Q ∨ R)",
		"True",
		FeedbackKind::WrongValue,
	);
	rejected(&mut session, "Q", "False", FeedbackKind::WrongValue);
	assert_eq!(session.history().len(), 1);
	assert_eq!(session.attempts().len(), 1);
}

#[test]
fn bindings_and_brackets_in_a_skippable_operand_keep_the_short_circuit() {
	let source: &str = "flag and (x + 1 > (x))";
	let bindings: [(&str, &str); 2] = [("flag", "False"), ("x", "2")];
	let mut session: Session = python_with(source, &bindings);
	assert_eq!(allowed(&session), texts(&["flag", "x", "x"]));
	step(
		&mut session,
		"flag",
		Some("False"),
		"False and (x + 1 > (x))",
	);
	assert_eq!(
		allowed(&session),
		texts(&["False and (x + 1 > (x))", "x", "x"])
	);

	// The occurrence inside the brackets stands for both.
	let inner: NodeId = occurrences(&session, "x")[1];
	assert_eq!(session.replacement_ids(inner).len(), 2);
	let feedback: Feedback = session.submit(inner, "2");
	assert!(feedback.accepted(), "{}", feedback.message);
	assert!(
		feedback
			.message
			.ends_with(" 已将所有 2 处同名变量一起代入。"),
		"{}",
		feedback.message
	);
	assert_eq!(session.render(), "False and (2 + 1 > (2))");
	assert_eq!(session.history().len(), 2);
	assert_eq!(session.attempts().len(), 2);
	assert_eq!(
		allowed(&session),
		texts(&["False and (2 + 1 > (2))", "2 + 1", "(2)"])
	);

	let group: NodeId = node(&session, "(2)");
	let removed: Feedback = session.remove_group(group);
	assert!(removed.accepted());
	assert_eq!(removed.message, "已去掉这一层括号。");
	assert_eq!(session.render(), "False and (2 + 1 > 2)");
	assert_eq!(
		session.attempts().last(),
		Some(&RecordedAttempt {
			node_id: group,
			input: None,
		})
	);
	assert_eq!(
		allowed(&session),
		texts(&["False and (2 + 1 > 2)", "2 + 1"])
	);
	let partial: Vec<RecordedAttempt> = session.attempts().to_vec();

	step(
		&mut session,
		"False and (2 + 1 > 2)",
		Some("False"),
		"False",
	);
	finished_with(&session, "False", "bool");

	let restored: Session = python_with(source, &bindings).replay(&partial).unwrap();
	assert_eq!(restored.render(), "False and (2 + 1 > 2)");
	assert_eq!(restored.attempts(), partial.as_slice());
	assert_eq!(
		allowed(&restored),
		texts(&["False and (2 + 1 > 2)", "2 + 1"])
	);
	let replayed: Session = python_with(source, &bindings)
		.replay(session.attempts())
		.unwrap();
	assert_eq!(replayed.root(), session.root());
	assert_eq!(replayed.attempts(), session.attempts());
	assert_eq!(replayed.history().len(), 4);
	finished_with(&replayed, "False", "bool");
}

/// The text a rejection names, between the words that frame it.
fn named<'a>(message: &'a str, before: &str, after: &str) -> &'a str {
	let start: usize = message.find(before).expect("names a step") + before.len();
	&message[start..start + message[start..].find(after).expect("closes the name")]
}

/// Every selection the rules refuse names work that exists: an out-of-order selection names an
/// allowed step that neither contains it nor lies inside it — taking an ancestor would remove
/// the selection, so it is never the advice — and a selection that needs inner work names a
/// node inside it, one the student may take now whenever the selection holds such a step.
fn rejections_name_steps_that_make_sense(session: &Session) {
	let allowed: Vec<NodeId> = session
		.allowed_steps()
		.iter()
		.map(|step| step.node_id)
		.collect();
	let within = |outer: NodeId, inner: NodeId| -> bool {
		session
			.root()
			.find(outer)
			.is_some_and(|tree| tree.find(inner).is_some())
	};
	let render = |id: NodeId| -> String { session.root().find(id).unwrap().render() };
	for (_, node) in session.root().rows() {
		if node.value().is_some() || allowed.contains(&node.id) {
			continue;
		}
		let feedback: Feedback = session.check_selection(node.id).unwrap_err();
		match feedback.kind {
			FeedbackKind::OutOfOrder => {
				let text: &str = named(&feedback.message, "请先处理 ", "。");
				assert!(
					allowed.iter().any(|id| render(*id) == text
						&& !within(*id, node.id)
						&& !within(node.id, *id)),
					"{}: {} → {}",
					session.render(),
					node.render(),
					feedback.message
				);
			}
			FeedbackKind::NeedsInner => {
				let text: &str = named(&feedback.message, "请先计算 ", "，");
				let inside: Vec<NodeId> = allowed
					.iter()
					.copied()
					.filter(|id| *id != node.id && within(node.id, *id))
					.collect();
				let candidates: Vec<NodeId> = if inside.is_empty() {
					node.rows()
						.into_iter()
						.skip(1)
						.map(|(_, inner)| inner.id)
						.collect()
				} else {
					inside
				};
				assert!(
					candidates.iter().any(|id| render(*id) == text),
					"{}: {} → {}",
					session.render(),
					node.render(),
					feedback.message
				);
			}
			kind => panic!(
				"{}: {} refused as {kind:?}",
				session.render(),
				node.render()
			),
		}
	}
}

/// Walk a question to its end, choosing each step with `choose`, and check at every state on
/// the way that the step a hint names is one the student may submit and that every refused
/// selection names work that makes sense.
fn walk(mut session: Session, choose: fn(&Session) -> NextStep) {
	for _ in 0..256 {
		let allowed: Vec<NextStep> = session.allowed_steps();
		let Some(hinted) = session.next_step() else {
			assert!(session.is_finished(), "no hint at {}", session.render());
			assert!(allowed.is_empty());
			return;
		};
		assert!(
			allowed.contains(&hinted),
			"{} hints {} outside {:?}",
			session.render(),
			shown(&session, hinted.node_id),
			allowed
				.iter()
				.map(|step| shown(&session, step.node_id))
				.collect::<Vec<String>>()
		);
		rejections_name_steps_that_make_sense(&session);
		let chosen: NextStep = choose(&session);
		let input: String = match &chosen.outcome {
			Ok(value) => value.to_string(),
			Err(error) => error
				.name()
				.unwrap_or_else(|| panic!("{} stops at a limit: {error}", session.render()))
				.into(),
		};
		let feedback: Feedback = session.submit(chosen.node_id, &input);
		assert!(feedback.accepted(), "{}", feedback.message);
	}
	panic!("{} never finished", session.render());
}

#[test]
fn the_hinted_step_is_always_allowed_and_nothing_is_once_finished() {
	let mut sessions: Vec<Session> = [
		AND,
		OR,
		"True and (2 + 3 > 1)",
		"True or (False and 4 > 5)",
		"(False and (4 > 5)) or ((1 + 1) == 2)",
		"1 + 2 == 3 or False and 4 > 5",
		"0 and (2 + 3)",
		"not (1 < 2) or 3 * (4 - 5) > 0 and 2 ** 3 ** 2 > 8",
		"False and (1 / 0 > 1)",
		"1 / 0 > 1 and False",
		"False and 1 * 2 + 3 ** 2 > 1",
		"True or 2 + 3 > 4 * 5",
		"False and 8 - 0 >= 9 / 4",
	]
	.into_iter()
	.map(python)
	.collect();
	sessions.push(logic_with(
		"(P → Q) ∧ ¬Q ∨ (P ↔ (Q ∨ ¬P))",
		&[("P", true), ("Q", false)],
	));
	for exercise in exercises::builtin().unwrap().exercises() {
		sessions.push(exercise.session().unwrap());
	}
	for language in [Language::Python, Language::Logic] {
		for seed in 0..8 {
			sessions.push(
				generate::generate(language, seed)
					.unwrap()
					.session()
					.unwrap(),
			);
		}
	}
	let reference: fn(&Session) -> NextStep = |session| session.next_step().unwrap();
	let first: fn(&Session) -> NextStep = |session| session.allowed_steps().remove(0);
	let last: fn(&Session) -> NextStep = |session| session.allowed_steps().pop().unwrap();
	let middle: fn(&Session) -> NextStep = |session| {
		let mut steps: Vec<NextStep> = session.allowed_steps();
		let index: usize = steps.len() / 2;
		steps.swap_remove(index)
	};
	for session in sessions {
		for choose in [reference, first, last, middle] {
			walk(session.clone(), choose);
		}
	}
}

/// A refused selection names what really holds it back. Inside an operand a short circuit
/// could skip, the student who keeps computing is pointed at the higher-precedence work beside
/// the selection, never at the short circuit that would remove it; a selection that needs inner
/// work is pointed at inner work it may do now.
#[test]
fn a_refused_selection_names_the_work_that_really_comes_first() {
	for (source, selected, blocker) in [
		("False and 1 * 2 + 3 ** 2 > 1", "1 * 2", "3 ** 2"),
		("False and (1 * 2 + 3 ** 2 > 1)", "1 * 2", "3 ** 2"),
		("True or 2 + 3 > 4 * 5", "2 + 3", "4 * 5"),
		("False and 8 - 0 >= 9 / 4", "8 - 0", "9 / 4"),
		("1 - 2 + 3 * 4", "1 - 2", "3 * 4"),
	] {
		let mut session: Session = python(source);
		let feedback: Feedback = rejected(&mut session, selected, "0", FeedbackKind::OutOfOrder);
		assert_eq!(
			feedback.message,
			format!("这里目前还不能计算，请先处理 {blocker}。同优先级的独立子式可以任选。"),
			"{source}"
		);
	}
	for (source, selected, inner) in [
		("True or 2 + 3 > 4 * 5", "2 + 3 > 4 * 5", "4 * 5"),
		("2 + 3 > 4 * 5", "2 + 3 > 4 * 5", "4 * 5"),
		("True and (2 + 3 > 1)", "True and (2 + 3 > 1)", "2 + 3"),
	] {
		let mut session: Session = python(source);
		let feedback: Feedback = rejected(&mut session, selected, "0", FeedbackKind::NeedsInner);
		assert_eq!(
			feedback.message,
			format!("这一步不能跳过内部运算。请先计算 {inner}，再回到当前表达式。"),
			"{source}"
		);
	}
}

/// What Python raises at `1 / 0`, in the words every answer about it repeats.
const DIVISION: &str = "ZeroDivisionError: 除数为零；这一步引发异常，不会得到一个数值。";

#[test]
fn an_exception_from_a_skippable_operand_ends_the_question_until_undone() {
	let source: &str = "False and (1 / 0 > 1)";
	let mut session: Session = python(source);
	assert_eq!(allowed(&session), texts(&[source, "1 / 0"]));
	let wrong: Feedback = rejected(&mut session, "1 / 0", "5", FeedbackKind::WrongValue);
	assert_eq!(
		wrong.message,
		format!("这一步不会产生普通值。{DIVISION} 请填写异常名称。")
	);

	let raised: Feedback = take(&mut session, "1 / 0", Some("ZeroDivisionError"));
	assert!(raised.accepted(), "{}", raised.message);
	assert!(
		raised
			.message
			.contains("这个异常来自你选择计算的子式 1 / 0")
	);
	assert!(raised.message.contains("Python 执行原式时不会在这里引发它"));
	assert_eq!(
		raised.message,
		format!(
			"正确。{DIVISION} 这个异常来自你选择计算的子式 1 / 0；Python 执行原式时不会在这里引发它。求值在这里终止。"
		)
	);
	assert_eq!(
		session.terminal_error().and_then(|error| error.name()),
		Some("ZeroDivisionError")
	);
	assert!(session.is_finished());
	assert!(session.allowed_steps().is_empty());
	assert_eq!(session.next_step(), None);
	assert_eq!(session.render(), source);
	assert_eq!(session.history().len(), 1);

	// Finished stays finished: no quiet rewrite into the short circuit's success.
	let root: NodeId = session.root().id;
	assert_eq!(
		session.check_selection(root).unwrap_err().kind,
		FeedbackKind::Finished
	);
	assert_eq!(session.submit(root, "False").kind, FeedbackKind::Finished);
	assert_eq!(session.render(), source);
	assert!(session.root().value().is_none());
	assert!(session.terminal_error().is_some());
	assert_eq!(session.history().len(), 1);
	assert_eq!(session.attempts().len(), 1);

	assert!(session.undo());
	assert_eq!(session.render(), source);
	assert!(session.terminal_error().is_none());
	assert!(session.history().is_empty());
	assert!(session.attempts().is_empty());
	assert_eq!(allowed(&session), texts(&[source, "1 / 0"]));
	step(&mut session, source, Some("False"), "False");
	finished_with(&session, "False", "bool");

	// Where Python itself raises, the answer says nothing about a chosen sub-expression.
	let mut raises: Session = python("1 / 0 > 1 and False");
	assert_eq!(allowed(&raises), texts(&["1 / 0"]));
	let feedback: Feedback = take(&mut raises, "1 / 0", Some("ZeroDivisionError"));
	assert!(feedback.accepted());
	assert_eq!(
		feedback.message,
		format!("正确。{DIVISION} 求值在这里终止。")
	);
	assert!(raises.is_finished());
	assert!(raises.terminal_error().is_some());

	// That order discards the division before it reaches a limit of this program, so Python
	// provably never raises there, whatever the limit would have said.
	let mut discarded: Session = python("False and (1 / 0 > 1) or 2 ** 5000 > 1");
	let feedback: Feedback = take(&mut discarded, "1 / 0", Some("ZeroDivisionError"));
	assert!(feedback.accepted(), "{}", feedback.message);
	assert_eq!(
		feedback.message,
		format!(
			"正确。{DIVISION} 这个异常来自你选择计算的子式 1 / 0；Python 执行原式时不会在这里引发它。求值在这里终止。"
		)
	);

	// Where a limit of this program stops that order first, nothing is claimed about Python.
	let mut limited: Session = python("2 ** 5000 > 1 or False and (1 / 0 > 1)");
	let feedback: Feedback = take(&mut limited, "1 / 0", Some("ZeroDivisionError"));
	assert!(feedback.accepted(), "{}", feedback.message);
	assert_eq!(
		feedback.message,
		format!("正确。{DIVISION} 这个异常来自你选择计算的子式 1 / 0。求值在这里终止。")
	);
}

#[test]
fn an_exception_path_survives_a_restart_and_gives_way_to_the_short_circuit() {
	let source: &str = "False and (1 / 0 > 1)";
	let question: Exercise = exercise("guarded-division", source);
	let mut practice: Practice = Practice::new(question.clone(), Progress::default()).unwrap();
	answer(&mut practice, "1 / 0", "ZeroDivisionError");
	assert!(
		taught(practice.report()).contains("Python 执行原式时不会在这里引发它"),
		"{:?}",
		practice.report()
	);
	assert!(practice.session().is_finished());
	practice.record();
	let raised: Vec<RecordedAttempt> = practice.session().attempts().to_vec();
	assert_eq!(raised.len(), 1);
	assert_eq!(raised[0].input.as_deref(), Some("ZeroDivisionError"));

	let mut restarted: Practice =
		Practice::new(question.clone(), practice.progress().clone()).unwrap();
	assert_eq!(restarted.session().render(), source);
	assert!(restarted.session().is_finished());
	assert_eq!(
		restarted.session().terminal_error(),
		practice.session().terminal_error()
	);
	assert_eq!(restarted.session().attempts(), raised.as_slice());
	assert_eq!(restarted.session().history().len(), 1);
	assert!(restarted.session().allowed_steps().is_empty());
	let root: NodeId = restarted.session().root().id;
	assert!(!restarted.select(root));
	assert!(restarted.draft().is_none());
	assert!(!restarted.report().good());

	assert!(restarted.undo());
	assert_eq!(restarted.session().render(), source);
	assert!(restarted.session().terminal_error().is_none());
	assert!(restarted.session().attempts().is_empty());
	answer(&mut restarted, source, "False");
	finished_with(restarted.session(), "False", "bool");
	restarted.record();

	let progress: Progress = restarted.progress().clone();
	assert_eq!(progress.sessions.len(), 1);
	let again: Practice = Practice::new(question, progress).unwrap();
	finished_with(again.session(), "False", "bool");
	assert_eq!(
		again.session().attempts(),
		&[RecordedAttempt {
			node_id: root,
			input: Some("False".into()),
		}]
	);
}

const LESSON: &str = r#"
version = 2
name = "choice-lesson"
title = "短路或继续"

[[questions]]
kind = "evaluation"
name = "decided-conjunction"
title = "左边已经决定"
language = "logic"
expression = "P ∧ (¬Q ∨ R)"

[questions.bindings]
P = "False"
Q = "True"
R = "False"

[[questions]]
kind = "proof"
name = "detour"
title = "中间的一道证明"
premises = ["P → Q", "P"]
conclusion = "Q"

[[questions]]
kind = "evaluation"
name = "decided-disjunction"
title = "左边已经决定"
language = "logic"
expression = "P ∨ (Q ∧ R)"

[questions.bindings]
P = "True"
Q = "True"
R = "False"
"#;

#[test]
fn a_mixed_path_survives_a_proof_in_between_and_a_restart() {
	let set: QuestionSet = QuestionSet::parse(LESSON).unwrap();
	assert_eq!(set.version(), 2);
	let questions: Vec<Question> = set.of_language(Language::Logic);
	assert_eq!(
		questions.iter().map(Question::name).collect::<Vec<&str>>(),
		["decided-conjunction", "detour", "decided-disjunction"]
	);
	let start: usize = resume_in_set(&Progress::default(), &questions).unwrap();
	assert_eq!(start, 0);
	let mut lesson: Lesson = Lesson::new(
		Course::ordered(questions.clone(), start).unwrap(),
		Progress::default(),
	)
	.unwrap();

	// Part of the operand the short circuit would skip, and no short circuit yet.
	let practice: &mut Practice = evaluating_mut(&mut lesson);
	answer(practice, "P", "False");
	answer(practice, "Q", "True");
	answer(practice, "¬True", "False");
	assert_eq!(practice.session().render(), "False ∧ (False ∨ R)");
	assert_eq!(
		allowed(practice.session()),
		texts(&["False ∧ (False ∨ R)", "R"])
	);
	let partial: Vec<RecordedAttempt> = practice.session().attempts().to_vec();
	assert_eq!(partial.len(), 3);

	assert!(lesson.next_question().unwrap());
	assert_eq!(proving(&lesson).question().name, "detour");
	assert!(proving(&lesson).proof().commands().is_empty());
	assert!(!proving(&lesson).is_finished());
	assert!(lesson.previous_question().unwrap());
	let practice: &Practice = evaluating(&lesson);
	assert_eq!(practice.question().name, "decided-conjunction");
	assert_eq!(practice.session().render(), "False ∧ (False ∨ R)");
	assert_eq!(practice.session().attempts(), partial.as_slice());
	assert_eq!(practice.session().history().len(), 3);
	assert_eq!(
		allowed(practice.session()),
		texts(&["False ∧ (False ∨ R)", "R"])
	);

	answer(evaluating_mut(&mut lesson), "False ∧ (False ∨ R)", "False");
	finished_with(evaluating(&lesson).session(), "False", "bool");
	lesson.record();
	let finished: Vec<RecordedAttempt> = evaluating(&lesson).session().attempts().to_vec();
	assert_eq!(finished.len(), 4);
	assert_eq!(&finished[..3], partial.as_slice());

	// A restart opens the first question still unfinished; the one before it kept its path.
	let progress: Progress = lesson.progress().clone();
	let index: usize = resume_in_set(&progress, &questions).unwrap();
	assert_eq!(index, 1);
	let mut restarted: Lesson =
		Lesson::new(Course::ordered(questions, index).unwrap(), progress).unwrap();
	assert_eq!(proving(&restarted).question().name, "detour");
	assert!(restarted.previous_question().unwrap());
	let practice: &Practice = evaluating(&restarted);
	finished_with(practice.session(), "False", "bool");
	assert_eq!(practice.session().attempts(), finished.as_slice());
	assert_eq!(practice.session().history().len(), 4);
}

#[test]
fn the_progress_key_names_no_strategy_and_one_record_serves_every_path() {
	let initial: Session = python(AND);
	let key: String = initial.progress_key();
	assert!(key.starts_with("flexible-substitution-v4\n"));
	assert_eq!(
		key,
		format!("flexible-substitution-v4\npython\n{{}}\n{AND}")
	);
	let mut short: Session = initial.clone();
	step(&mut short, AND, Some("False"), "False");
	let mut all: Session = initial.clone();
	step(&mut all, "2 + 3", Some("5"), "False and (5 > 1)");
	step(&mut all, "5 > 1", Some("True"), "False and (True)");
	step(&mut all, "(True)", None, "False and True");
	step(&mut all, "False and True", Some("False"), "False");
	assert_eq!(short.progress_key(), key);
	assert_eq!(all.progress_key(), key);

	let logic: Session = logic_with("P ∧ (Q ∨ R)", &[("P", false), ("Q", true), ("R", false)]);
	assert_eq!(
		logic.progress_key(),
		"flexible-substitution-v4\nlogic\n{\"P\": false, \"Q\": true, \"R\": false}\nP ∧ (Q ∨ R)"
	);

	// Whichever path the student takes, the question keeps one record, the latest.
	let question: Exercise = exercise("decided-and", AND);
	let mut practice: Practice = Practice::new(question.clone(), Progress::default()).unwrap();
	answer(&mut practice, AND, "False");
	practice.record();
	assert_eq!(
		practice
			.progress()
			.sessions
			.keys()
			.collect::<Vec<&String>>(),
		[&key]
	);
	assert!(practice.reset().unwrap());
	answer(&mut practice, "2 + 3", "5");
	answer(&mut practice, "5 > 1", "True");
	let group: NodeId = node(practice.session(), "(True)");
	assert!(practice.select(group));
	assert_eq!(practice.draft(), Some(practice.session().root().id));
	practice.paste("False");
	assert!(practice.submit());
	finished_with(practice.session(), "False", "bool");
	practice.record();
	assert_eq!(
		practice
			.progress()
			.sessions
			.keys()
			.collect::<Vec<&String>>(),
		[&key]
	);
	assert_eq!(
		practice.progress().sessions[&key],
		practice.session().attempts()
	);
	assert_eq!(practice.progress().sessions[&key].len(), 4);
	let resumed: Practice = Practice::new(question, practice.progress().clone()).unwrap();
	assert_eq!(resumed.session().attempts(), practice.session().attempts());
	finished_with(resumed.session(), "False", "bool");
}
