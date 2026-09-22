//! The application layer answers a front end without one existing: every step below runs
//! with no terminal, no window and no event library, through the public API only.

use std::collections::BTreeMap;

use stepwise::{
	app::{Course, Notice, Practice, ProofPractice, Report, Supply, Transcript},
	core::{EvaluationMode, Language, NodeId},
	exercises::{self, Exercise},
	generate,
	logic::{parse_formula, proof::Proof},
	progress::Progress,
};

fn builtin(id: &str) -> Exercise {
	exercises::builtin()
		.unwrap()
		.exercises()
		.find(|exercise| exercise.name == id)
		.expect("embedded question")
		.clone()
}

fn practice(questions: Vec<Exercise>, progress: Progress, mode: EvaluationMode) -> Practice {
	Practice::new(Course::random(questions, 0).unwrap(), progress, mode).unwrap()
}

/// The sentence the teaching rules worded. A reason carries none, and a test asking for
/// one where the app layer reported a reason is itself the failure.
fn taught(report: &Report) -> &str {
	match report {
		Report::Taught { message, .. } => message,
		other => panic!("expected a sentence from the teaching rules, got {other:?}"),
	}
}

/// Front ends point at a byte of the rendered expression, exactly as a click would.
fn at(practice: &Practice, token: &str) -> NodeId {
	let index: usize = practice
		.session()
		.render()
		.find(token)
		.expect("token is displayed");
	practice.node_owners()[index].expect("a selectable node")
}

