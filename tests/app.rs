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
		.into_iter()
		.find(|exercise| exercise.id == id)
		.expect("embedded question")
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
		id: "nested".into(),
		title: "nested".into(),
		expression: "((3))".into(),
		goal: String::new(),
		language: Language::Python,
		bindings: BTreeMap::new(),
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
		id: "repeated".into(),
		title: "repeated".into(),
		expression: "x + y * x".into(),
		goal: String::new(),
		language: Language::Python,
		bindings: BTreeMap::from([("x".into(), "2".into()), ("y".into(), "3".into())]),
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
	assert!(session.question().id.starts_with("random-v1-python-"));
	assert!(!session.question().bindings.is_empty());
	assert!(session.session().history().is_empty());
	assert_eq!(session.course().index(), 1);
	let generated: String = session.session().source().into();

	assert!(session.previous_question().unwrap());
	assert_eq!(session.question().id, "precedence");
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
	assert_eq!(session.question().id, "true-division");

	assert!(!session.next_question().unwrap());
	assert_eq!(session.question().id, "true-division");
	assert_eq!(session.course().questions().len(), 2);
	assert_eq!(session.report(), &Report::Notice(Notice::CourseEnded));

	assert!(session.previous_question().unwrap());
	assert_eq!(session.question().id, "precedence");
}

#[test]
fn a_default_launch_resumes_unfinished_work_and_replaces_a_finished_question() {
	let embedded: Vec<Exercise> = exercises::builtin()
		.unwrap()
		.into_iter()
		.filter(|exercise| exercise.language == Language::Python)
		.collect();

	// Nothing saved: draw a random question.
	let fresh: Exercise =
		stepwise::app::resume_or_generate(Language::Python, &Progress::default(), &embedded)
			.unwrap();
	assert!(fresh.id.starts_with("random-v1-python-"));

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
			.id,
		"precedence"
	);
	assert_eq!(
		stepwise::app::starting_mode(None, &unfinished, &builtin("precedence")),
		EvaluationMode::ShortCircuit
	);

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
			.id
			.starts_with("random-v1-python-")
	);

	// A saved random question is reconstructed from its versioned seed alone.
	let mut random: Progress = Progress::default();
	random.current = generate::generate(Language::Logic, 42).unwrap().id;
	random.mode = EvaluationMode::Eager;
	let restored: Exercise =
		stepwise::app::resume_or_generate(Language::Logic, &random, &[]).unwrap();
	assert_eq!(restored.id, random.current);
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
		id: "pair".into(),
		title: "pair".into(),
		expression: "2 + 3".into(),
		goal: String::new(),
		language: Language::Python,
		bindings: BTreeMap::new(),
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
	assert_eq!(session.question().id, "long-arithmetic");
	assert_eq!(session.course().index(), 0);
	assert!(session.previous_question().unwrap());
	assert_eq!(session.question().id, "long-arithmetic");
	assert_eq!(session.course().index(), 0);
	assert_eq!(session.course().questions().len(), 2);
}