#[test]
fn a_whole_question_is_selected_answered_undone_and_resumed_without_a_terminal() {
	let mut session: Practice = practice(
		vec![builtin("precedence")],
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	assert_eq!(session.session().render(), "2 + (3 * 4)");

	// Selecting opens a blank and reports why. The reason carries no sentence at all, so it
	// cannot leak the value the way a worded prompt could.
	let multiply: NodeId = at(&session, "3 * 4");
	assert!(!session.select(multiply));
	assert_eq!(session.draft(), Some(multiply));
	assert!(session.input().is_empty());
	assert_eq!(session.report(), &Report::Notice(Notice::DraftOpen));
	assert!(session.session().history().is_empty());
	assert_eq!(session.draft_spans().len(), 1);

	// A wrong answer keeps the draft and leaves the session exactly where it was, and what
	// the rules say about it still withholds the right answer.
	session.paste("13");
	assert!(!session.submit());
	assert!(!session.report().good());
	assert!(!taught(session.report()).contains("12"));
	assert_eq!(session.input(), "13");
	assert_eq!(session.session().render(), "2 + (3 * 4)");
	assert!(session.session().attempts().is_empty());

	session.backspace();
	assert!(session.type_character('2'));
	assert!(session.submit());
	assert!(session.report().good());
	assert_eq!(session.session().render(), "2 + (12)");
	assert!(session.draft().is_none());

	assert!(session.undo());
	assert_eq!(session.session().render(), "2 + (3 * 4)");
	assert!(session.session().attempts().is_empty());

	// Resume: record the snapshot, then rebuild the same question from it alone.
	session.select(multiply);
	session.paste("12");
	assert!(session.submit());
	session.record();
	let resumed: Practice = practice(
		vec![builtin("precedence")],
		session.progress().clone(),
		EvaluationMode::ShortCircuit,
	);
	assert_eq!(resumed.session().render(), "2 + (12)");
	assert_eq!(resumed.session().attempts().len(), 1);

	// The archived history is the same lines a window would show.
	let mut transcript: Transcript = Transcript::default();
	let lines: Vec<String> = transcript.sync(&resumed);
	assert_eq!(lines, ["Python 运算练习", "2 + (3 * 4)"]);
	assert!(transcript.sync(&resumed).is_empty());
}

#[test]
fn a_selection_the_rules_forbid_only_changes_the_feedback() {
	let mut session: Practice = practice(
		vec![builtin("independent-sums")],
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	let outer: NodeId = at(&session, "*");
	assert!(!session.select(outer));
	assert!(session.draft().is_none());
	assert!(taught(session.report()).contains("不能跳过"));
	assert!(session.session().history().is_empty());

	// A skipped branch stays unselectable while short circuit is on.
	let mut short: Practice = practice(
		vec![builtin("short-circuit")],
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	let division: NodeId = at(&short, "/");
	assert!(!short.select(division));
	assert!(short.draft().is_none());
	assert!(taught(short.report()).contains("跳过"));
	assert!(short.session().attempts().is_empty());
}

#[test]
fn a_finished_pair_of_brackets_is_removed_by_selection_and_saves_no_answer() {
	let nested: Exercise = Exercise {
		set: String::new(),
		name: "nested".into(),
		title: "nested".into(),
		expression: "((3))".into(),
		language: Language::Python,
		bindings: BTreeMap::new(),
		evaluation: None,
		note: None,
	};
	let mut session: Practice = practice(
		vec![nested.clone()],
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	// The outer pair is not finished yet, so selecting it removes nothing.
	let outer: NodeId = session.node_owners()[0].unwrap();
	assert!(!session.select(outer));
	assert!(session.session().history().is_empty());

	let inner: NodeId = session.node_owners()[1].unwrap();
	assert!(session.select(inner));
	assert_eq!(session.session().render(), "(3)");
	assert!(session.draft().is_none());
	assert!(session.input().is_empty());
	assert_eq!(session.session().attempts()[0].input, None);

	session.record();
	let resumed: Practice = practice(
		vec![nested],
		session.progress().clone(),
		EvaluationMode::ShortCircuit,
	);
	assert_eq!(resumed.session().render(), "(3)");
	assert!(
		resumed
			.session()
			.attempts()
			.iter()
			.all(|attempt| attempt.input.is_none())
	);
}

#[test]
fn one_answer_substitutes_every_occurrence_of_the_same_name() {
	let repeated: Exercise = Exercise {
		set: String::new(),
		name: "repeated".into(),
		title: "repeated".into(),
		expression: "x + y * x".into(),
		language: Language::Python,
		bindings: BTreeMap::from([("x".into(), "2".into()), ("y".into(), "3".into())]),
		evaluation: None,
		note: None,
	};
	let mut session: Practice = practice(
		vec![repeated],
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	let first: NodeId = session.node_owners()[0].unwrap();
	assert!(!session.select(first));
	// Both blanks belong to one answer, and a front end reads them straight off.
	let spans: Vec<(NodeId, std::ops::Range<usize>)> = session.draft_spans();
	assert_eq!(spans.len(), 2);
	assert_eq!(spans[0].1, 0..1);
	assert_eq!(spans[1].1, 8..9);

	session.paste("2");
	assert!(session.submit());
	assert_eq!(session.session().render(), "2 + y * 2");
	assert_eq!(session.session().attempts().len(), 1);
	assert!(session.undo());
	assert_eq!(session.session().render(), "x + y * x");
}

#[test]
fn each_evaluation_strategy_keeps_its_own_progress() {
	let mut session: Practice = practice(
		vec![builtin("precedence")],
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	let multiply: NodeId = at(&session, "3 * 4");
	session.select(multiply);
	session.paste("12");
	assert!(session.submit());

	assert!(session.toggle_mode().unwrap());
	assert_eq!(session.session().mode(), EvaluationMode::Eager);
	assert_eq!(session.session().render(), "2 + (3 * 4)");
	assert!(session.session().history().is_empty());

	assert!(session.toggle_mode().unwrap());
	assert_eq!(session.session().mode(), EvaluationMode::ShortCircuit);
	assert_eq!(session.session().render(), "2 + (12)");
	assert_eq!(session.progress().sessions.len(), 2);
}

#[test]
fn the_next_question_is_generated_and_going_back_restores_the_earlier_attempts() {
	let mut session: Practice = practice(
		vec![builtin("precedence")],
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	let multiply: NodeId = at(&session, "3 * 4");
	session.select(multiply);
	session.paste("12");
	assert!(session.submit());

	assert!(session.next_question().unwrap());
	assert!(session.question().name.starts_with("random-v1-python-"));
	assert!(!session.question().bindings.is_empty());
	assert!(session.session().history().is_empty());
	assert_eq!(session.course().index(), 1);
	let generated: String = session.session().source().into();

	assert!(session.previous_question().unwrap());
	assert_eq!(session.question().name, "precedence");
	assert_eq!(session.session().render(), "2 + (12)");
	assert_eq!(session.session().history().len(), 1);

	assert!(session.next_question().unwrap());
	assert_eq!(session.session().source(), generated);
	assert_eq!(session.course().questions().len(), 2);
}

#[test]
fn an_ordered_course_stops_at_its_last_question_without_generating_one() {
	let course: Course =
		Course::ordered(vec![builtin("precedence"), builtin("true-division")], 0).unwrap();
	assert_eq!(course.supply(), Supply::Ordered);
	let mut session: Practice =
		Practice::new(course, Progress::default(), EvaluationMode::ShortCircuit).unwrap();

	assert!(session.next_question().unwrap());
	assert_eq!(session.question().name, "true-division");

	assert!(!session.next_question().unwrap());
	assert_eq!(session.question().name, "true-division");
	assert_eq!(session.course().questions().len(), 2);
	assert_eq!(session.report(), &Report::Notice(Notice::CourseEnded));

	assert!(session.previous_question().unwrap());
	assert_eq!(session.question().name, "precedence");
}

#[test]
fn a_default_launch_resumes_unfinished_work_and_replaces_a_finished_question() {
	let embedded: Vec<Exercise> = exercises::builtin().unwrap().evaluations(Language::Python);

	// Nothing saved: draw a random question.
	let fresh: Exercise =
		stepwise::app::resume_or_generate(Language::Python, &Progress::default(), &embedded)
			.unwrap();
	assert!(fresh.name.starts_with("random-v1-python-"));

	// An unfinished embedded question comes back.
	let mut session: Practice = practice(
		vec![builtin("precedence")],
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	let multiply: NodeId = at(&session, "3 * 4");
	session.select(multiply);
	session.paste("12");
	assert!(session.submit());
	session.record();
	let unfinished: Progress = session.progress().clone();
	assert_eq!(
		stepwise::app::resume_or_generate(Language::Python, &unfinished, &embedded)
			.unwrap()
			.name,
		"precedence"
	);
	assert_eq!(
		stepwise::app::starting_mode(None, &unfinished, &builtin("precedence")),
		EvaluationMode::ShortCircuit
	);

	// A pointer into a set this launch does not have — the file was not passed, or is gone —
	// draws a new question rather than reaching into whichever set happens to be loaded. The
	// work itself stays in the snapshot, keyed by content, and comes back with that set.
	let mut absent: Progress = unfinished.clone();
	absent.current_set = "a-set-that-is-not-here".into();
	assert!(
		stepwise::app::resume_or_generate(Language::Python, &absent, &embedded)
			.unwrap()
			.name
			.starts_with("random-v1-python-")
	);
	assert_eq!(unfinished.sessions, absent.sessions);

	// Once it is finished, the next launch draws a new question instead.
	while !session.session().is_finished() {
		let step: NodeId = session.session().next_step().unwrap().node_id;
		if !session.select(step) {
			let answer: String = session
				.session()
				.next_step()
				.unwrap()
				.outcome
				.unwrap()
				.to_string();
			session.paste(&answer);
			assert!(session.submit());
		}
	}
	session.record();
	assert!(
		stepwise::app::resume_or_generate(Language::Python, session.progress(), &embedded)
			.unwrap()
			.name
			.starts_with("random-v1-python-")
	);

	// A saved random question is reconstructed from its versioned seed alone.
	let mut random: Progress = Progress::default();
	random.current = generate::generate(Language::Logic, 42).unwrap().name;
	random.mode = EvaluationMode::Eager;
	let restored: Exercise =
		stepwise::app::resume_or_generate(Language::Logic, &random, &[]).unwrap();
	assert_eq!(restored.name, random.current);
	assert_eq!(
		stepwise::app::starting_mode(None, &random, &restored),
		EvaluationMode::Eager
	);
	// An explicit choice and another language's default both override the saved strategy.
	assert_eq!(
		stepwise::app::starting_mode(Some(EvaluationMode::ShortCircuit), &random, &restored),
		EvaluationMode::ShortCircuit
	);
	assert_eq!(
		stepwise::app::starting_mode(None, &Progress::default(), &builtin("logic-and")),
		EvaluationMode::Eager
	);
}

#[test]
fn a_proof_is_submitted_undone_and_resumed_from_its_saved_commands() {
	let goal: Proof = Proof::new(
		vec![
			parse_formula("P").unwrap(),
			parse_formula("P -> Q").unwrap(),
		],
		parse_formula("Q").unwrap(),
	);
	let mut session: ProofPractice = ProofPractice::new(goal.clone(), Progress::default()).unwrap();
	assert_eq!(
		session.opening(),
		[
			"自然演绎 · 目标：Q",
			"1 P [premise ]",
			"2 (P → Q) [premise ]",
		]
	);

	// Semantic equivalence never authorises a step.
	session.paste("Q ; and-intro ; 1,2");
	assert!(!session.submit());
	assert!(!session.report().good());
	assert_eq!(session.proof().lines().len(), 2);

	session.clear_input();
	session.paste("Q ; mp ; 1,2");
	assert!(session.submit());
	assert!(session.is_finished());
	assert!(session.input().is_empty());
	assert_eq!(session.appended(2), ["3 Q [mp 1,2]"]);
	session.record();

	// Resume: the saved commands alone rebuild the finished proof.
	let resumed: ProofPractice =
		ProofPractice::new(goal.clone(), session.progress().clone()).unwrap();
	assert!(resumed.is_finished());
	assert_eq!(resumed.proof().lines().len(), 3);

	// Undo reopens the proof, and recording that removes the saved command.
	let mut resumed: ProofPractice = resumed;
	assert!(resumed.undo());
	assert!(!resumed.is_finished());
	assert_eq!(resumed.appended(3), ["↶ 撤销第 3 行及其假设作用域变更。"]);
	resumed.record();
	assert!(
		!ProofPractice::new(goal, resumed.progress().clone())
			.unwrap()
			.is_finished()
	);
	assert!(!resumed.undo());
}

#[test]
fn the_draft_and_selection_contract_holds_without_a_front_end() {
	let mut session: Practice = practice(
		vec![builtin("independent-sums")],
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	assert_eq!(session.report(), &Report::Notice(Notice::Start));

	// A hint names the next step; it never opens a draft or fills one in.
	session.hint();
	assert!(taught(session.report()).contains("下一步选择"));
	assert!(session.draft().is_none());
	assert!(session.session().attempts().is_empty());

	// Keyboard-style navigation walks the nodes still waiting for a step.
	let before: NodeId = session.selected();
	session.select_next();
	assert_ne!(session.selected(), before);
	session.select_previous();
	assert_eq!(session.selected(), before);

	// Navigating away drops an open draft rather than carrying it to another node.
	let left: NodeId = at(&session, "2 + 3");
	session.select(left);
	session.paste("5");
	assert_eq!(session.input(), "5");
	session.select_next();
	assert!(session.draft().is_none());
	assert!(session.input().is_empty());

	// The draft refuses control characters and stops at its own limit.
	session.select(at(&session, "2 + 3"));
	assert!(!session.type_character('\n'));
	assert!(!session.type_character('\t'));
	assert!(session.input().is_empty());
	session.paste(&"9".repeat(4096));
	assert_eq!(session.input().len(), 2048);
	assert!(!session.type_character('9'));
	session.clear_input();
	assert!(session.input().is_empty());
	assert!(session.draft().is_some());
	session.backspace();
	session.cancel();
	assert!(session.draft().is_none());
	assert!(session.session().history().is_empty());
}

/// Every notice the app layer raises arrives as a reason with no sentence behind it, so a
/// second front end answers the same reasons instead of copying the terminal's wording.
#[test]
fn every_notice_the_app_layer_raises_is_a_reason_and_carries_no_sentence() {
	let pair: Exercise = Exercise {
		set: String::new(),
		name: "pair".into(),
		title: "pair".into(),
		expression: "2 + 3".into(),
		language: Language::Python,
		bindings: BTreeMap::new(),
		evaluation: None,
		note: None,
	};
	// A whole binary expression over two values opens its own blank and says why.
	let mut session: Practice = practice(
		vec![pair.clone()],
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	assert_eq!(session.report(), &Report::Notice(Notice::FinalPair));
	session.paste("5");
	assert!(session.submit());
	assert!(session.report().good());

	// With no step left, a hint has nothing to name and reports that instead.
	session.hint();
	assert_eq!(session.report(), &Report::Notice(Notice::NoNextStep));

	assert!(session.undo());
	assert_eq!(session.report(), &Report::Notice(Notice::Undone));
	assert!(session.reset().unwrap());
	assert_eq!(session.report(), &Report::Notice(Notice::Restarted));

	// Switching strategy reports which one is now running, not a sentence about it.
	assert!(session.toggle_mode().unwrap());
	assert_eq!(
		session.report(),
		&Report::Notice(Notice::ModeSwitched(EvaluationMode::Eager))
	);
	assert!(session.toggle_mode().unwrap());
	assert_eq!(
		session.report(),
		&Report::Notice(Notice::ModeSwitched(EvaluationMode::ShortCircuit))
	);

	// A question a student has not finished opens on the starting reason.
	let mut course: Practice = practice(
		vec![builtin("precedence")],
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	assert_eq!(course.report(), &Report::Notice(Notice::Start));
	// A sentence the front end owns still passes through untouched.
	course.note("front-end help");
	assert_eq!(course.report(), &Report::Note("front-end help".into()));

	// Proofs report their own two reasons the same way.
	let goal: Proof = Proof::new(
		vec![
			parse_formula("P").unwrap(),
			parse_formula("P -> Q").unwrap(),
		],
		parse_formula("Q").unwrap(),
	);
	let mut proof: ProofPractice = ProofPractice::new(goal, Progress::default()).unwrap();
	assert_eq!(proof.report(), &Report::Notice(Notice::ProofStart));
	proof.paste("Q ; mp ; 1,2");
	assert!(proof.submit());
	assert!(proof.report().good());
	assert!(proof.undo());
	assert_eq!(proof.report(), &Report::Notice(Notice::ProofUndone));
	proof.note("front-end rules");
	assert_eq!(proof.report(), &Report::Note("front-end rules".into()));
}

/// Taking a step back and starting the question over both shorten the attempts, and the
/// archived record has to say which happened. That marker is the permanent record every
/// front end writes, so the app layer words it rather than a front end — these are the
/// exact lines, and this is the only place they are written.
#[test]
fn the_archived_marker_says_whether_a_step_was_taken_back_or_the_question_restarted() {
	let mut session: Practice = practice(
		vec![builtin("precedence")],
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	let mut transcript: Transcript = Transcript::default();
	assert_eq!(transcript.sync(&session), ["Python 运算练习"]);

	let multiply: NodeId = at(&session, "3 * 4");
	session.select(multiply);
	session.paste("12");
	assert!(session.submit());
	assert_eq!(transcript.sync(&session), ["2 + (3 * 4)"]);

	assert!(session.undo());
	assert_eq!(session.report(), &Report::Notice(Notice::Undone));
	assert_eq!(transcript.sync(&session), ["↶ 已撤销上一步。"]);

	// Starting over reaches the same branch and has to be told apart from an undo.
	session.select(multiply);
	session.paste("12");
	assert!(session.submit());
	assert_eq!(transcript.sync(&session), ["2 + (3 * 4)"]);
	assert!(session.reset().unwrap());
	assert_eq!(session.report(), &Report::Notice(Notice::Restarted));
	assert_eq!(transcript.sync(&session), ["↶ 已重新开始本题。"]);
}

#[test]
fn changing_question_archives_a_new_header_and_the_first_question_has_no_earlier_one() {
	let mut session: Practice = practice(
		vec![builtin("long-arithmetic")],
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	let mut transcript: Transcript = Transcript::default();
	// Nothing is archived before a step: just the language and this question's valuation.
	let opening: Vec<String> = transcript.sync(&session);
	assert_eq!(opening.len(), 2);
	assert_eq!(opening[0], "Python 运算练习");
	assert_eq!(opening[1], session.question().assignments());
	assert!(opening[1].contains("a=2"));

	// Moving to a generated question starts a fresh header block below the old state.
	let previous: String = session.session().render().into();
	assert!(session.next_question().unwrap());
	let switched: Vec<String> = transcript.sync(&session);
	assert_eq!(switched[0], previous);
	assert_eq!(switched[1], "");
	assert_eq!(switched[2], "Python 运算练习");
	assert_eq!(switched[3], session.question().assignments());
	assert!(transcript.sync(&session).is_empty());

	// Going back before the first question keeps it, and still reports a recordable change.
	assert!(session.previous_question().unwrap());
	assert_eq!(session.question().name, "long-arithmetic");
	assert_eq!(session.course().index(), 0);
	assert!(session.previous_question().unwrap());
	assert_eq!(session.question().name, "long-arithmetic");
	assert_eq!(session.course().index(), 0);
	assert_eq!(session.course().questions().len(), 2);
}

// ---------------------------------------------------------------------------
// Imported question sets: walking one in order, and whose work a saved pointer
// reopens. Every set below goes through `QuestionSet::parse`, the one load path the
// program itself uses, so nothing here is practised through a shape a file cannot say.
// ---------------------------------------------------------------------------

/// Three questions whose file order is not their alphabetical order: a course that sorted
/// them, or reordered them at all, would walk `add, div, mul` instead.
const ORDERED_SET: &str = r##"
version = 1
name = "ordered-set"
title = "顺序题集"

[[questions]]
kind = "evaluation"
name = "mul"
title = "先乘后加"
language = "python"
expression = "2 + (3 * 4)"
note = "选出下一步执行的子表达式。"

[[questions]]
kind = "evaluation"
name = "div"
title = "除法的类型"
language = "python"
expression = "6 / (1 + 2)"
note = "结果的类型也是答案的一部分。"

[[questions]]
kind = "evaluation"
name = "add"
title = "只剩一步"
language = "python"
expression = "1 + 2"
note = "整个式子只是一次加法。"
"##;

/// A set carrying both question kinds and both languages, for what a course may be asked
/// to filter out of one file.
const MIXED_SET: &str = r##"
version = 1
name = "mixed-set"
title = "混合题集"

[[questions]]
kind = "evaluation"
name = "py-one"
title = "第一道 Python 题"
language = "python"
expression = "2 + (3 * 4)"
note = "先乘后加。"

[[questions]]
kind = "evaluation"
name = "logic-one"
title = "一行真值表"
language = "logic"
expression = "(P → Q) ∧ ¬Q → ¬P"
note = "只算这一个赋值。"

[questions.bindings]
P = "True"
Q = "False"

[[questions]]
kind = "proof"
name = "chain"
title = "连续两次肯定前件"
premises = ["P -> Q", "Q -> R", "P"]
conclusion = "R"
note = "每行写「公式 ; 规则 ; 引用行」。"

[[questions]]
kind = "evaluation"
name = "py-two"
title = "第二道 Python 题"
language = "python"
expression = "6 / (1 + 2)"
note = "结果的类型也是答案的一部分。"
"##;

/// One set whose questions differ in where their opening strategy comes from: a field of
/// their own, or the default of the language they are written in.
const MODE_SET: &str = r##"
version = 1
name = "mode-set"
title = "开场策略"

[[questions]]
kind = "evaluation"
name = "eager-q"
title = "关掉短路之后"
language = "python"
expression = "False and (6 / 3)"
note = "短路开着时右边整支跳过；这题默认关掉短路。"
evaluation = "eager"

[[questions]]
kind = "evaluation"
name = "plain-q"
title = "跳过的右边"
language = "python"
expression = "False and (1 + 2)"
note = "括号里的运算一定会执行吗？"

[[questions]]
kind = "evaluation"
name = "logic-q"
title = "一行真值表"
language = "logic"
expression = "(P → Q) ∧ ¬Q → ¬P"
note = "只算这一个赋值。"

[questions.bindings]
P = "True"
Q = "False"
"##;

/// One set around one question. The set's name and the question's prose are arguments; the
/// language, the expression and the bindings — everything a progress key is built from —
/// are the same bytes whatever is passed in, unless the expression itself is changed.
fn set_around(set_id: &str, name: &str, expression: &str) -> String {
	format!(
		r##"
version = 1
name = "{set_id}"
title = "{name}"

[[questions]]
kind = "evaluation"
name = "warmup"
title = "热身"
language = "python"
expression = "1 + 2"
note = "先做一步。"

[[questions]]
kind = "evaluation"
name = "q1"
title = "{name}的第一题"
language = "python"
expression = "{expression}"
note = "{name}要问的事。"
"##
	)
}

fn imported(text: &str) -> exercises::QuestionSet {
	exercises::QuestionSet::parse(text).expect("the set loads")
}

fn ids(questions: &[Exercise]) -> Vec<&str> {
	questions
		.iter()
		.map(|exercise| exercise.name.as_str())
		.collect()
}

fn ordered(questions: &[Exercise], index: usize, progress: Progress) -> Practice {
	Practice::new(
		Course::ordered(questions.to_vec(), index).unwrap(),
		progress,
		EvaluationMode::ShortCircuit,
	)
	.unwrap()
}

/// One student step: take the step the rules name, answering it where it needs an answer.
fn step_once(session: &mut Practice) {
	let step: NodeId = session.session().next_step().expect("a step left").node_id;
	if !session.select(step) {
		let answer: String = session
			.session()
			.next_step()
			.unwrap()
			.outcome
			.unwrap()
			.to_string();
		session.paste(&answer);
		assert!(session.submit());
	}
}

fn solve(session: &mut Practice) {
	while !session.session().is_finished() {
		step_once(session);
	}
}

/// The progress a student leaves behind after working on one question of one set.
fn worked(questions: &[Exercise], index: usize, finish: bool) -> Progress {
	let mut session: Practice = ordered(questions, index, Progress::default());
	if finish {
		solve(&mut session);
	} else {
		step_once(&mut session);
		assert!(!session.session().is_finished());
	}
	session.record();
	session.progress().clone()
}

#[test]
fn an_ordered_course_over_an_imported_set_walks_the_file_and_ends_at_its_last_question() {
	let set: exercises::QuestionSet = imported(ORDERED_SET);
	let questions: Vec<Exercise> = set.evaluations(Language::Python);
	assert_eq!(ids(&questions), ["mul", "div", "add"]);

	let mut session: Practice = ordered(&questions, 0, Progress::default());
	assert_eq!(session.course().supply(), Supply::Ordered);
	// The set travels with every question it handed out; progress is saved against it.
	assert_eq!(session.question().set, "ordered-set");

	assert!(session.next_question().unwrap());
	assert_eq!(session.question().name, "div");
	assert!(session.next_question().unwrap());
	assert_eq!(session.question().name, "add");

	// The end of the file is the end of the practice: no question is drawn to fill it.
	assert!(!session.next_question().unwrap());
	assert_eq!(session.report(), &Report::Notice(Notice::CourseEnded));
	assert_eq!(session.question().name, "add");
	assert_eq!(session.course().questions().len(), 3);
	assert_eq!(ids(session.course().questions()), ["mul", "div", "add"]);

	assert!(session.previous_question().unwrap());
	assert_eq!(session.question().name, "div");
	assert!(session.previous_question().unwrap());
	assert_eq!(session.question().name, "mul");
	assert_eq!(session.course().index(), 0);
}

#[test]
fn an_ordered_set_opens_where_the_saved_pointer_left_off_inside_that_very_set() {
	let set: exercises::QuestionSet = imported(ORDERED_SET);
	let questions: Vec<Exercise> = set.evaluations(Language::Python);

	// Nothing saved: the set opens at its first question.
	assert_eq!(
		stepwise::app::resume_in_set(&Progress::default(), &questions, None).unwrap(),
		0
	);

	// An unfinished question is the one to come back to.
	let unfinished: Progress = worked(&questions, 1, false);
	assert!(unfinished.points_at("ordered-set", "div"));
	assert_eq!(
		stepwise::app::resume_in_set(&unfinished, &questions, None).unwrap(),
		1
	);

	// Once it is finished, the set moves on by one — from wherever it was, not to the end.
	let finished_first: Progress = worked(&questions, 0, true);
	assert_eq!(
		stepwise::app::resume_in_set(&finished_first, &questions, None).unwrap(),
		1
	);
	let finished_middle: Progress = worked(&questions, 1, true);
	assert_eq!(
		stepwise::app::resume_in_set(&finished_middle, &questions, None).unwrap(),
		2
	);

	// The last question has nowhere to move on to, so it stays in hand.
	let finished_last: Progress = worked(&questions, 2, true);
	assert!(finished_last.points_at("ordered-set", "add"));
	assert_eq!(
		stepwise::app::resume_in_set(&finished_last, &questions, None).unwrap(),
		2
	);

	// The same question ID under another set name is another student's business: this set
	// opens at its first question rather than following a pointer it does not own.
	let mut elsewhere: Progress = finished_middle.clone();
	elsewhere.current_set = "other-set".into();
	assert_eq!(elsewhere.current, "div");
	assert_eq!(
		stepwise::app::resume_in_set(&elsewhere, &questions, None).unwrap(),
		0
	);

	// Only one pointer is saved, so alternating between two sets loses the position in the
	// one you left. Coming back searches this set instead of restarting it: the first
	// question still unfinished, never a finished one handed back as if it were new.
	let mut alternated: Progress = worked(&questions, 0, true);
	alternated.current_set = "other-set".into();
	alternated.current = "whatever-was-practised-there".into();
	assert_eq!(
		stepwise::app::resume_in_set(&alternated, &questions, None).unwrap(),
		1
	);
}

#[test]
fn two_sets_naming_the_same_question_differently_never_reopen_each_others_work() {
	let first: exercises::QuestionSet = imported(&set_around("set-a", "甲套", "2 + (3 * 4)"));
	let other: exercises::QuestionSet = imported(&set_around("set-b", "乙套", "10 - 4"));
	let first_questions: Vec<Exercise> = first.evaluations(Language::Python);
	let other_questions: Vec<Exercise> = other.evaluations(Language::Python);
	assert_eq!(ids(&first_questions), ["warmup", "q1"]);
	assert_eq!(ids(&other_questions), ["warmup", "q1"]);

	let progress: Progress = worked(&first_questions, 1, true);
	assert!(progress.points_at("set-a", "q1"));

	// Its own set reopens the finished work — without this the negative below proves nothing.
	assert!(
		ordered(&first_questions, 1, progress.clone())
			.session()
			.is_finished()
	);

	// The other set's `q1` is a different question that happens to share an ID.
	let stranger: Practice = ordered(&other_questions, 1, progress.clone());
	assert_eq!(stranger.session().render(), "10 - 4");
	assert!(!stranger.session().is_finished());
	assert!(stranger.session().attempts().is_empty());
	assert_eq!(
		stepwise::app::resume_in_set(&progress, &other_questions, None).unwrap(),
		0
	);
}

#[test]
fn a_question_copied_byte_for_byte_into_another_set_shares_the_work_but_not_the_pointer() {
	let first: exercises::QuestionSet = imported(&set_around("set-a", "甲套", "2 + (3 * 4)"));
	let copy: exercises::QuestionSet = imported(&set_around("set-c", "丙套", "2 + (3 * 4)"));
	let first_questions: Vec<Exercise> = first.evaluations(Language::Python);
	let copy_questions: Vec<Exercise> = copy.evaluations(Language::Python);
	// Everything but the expression differs: the set's name, its title and the question's
	// own prose.
	assert_ne!(first_questions[1].title, copy_questions[1].title);
	assert_ne!(first_questions[1].set, copy_questions[1].set);

	let progress: Progress = worked(&first_questions, 1, true);

	// Attempts are keyed by content — the rules version, the strategy, the language, the
	// bindings and the source — so the same work done on the same question is the same
	// work, whichever file it was distributed in. This is the key doing its job, not a
	// leak: what is set-scoped is the POINTER, and that is asserted right below.
	let shared: Practice = ordered(&copy_questions, 1, progress.clone());
	assert!(shared.session().is_finished());
	assert_eq!(
		shared.session().attempts(),
		ordered(&first_questions, 1, progress.clone())
			.session()
			.attempts()
	);
	assert_eq!(
		stepwise::app::resume_in_set(&progress, &copy_questions, None).unwrap(),
		0
	);
}

#[test]
fn editing_a_question_opens_it_fresh_and_leaves_the_earlier_attempts_in_the_file() {
	let before: exercises::QuestionSet = imported(&set_around("set-a", "甲套", "2 + (3 * 4)"));
	let before_questions: Vec<Exercise> = before.evaluations(Language::Python);
	let progress: Progress = worked(&before_questions, 1, true);
	let old_key: String = before_questions[1]
		.session(EvaluationMode::ShortCircuit)
		.unwrap()
		.progress_key();
	assert!(progress.sessions.contains_key(&old_key));
	let saved: usize = progress.sessions.len();

	// The teacher edits the expression and keeps the set and the question ID.
	let after: exercises::QuestionSet = imported(&set_around("set-a", "甲套", "2 + (3 * 5)"));
	let after_questions: Vec<Exercise> = after.evaluations(Language::Python);
	let edited: Practice = ordered(&after_questions, 1, progress.clone());
	assert_eq!(edited.session().render(), "2 + (3 * 5)");
	assert!(!edited.session().is_finished());
	assert!(edited.session().attempts().is_empty());

	// Nothing was rewritten or dropped: the old key is still there with its attempts, so a
	// teacher who puts the expression back finds the work again.
	assert_eq!(edited.progress().sessions.len(), saved);
	assert!(edited.progress().sessions.contains_key(&old_key));

	// The pointer still names this question of this set, so that is where the set opens.
	assert_eq!(
		stepwise::app::resume_in_set(&progress, &after_questions, None).unwrap(),
		1
	);
}

#[test]
fn the_set_a_question_came_from_travels_into_the_progress_and_a_random_one_carries_none() {
	let set: exercises::QuestionSet = imported(ORDERED_SET);
	let questions: Vec<Exercise> = set.evaluations(Language::Python);
	let mut session: Practice = ordered(&questions, 1, Progress::default());
	session.record();
	assert_eq!(session.progress().current_set, "ordered-set");
	assert_eq!(session.progress().current, "div");

	// A generated question belongs to no set, and an empty name is what says so.
	let random: Exercise = generate::generate(Language::Python, 7).unwrap();
	let mut drawn: Practice = practice(
		vec![random.clone()],
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	drawn.record();
	assert!(drawn.progress().current_set.is_empty());
	assert_eq!(drawn.progress().current, random.name);
}

#[test]
fn a_question_opens_in_the_strategy_its_own_file_asked_for_unless_something_beats_it() {
	let set: exercises::QuestionSet = imported(MODE_SET);
	let questions: Vec<Exercise> = set.evaluations(Language::Python);
	let eager: &Exercise = &questions[0];
	let plain: &Exercise = &questions[1];
	let logic_questions: Vec<Exercise> = set.evaluations(Language::Logic);
	let logic: &Exercise = &logic_questions[0];

	// Python is taught with short circuit on; this question says otherwise for itself, and
	// its neighbour in the same file shows the difference is the field, not the language.
	assert_eq!(
		stepwise::app::starting_mode(None, &Progress::default(), eager),
		EvaluationMode::Eager
	);
	assert_eq!(
		stepwise::app::starting_mode(None, &Progress::default(), plain),
		EvaluationMode::ShortCircuit
	);

	// What the student asked for on the command line still wins.
	assert_eq!(
		stepwise::app::starting_mode(
			Some(EvaluationMode::ShortCircuit),
			&Progress::default(),
			eager
		),
		EvaluationMode::ShortCircuit
	);

	// A saved strategy is this question's only when the pointer names this set and this ID.
	let mut saved: Progress = Progress::default();
	saved.current_set = "mode-set".into();
	saved.current = "plain-q".into();
	saved.mode = EvaluationMode::Eager;
	assert_eq!(
		stepwise::app::starting_mode(None, &saved, plain),
		EvaluationMode::Eager
	);
	let mut elsewhere: Progress = saved.clone();
	elsewhere.current_set = "other-set".into();
	assert_eq!(
		stepwise::app::starting_mode(None, &elsewhere, plain),
		EvaluationMode::ShortCircuit
	);

	// Nor does a saved strategy reach the question beside it in its own set: the logic
	// question keeps what the pointer left on it, and the Python question next to it opens
	// in its own strategy rather than in logic's.
	let mut on_logic: Progress = Progress::default();
	on_logic.current_set = "mode-set".into();
	on_logic.current = "logic-q".into();
	on_logic.mode = EvaluationMode::ShortCircuit;
	assert_eq!(logic.mode(), EvaluationMode::Eager);
	assert_eq!(
		stepwise::app::starting_mode(None, &on_logic, logic),
		EvaluationMode::ShortCircuit
	);
	assert_eq!(
		stepwise::app::starting_mode(None, &on_logic, eager),
		EvaluationMode::Eager
	);
}

#[test]
fn one_file_may_mix_languages_and_question_kinds_and_a_course_gets_only_what_it_practises() {
	let set: exercises::QuestionSet = imported(MIXED_SET);
	assert_eq!(set.questions().len(), 4);
	assert_eq!(set.exercises().count(), 3);

	// One language's evaluation questions, in file order, and never a proof.
	assert_eq!(
		ids(&set.evaluations(Language::Python)),
		["py-one", "py-two"]
	);
	assert_eq!(ids(&set.evaluations(Language::Logic)), ["logic-one"]);

	// Natural deduction is propositional, so the proof is a logic question — it is simply
	// not an evaluation one, and no ordered course of either language is handed it.
	let proof: &exercises::Question = set.find("chain").expect("the proof question");
	assert_eq!(proof.language(), Language::Logic);
	assert!(proof.evaluation().is_none());

	let mut session: Practice = ordered(&set.evaluations(Language::Python), 0, Progress::default());
	assert!(session.next_question().unwrap());
	assert_eq!(session.question().name, "py-two");
	assert!(!session.next_question().unwrap());
	assert_eq!(session.report(), &Report::Notice(Notice::CourseEnded));
}
