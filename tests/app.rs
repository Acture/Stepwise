//! The application layer answers a front end without one existing: every step below runs
//! with no terminal, no window and no event library, through the public API only.

use std::collections::BTreeMap;

use stepwise::{
	app::{Course, Lesson, Notice, Practice, ProofPractice, Report, Supply, Task, Transcript},
	core::{EvaluationMode, Language, NodeId},
	exercises::{self, Exercise, ProofQuestion, Question, QuestionSet},
	generate,
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

/// A proof question of `set`, with the set it came from stamped on it.
fn proof_question(set: &QuestionSet, name: &str) -> ProofQuestion {
	match set.find(name) {
		Some(Question::Proof(question)) => question.clone(),
		Some(Question::Evaluation(exercise)) => {
			panic!("{name} is an evaluation question: {}", exercise.expression)
		}
		None => panic!("{name} is not a question of {}", set.name),
	}
}

/// An evaluation question of `set`, with the set it came from stamped on it.
fn evaluation_question(set: &QuestionSet, name: &str) -> Exercise {
	match set.find(name) {
		Some(Question::Evaluation(exercise)) => exercise.clone(),
		Some(Question::Proof(question)) => {
			panic!("{name} is a proof question: {}", question.sequent())
		}
		None => panic!("{name} is not a question of {}", set.name),
	}
}

fn builtin_proof(name: &str) -> ProofQuestion {
	proof_question(&exercises::builtin().unwrap(), name)
}

/// A proof written out on the command line: premises and a conclusion, and no set.
fn written(premises: &[&str], conclusion: &str) -> ProofQuestion {
	ProofQuestion {
		set: String::new(),
		name: "custom-proof".into(),
		title: "自定义证明".into(),
		premises: premises.iter().map(|premise| (*premise).into()).collect(),
		conclusion: conclusion.into(),
		note: None,
	}
}

/// One question on its own. Which question comes next is a lesson's business, so a test
/// that never moves needs no course.
fn practice(question: Exercise, progress: Progress, mode: EvaluationMode) -> Practice {
	Practice::new(question, progress, mode).unwrap()
}

/// Random practice opening on `first` and drawing a new question after it, as a launch with
/// no set does.
fn random_lesson(first: Exercise, progress: Progress, mode: EvaluationMode) -> Lesson {
	Lesson::new(
		Course::random(vec![Question::Evaluation(first)], 0).unwrap(),
		progress,
		mode,
	)
	.unwrap()
}

/// A fixed set walked in file order from `index`, as a launch with `--set` does.
fn lesson(
	questions: Vec<Question>,
	index: usize,
	progress: Progress,
	mode: EvaluationMode,
) -> Lesson {
	Lesson::new(Course::ordered(questions, index).unwrap(), progress, mode).unwrap()
}

/// The evaluation question in hand. A proof there is the failure, and the message says which.
fn evaluating(lesson: &Lesson) -> &Practice {
	match lesson.task() {
		Task::Evaluation(practice) => practice,
		Task::Proof(practice) => panic!(
			"expected an evaluation question, got the proof {}",
			practice.question().name
		),
	}
}

fn evaluating_mut(lesson: &mut Lesson) -> &mut Practice {
	match lesson.task_mut() {
		Task::Evaluation(practice) => practice,
		Task::Proof(practice) => panic!(
			"expected an evaluation question, got the proof {}",
			practice.question().name
		),
	}
}

/// The proof in hand. An evaluation question there is the failure, and the message says which.
fn proving(lesson: &Lesson) -> &ProofPractice {
	match lesson.task() {
		Task::Proof(practice) => practice,
		Task::Evaluation(practice) => panic!(
			"expected a proof, got the evaluation question {}",
			practice.question().name
		),
	}
}

fn proving_mut(lesson: &mut Lesson) -> &mut ProofPractice {
	match lesson.task_mut() {
		Task::Proof(practice) => practice,
		Task::Evaluation(practice) => panic!(
			"expected a proof, got the evaluation question {}",
			practice.question().name
		),
	}
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
		builtin("precedence"),
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
	let resumed: Lesson = random_lesson(
		builtin("precedence"),
		session.progress().clone(),
		EvaluationMode::ShortCircuit,
	);
	assert_eq!(evaluating(&resumed).session().render(), "2 + (12)");
	assert_eq!(evaluating(&resumed).session().attempts().len(), 1);

	// The archived history is the same lines a window would show.
	let mut transcript: Transcript = Transcript::default();
	let lines: Vec<String> = transcript.sync(&resumed);
	assert_eq!(lines, ["Python 运算练习", "2 + (3 * 4)"]);
	assert!(transcript.sync(&resumed).is_empty());
}

#[test]
fn a_selection_the_rules_forbid_only_changes_the_feedback() {
	let mut session: Practice = practice(
		builtin("independent-sums"),
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
		builtin("short-circuit"),
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
		nested.clone(),
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
		nested,
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
	let mut session: Practice =
		practice(repeated, Progress::default(), EvaluationMode::ShortCircuit);
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
		builtin("precedence"),
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
	let mut walk: Lesson = random_lesson(
		builtin("precedence"),
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	let multiply: NodeId = at(evaluating(&walk), "3 * 4");
	let session: &mut Practice = evaluating_mut(&mut walk);
	session.select(multiply);
	session.paste("12");
	assert!(session.submit());

	assert!(walk.next_question().unwrap());
	assert!(
		evaluating(&walk)
			.question()
			.name
			.starts_with("random-v1-python-")
	);
	assert!(!evaluating(&walk).question().bindings.is_empty());
	assert!(evaluating(&walk).session().history().is_empty());
	assert_eq!(walk.course().index(), 1);
	let generated: String = evaluating(&walk).session().source().into();

	assert!(walk.previous_question().unwrap());
	assert_eq!(evaluating(&walk).question().name, "precedence");
	assert_eq!(evaluating(&walk).session().render(), "2 + (12)");
	assert_eq!(evaluating(&walk).session().history().len(), 1);

	assert!(walk.next_question().unwrap());
	assert_eq!(evaluating(&walk).session().source(), generated);
	assert_eq!(walk.course().questions().len(), 2);
}

#[test]
fn an_ordered_course_stops_at_its_last_question_without_generating_one() {
	let course: Course = Course::ordered(
		vec![
			Question::Evaluation(builtin("precedence")),
			Question::Evaluation(builtin("true-division")),
		],
		0,
	)
	.unwrap();
	assert_eq!(course.supply(), Supply::Ordered);
	let mut walk: Lesson =
		Lesson::new(course, Progress::default(), EvaluationMode::ShortCircuit).unwrap();

	assert!(walk.next_question().unwrap());
	assert_eq!(walk.question().name(), "true-division");

	assert!(!walk.next_question().unwrap());
	assert_eq!(walk.question().name(), "true-division");
	assert_eq!(walk.course().questions().len(), 2);
	assert_eq!(walk.report(), &Report::Notice(Notice::CourseEnded));

	assert!(walk.previous_question().unwrap());
	assert_eq!(walk.question().name(), "precedence");
}

#[test]
fn a_default_launch_resumes_unfinished_work_and_replaces_a_finished_question() {
	let embedded: Vec<Question> = exercises::builtin().unwrap().of_language(Language::Python);

	// Nothing saved: draw a random question.
	let fresh: Question =
		stepwise::app::resume_or_generate(Language::Python, &Progress::default(), &embedded)
			.unwrap();
	assert!(fresh.name().starts_with("random-v1-python-"));

	// An unfinished embedded question comes back.
	let mut session: Practice = practice(
		builtin("precedence"),
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
			.name(),
		"precedence"
	);
	assert_eq!(
		stepwise::app::starting_mode(
			None,
			&unfinished,
			&Question::Evaluation(builtin("precedence"))
		),
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
			.name()
			.starts_with("random-v1-python-")
	);
	assert_eq!(unfinished.sessions, absent.sessions);

	// Once it is finished, the next launch draws a new question instead.
	solve(&mut session);
	session.record();
	assert!(
		stepwise::app::resume_or_generate(Language::Python, session.progress(), &embedded)
			.unwrap()
			.name()
			.starts_with("random-v1-python-")
	);

	// A saved random question is reconstructed from its versioned seed alone.
	let mut random: Progress = Progress::default();
	random.current = generate::generate(Language::Logic, 42).unwrap().name;
	random.mode = EvaluationMode::Eager;
	let restored: Question =
		stepwise::app::resume_or_generate(Language::Logic, &random, &[]).unwrap();
	assert_eq!(restored.name(), random.current);
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
		stepwise::app::starting_mode(
			None,
			&Progress::default(),
			&Question::Evaluation(builtin("logic-and"))
		),
		EvaluationMode::Eager
	);
}

#[test]
fn a_proof_is_submitted_undone_and_resumed_from_its_saved_commands() {
	let goal: ProofQuestion = written(&["P", "P -> Q"], "Q");
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
		builtin("independent-sums"),
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
		pair.clone(),
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
		builtin("precedence"),
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	assert_eq!(course.report(), &Report::Notice(Notice::Start));
	// A sentence the front end owns still passes through untouched.
	course.note("front-end help");
	assert_eq!(course.report(), &Report::Note("front-end help".into()));

	// Proofs report their own two reasons the same way.
	let mut proof: ProofPractice =
		ProofPractice::new(written(&["P", "P -> Q"], "Q"), Progress::default()).unwrap();
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
	let mut walk: Lesson = random_lesson(
		builtin("precedence"),
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	let mut transcript: Transcript = Transcript::default();
	assert_eq!(transcript.sync(&walk), ["Python 运算练习"]);

	let multiply: NodeId = at(evaluating(&walk), "3 * 4");
	let session: &mut Practice = evaluating_mut(&mut walk);
	session.select(multiply);
	session.paste("12");
	assert!(session.submit());
	assert_eq!(transcript.sync(&walk), ["2 + (3 * 4)"]);

	assert!(evaluating_mut(&mut walk).undo());
	assert_eq!(walk.report(), &Report::Notice(Notice::Undone));
	assert_eq!(transcript.sync(&walk), ["↶ 已撤销上一步。"]);

	// Starting over reaches the same branch and has to be told apart from an undo.
	let session: &mut Practice = evaluating_mut(&mut walk);
	session.select(multiply);
	session.paste("12");
	assert!(session.submit());
	assert_eq!(transcript.sync(&walk), ["2 + (3 * 4)"]);
	assert!(evaluating_mut(&mut walk).reset().unwrap());
	assert_eq!(walk.report(), &Report::Notice(Notice::Restarted));
	assert_eq!(transcript.sync(&walk), ["↶ 已重新开始本题。"]);
}

#[test]
fn changing_question_archives_a_new_header_and_the_first_question_has_no_earlier_one() {
	let mut walk: Lesson = random_lesson(
		builtin("long-arithmetic"),
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	let mut transcript: Transcript = Transcript::default();
	// Nothing is archived before a step: just the language and this question's valuation.
	let opening: Vec<String> = transcript.sync(&walk);
	assert_eq!(opening.len(), 2);
	assert_eq!(opening[0], "Python 运算练习");
	assert_eq!(opening[1], evaluating(&walk).question().assignments());
	assert!(opening[1].contains("a=2"));

	// Moving to a generated question starts a fresh header block below the old state.
	let previous: String = evaluating(&walk).session().render().into();
	assert!(walk.next_question().unwrap());
	let switched: Vec<String> = transcript.sync(&walk);
	assert_eq!(switched[0], previous);
	assert_eq!(switched[1], "");
	assert_eq!(switched[2], "Python 运算练习");
	assert_eq!(switched[3], evaluating(&walk).question().assignments());
	assert!(transcript.sync(&walk).is_empty());

	// Going back before the first question keeps it, and still reports a recordable change.
	assert!(walk.previous_question().unwrap());
	assert_eq!(walk.question().name(), "long-arithmetic");
	assert_eq!(walk.course().index(), 0);
	assert!(walk.previous_question().unwrap());
	assert_eq!(walk.question().name(), "long-arithmetic");
	assert_eq!(walk.course().index(), 0);
	assert_eq!(walk.course().questions().len(), 2);
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

/// A logic course that runs evaluation → proof → evaluation, with a Python question between
/// the first two in the file. The logic course steps over that one and walks the rest in file
/// order whichever kind each is, so every move below crosses from one kind to the other.
const WALK_SET: &str = r##"
version = 1
name = "walk-set"
title = "求值与证明交替"

[[questions]]
kind = "evaluation"
name = "before"
title = "证明之前"
language = "logic"
expression = "P ∧ Q"

[questions.bindings]
P = "True"
Q = "False"

[[questions]]
kind = "evaluation"
name = "aside"
title = "另一门语言"
language = "python"
expression = "1 + 2"

[[questions]]
kind = "proof"
name = "chain"
title = "连续两次肯定前件"
premises = ["P -> Q", "Q -> R", "P"]
conclusion = "R"

[[questions]]
kind = "evaluation"
name = "after"
title = "证明之后"
language = "logic"
expression = "P ∨ Q"

[questions.bindings]
P = "False"
Q = "True"
"##;

/// The two lines that finish `chain`: modus ponens twice, each citing the implication first.
const CHAIN: [&str; 2] = ["Q ; mp ; 1,3", "R ; mp ; 2,4"];

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

fn imported(text: &str) -> QuestionSet {
	QuestionSet::parse(text).expect("the set loads")
}

/// One language's questions of a set that holds only evaluation questions in that language.
fn evaluations(set: &QuestionSet, language: Language) -> Vec<Exercise> {
	set.of_language(language)
		.iter()
		.map(|question| {
			question
				.evaluation()
				.unwrap_or_else(|| panic!("{} is a proof question", question.name()))
				.clone()
		})
		.collect()
}

fn ids(questions: &[Question]) -> Vec<&str> {
	questions.iter().map(Question::name).collect()
}

/// The question at `index` of a set, opened on its own in the strategy these tests share.
fn ordered(questions: &[Exercise], index: usize, progress: Progress) -> Practice {
	practice(
		questions[index].clone(),
		progress,
		EvaluationMode::ShortCircuit,
	)
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

/// The progress after one step of `exercise` in `mode`, or every step when `finish`, added to
/// what `progress` already holds.
fn stepped(
	exercise: &Exercise,
	mode: EvaluationMode,
	finish: bool,
	progress: Progress,
) -> Progress {
	let mut session: Practice = practice(exercise.clone(), progress, mode);
	if finish {
		solve(&mut session);
	} else {
		step_once(&mut session);
		assert!(!session.session().is_finished());
	}
	session.record();
	session.progress().clone()
}

/// The progress a student leaves behind after working on one question of one set.
fn worked(questions: &[Exercise], index: usize, finish: bool) -> Progress {
	stepped(
		&questions[index],
		EvaluationMode::ShortCircuit,
		finish,
		Progress::default(),
	)
}

/// The progress after writing these proof lines, each one accepted, added to what `progress`
/// already holds. No lines at all is a proof opened and left.
fn proved(question: &ProofQuestion, lines: &[&str], progress: Progress) -> Progress {
	let mut practice: ProofPractice = ProofPractice::new(question.clone(), progress).unwrap();
	for line in lines {
		practice.paste(line);
		assert!(practice.submit(), "{line}: {:?}", practice.report());
	}
	practice.record();
	practice.progress().clone()
}

#[test]
fn an_ordered_course_over_an_imported_set_walks_the_file_and_ends_at_its_last_question() {
	let set: QuestionSet = imported(ORDERED_SET);
	let questions: Vec<Question> = set.of_language(Language::Python);
	assert_eq!(ids(&questions), ["mul", "div", "add"]);

	let mut walk: Lesson = lesson(
		questions,
		0,
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	assert_eq!(walk.course().supply(), Supply::Ordered);
	// The set travels with every question it handed out; progress is saved against it.
	assert_eq!(evaluating(&walk).question().set, "ordered-set");

	assert!(walk.next_question().unwrap());
	assert_eq!(walk.question().name(), "div");
	assert!(walk.next_question().unwrap());
	assert_eq!(walk.question().name(), "add");

	// The end of the file is the end of the practice: no question is drawn to fill it.
	assert!(!walk.next_question().unwrap());
	assert_eq!(walk.report(), &Report::Notice(Notice::CourseEnded));
	assert_eq!(walk.question().name(), "add");
	assert_eq!(walk.course().questions().len(), 3);
	assert_eq!(ids(walk.course().questions()), ["mul", "div", "add"]);

	assert!(walk.previous_question().unwrap());
	assert_eq!(walk.question().name(), "div");
	assert!(walk.previous_question().unwrap());
	assert_eq!(walk.question().name(), "mul");
	assert_eq!(walk.course().index(), 0);
}

#[test]
fn an_ordered_set_opens_where_the_saved_pointer_left_off_inside_that_very_set() {
	let set: QuestionSet = imported(ORDERED_SET);
	let exercises: Vec<Exercise> = evaluations(&set, Language::Python);
	let questions: Vec<Question> = set.of_language(Language::Python);

	// Nothing saved: the set opens at its first question.
	assert_eq!(
		stepwise::app::resume_in_set(&Progress::default(), &questions, None).unwrap(),
		0
	);

	// An unfinished question is the one to come back to.
	let unfinished: Progress = worked(&exercises, 1, false);
	assert!(unfinished.points_at("ordered-set", "div"));
	assert_eq!(
		stepwise::app::resume_in_set(&unfinished, &questions, None).unwrap(),
		1
	);

	// Once it is finished, the set moves on by one — from wherever it was, not to the end.
	let finished_first: Progress = worked(&exercises, 0, true);
	assert_eq!(
		stepwise::app::resume_in_set(&finished_first, &questions, None).unwrap(),
		1
	);
	let finished_middle: Progress = worked(&exercises, 1, true);
	assert_eq!(
		stepwise::app::resume_in_set(&finished_middle, &questions, None).unwrap(),
		2
	);

	// The last question has nowhere to move on to, so it stays in hand.
	let finished_last: Progress = worked(&exercises, 2, true);
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
	let mut alternated: Progress = worked(&exercises, 0, true);
	alternated.current_set = "other-set".into();
	alternated.current = "whatever-was-practised-there".into();
	assert_eq!(
		stepwise::app::resume_in_set(&alternated, &questions, None).unwrap(),
		1
	);
}

#[test]
fn two_sets_naming_the_same_question_differently_never_reopen_each_others_work() {
	let first: QuestionSet = imported(&set_around("set-a", "甲套", "2 + (3 * 4)"));
	let other: QuestionSet = imported(&set_around("set-b", "乙套", "10 - 4"));
	assert_eq!(ids(&first.of_language(Language::Python)), ["warmup", "q1"]);
	assert_eq!(ids(&other.of_language(Language::Python)), ["warmup", "q1"]);
	let first_questions: Vec<Exercise> = evaluations(&first, Language::Python);
	let other_questions: Vec<Exercise> = evaluations(&other, Language::Python);

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
		stepwise::app::resume_in_set(&progress, &other.of_language(Language::Python), None)
			.unwrap(),
		0
	);
}

#[test]
fn a_question_copied_byte_for_byte_into_another_set_shares_the_work_but_not_the_pointer() {
	let first: QuestionSet = imported(&set_around("set-a", "甲套", "2 + (3 * 4)"));
	let copy: QuestionSet = imported(&set_around("set-c", "丙套", "2 + (3 * 4)"));
	let first_questions: Vec<Exercise> = evaluations(&first, Language::Python);
	let copy_questions: Vec<Exercise> = evaluations(&copy, Language::Python);
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
		stepwise::app::resume_in_set(&progress, &copy.of_language(Language::Python), None).unwrap(),
		0
	);
}

#[test]
fn editing_a_question_opens_it_fresh_and_leaves_the_earlier_attempts_in_the_file() {
	let before: QuestionSet = imported(&set_around("set-a", "甲套", "2 + (3 * 4)"));
	let before_questions: Vec<Exercise> = evaluations(&before, Language::Python);
	let progress: Progress = worked(&before_questions, 1, true);
	let old_key: String = before_questions[1]
		.session(EvaluationMode::ShortCircuit)
		.unwrap()
		.progress_key();
	assert!(progress.sessions.contains_key(&old_key));
	let saved: usize = progress.sessions.len();

	// The teacher edits the expression and keeps the set and the question ID.
	let after: QuestionSet = imported(&set_around("set-a", "甲套", "2 + (3 * 5)"));
	let after_questions: Vec<Exercise> = evaluations(&after, Language::Python);
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
		stepwise::app::resume_in_set(&progress, &after.of_language(Language::Python), None)
			.unwrap(),
		1
	);
}

#[test]
fn the_set_a_question_came_from_travels_into_the_progress_and_a_random_one_carries_none() {
	let set: QuestionSet = imported(ORDERED_SET);
	let questions: Vec<Exercise> = evaluations(&set, Language::Python);
	let mut session: Practice = ordered(&questions, 1, Progress::default());
	session.record();
	assert_eq!(session.progress().current_set, "ordered-set");
	assert_eq!(session.progress().current, "div");

	// A generated question belongs to no set, and an empty name is what says so.
	let random: Exercise = generate::generate(Language::Python, 7).unwrap();
	let mut drawn: Practice = practice(
		random.clone(),
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	drawn.record();
	assert!(drawn.progress().current_set.is_empty());
	assert_eq!(drawn.progress().current, random.name);
}

#[test]
fn a_question_opens_in_the_strategy_its_own_file_asked_for_unless_something_beats_it() {
	let set: QuestionSet = imported(MODE_SET);
	let questions: Vec<Question> = set.of_language(Language::Python);
	let eager: &Question = &questions[0];
	let plain: &Question = &questions[1];
	let logic_questions: Vec<Question> = set.of_language(Language::Logic);
	let logic: &Question = &logic_questions[0];

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
	assert_eq!(
		logic.evaluation().expect("an evaluation question").mode(),
		EvaluationMode::Eager
	);
	assert_eq!(
		stepwise::app::starting_mode(None, &on_logic, logic),
		EvaluationMode::ShortCircuit
	);
	assert_eq!(
		stepwise::app::starting_mode(None, &on_logic, eager),
		EvaluationMode::Eager
	);
}

/// A course is one language's questions of a set, every kind included: the proof in the file
/// is a stop on the logic course, and a course of the other language never reaches it.
#[test]
fn one_file_may_mix_languages_and_question_kinds_and_a_course_gets_every_question_of_its_language()
{
	let set: QuestionSet = imported(MIXED_SET);
	assert_eq!(set.questions().len(), 4);
	assert_eq!(set.exercises().count(), 3);

	// One language's questions, in file order, whichever kind each one is.
	assert_eq!(
		ids(&set.of_language(Language::Python)),
		["py-one", "py-two"]
	);
	assert_eq!(
		ids(&set.of_language(Language::Logic)),
		["logic-one", "chain"]
	);

	// Natural deduction is propositional, so the proof is a logic question — not an
	// evaluation one, and stamped with its set like one.
	let proof: &Question = set.find("chain").expect("the proof question");
	assert_eq!(proof.language(), Language::Logic);
	assert!(proof.evaluation().is_none());
	assert_eq!(proof.set(), "mixed-set");

	// The Python course steps from one Python question to the next and ends there.
	let mut walk: Lesson = lesson(
		set.of_language(Language::Python),
		0,
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	assert!(walk.next_question().unwrap());
	assert_eq!(evaluating(&walk).question().name, "py-two");
	assert!(!walk.next_question().unwrap());
	assert_eq!(walk.report(), &Report::Notice(Notice::CourseEnded));
}

// ---------------------------------------------------------------------------
// One course, both kinds: a logic course walks evaluation and proof questions
// through the same lesson, and whatever the student did on each survives the
// moves between them.
// ---------------------------------------------------------------------------

/// The walk forward, the walk back and the end of the set are the same moves whichever kind
/// of question is in hand, and the end is reported by that question whether it is an
/// expression or a proof.
#[test]
fn a_logic_course_walks_evaluation_and_proof_questions_in_file_order_and_ends_on_either_kind() {
	let set: QuestionSet = imported(WALK_SET);
	let questions: Vec<Question> = set.of_language(Language::Logic);
	assert_eq!(ids(&questions), ["before", "chain", "after"]);

	let mut walk: Lesson = lesson(questions, 0, Progress::default(), EvaluationMode::Eager);
	assert_eq!(evaluating(&walk).question().name, "before");

	assert!(walk.next_question().unwrap());
	assert_eq!(walk.question().name(), "chain");
	assert_eq!(proving(&walk).question().set, "walk-set");
	assert_eq!(walk.report(), &Report::Notice(Notice::ProofStart));
	assert_eq!(proving(&walk).proof().lines().len(), 3);

	assert!(walk.next_question().unwrap());
	assert_eq!(evaluating(&walk).question().name, "after");
	assert_eq!(walk.course().index(), 2);

	// The set ends on an evaluation question, which says so and stays in hand.
	assert!(!walk.next_question().unwrap());
	assert_eq!(walk.report(), &Report::Notice(Notice::CourseEnded));
	assert_eq!(evaluating(&walk).question().name, "after");
	assert_eq!(walk.course().index(), 2);
	assert_eq!(walk.course().questions().len(), 3);

	assert!(walk.previous_question().unwrap());
	assert_eq!(proving(&walk).question().name, "chain");
	assert!(walk.previous_question().unwrap());
	assert_eq!(evaluating(&walk).question().name, "before");
	assert_eq!(walk.course().index(), 0);

	// A set that ends on a proof ends the same way, and the proof in hand keeps the line the
	// student was still typing.
	let mixed: QuestionSet = imported(MIXED_SET);
	let mut ending: Lesson = lesson(
		mixed.of_language(Language::Logic),
		0,
		Progress::default(),
		EvaluationMode::Eager,
	);
	assert!(ending.next_question().unwrap());
	assert_eq!(proving(&ending).question().name, "chain");
	proving_mut(&mut ending).paste("Q ; mp");
	assert!(!ending.next_question().unwrap());
	assert_eq!(ending.report(), &Report::Notice(Notice::CourseEnded));
	assert_eq!(proving(&ending).question().name, "chain");
	assert_eq!(proving(&ending).input(), "Q ; mp");
	assert_eq!(ending.course().questions().len(), 2);
}

/// Moving away records the question being left, and moving back replays it from that record,
/// so neither kind of work is lost on the way. Undo belongs to the question in hand: a proof
/// with no line of the student's own has nothing to take back, and never reaches across into
/// the evaluation question before it.
#[test]
fn work_on_each_question_survives_moving_away_and_back_and_undo_stays_inside_its_question() {
	let set: QuestionSet = imported(WALK_SET);
	let before: Exercise = evaluation_question(&set, "before");
	let chain: ProofQuestion = proof_question(&set, "chain");
	let mut walk: Lesson = lesson(
		set.of_language(Language::Logic),
		0,
		Progress::default(),
		EvaluationMode::Eager,
	);
	step_once(evaluating_mut(&mut walk));
	let stepped_render: String = evaluating(&walk).session().render().into();
	assert_eq!(evaluating(&walk).session().attempts().len(), 1);

	assert!(walk.next_question().unwrap());
	assert!(!proving_mut(&mut walk).undo());
	assert_eq!(walk.report(), &Report::Notice(Notice::ProofStart));
	assert_eq!(proving(&walk).proof().lines().len(), 3);
	let proof: &mut ProofPractice = proving_mut(&mut walk);
	proof.paste(CHAIN[0]);
	assert!(proof.submit());

	// Leaving the proof records its line beside the evaluation question's attempt.
	assert!(walk.next_question().unwrap());
	assert_eq!(evaluating(&walk).question().name, "after");
	assert_eq!(
		walk.progress().commands(&chain.proof().unwrap()),
		[CHAIN[0]]
	);
	assert_eq!(
		walk.progress()
			.attempts(&before.session(EvaluationMode::Eager).unwrap())
			.len(),
		1
	);

	// Back on the proof: its line is there, and undo takes back that line and nothing more.
	assert!(walk.previous_question().unwrap());
	assert_eq!(proving(&walk).proof().commands(), [CHAIN[0]]);
	assert_eq!(proving(&walk).proof().lines().len(), 4);
	assert!(proving_mut(&mut walk).undo());
	assert_eq!(proving(&walk).proof().lines().len(), 3);
	assert!(!proving_mut(&mut walk).undo());

	// Back on the evaluation question: its attempt survived both crossings, and the line
	// taken back on the proof was recorded as taken back.
	assert!(walk.previous_question().unwrap());
	assert_eq!(evaluating(&walk).session().attempts().len(), 1);
	assert_eq!(evaluating(&walk).session().render(), stepped_render);
	assert!(walk.progress().commands(&chain.proof().unwrap()).is_empty());
	assert!(evaluating_mut(&mut walk).undo());
	assert!(evaluating(&walk).session().attempts().is_empty());
}

/// A proof has no evaluation strategy, so the one the student switched to on an evaluation
/// question is carried across it and the next evaluation question opens in it; a course that
/// opens on a proof carries the strategy it was opened with.
#[test]
fn the_strategy_the_student_chose_is_carried_across_a_proof_to_the_next_evaluation_question() {
	let set: QuestionSet = imported(WALK_SET);
	let mut walk: Lesson = lesson(
		set.of_language(Language::Logic),
		0,
		Progress::default(),
		EvaluationMode::Eager,
	);
	assert_eq!(walk.mode(), EvaluationMode::Eager);
	assert!(evaluating_mut(&mut walk).toggle_mode().unwrap());
	assert_eq!(walk.mode(), EvaluationMode::ShortCircuit);

	assert!(walk.next_question().unwrap());
	assert_eq!(proving(&walk).question().name, "chain");
	assert_eq!(walk.mode(), EvaluationMode::ShortCircuit);

	assert!(walk.next_question().unwrap());
	assert_eq!(evaluating(&walk).question().name, "after");
	assert_eq!(
		evaluating(&walk).session().mode(),
		EvaluationMode::ShortCircuit
	);
	// Recording the proof on the way left the saved strategy where the evaluation question
	// before it put it.
	assert_eq!(walk.progress().mode, EvaluationMode::ShortCircuit);

	// Back across the proof, the first question opens in that strategy too.
	assert!(walk.previous_question().unwrap());
	assert!(walk.previous_question().unwrap());
	assert_eq!(evaluating(&walk).question().name, "before");
	assert_eq!(
		evaluating(&walk).session().mode(),
		EvaluationMode::ShortCircuit
	);

	// Opened on a proof, the course carries what it was opened with — here not the logic
	// default — into the evaluation question it goes back to.
	let mixed: QuestionSet = imported(MIXED_SET);
	let mut on_proof: Lesson = lesson(
		mixed.of_language(Language::Logic),
		1,
		Progress::default(),
		EvaluationMode::ShortCircuit,
	);
	assert_eq!(proving(&on_proof).question().name, "chain");
	assert_eq!(on_proof.mode(), EvaluationMode::ShortCircuit);
	assert!(on_proof.previous_question().unwrap());
	assert_eq!(
		evaluating(&on_proof).session().mode(),
		EvaluationMode::ShortCircuit
	);
}

/// Recording a proof moves the pointer to it, set and name, so a later launch knows where the
/// student was. The strategy is an evaluation matter and is left as it was. A proof written
/// out on the command line belongs to no set and says so with an empty name, and a bare
/// launch after it has nothing to rebuild it from, so it draws a new question.
#[test]
fn recording_a_proof_points_the_progress_at_it_and_leaves_the_saved_strategy_alone() {
	let set: QuestionSet = imported(MIXED_SET);
	let chain: ProofQuestion = proof_question(&set, "chain");
	assert_eq!(chain.set, "mixed-set");
	for mode in [EvaluationMode::ShortCircuit, EvaluationMode::Eager] {
		let mut elsewhere: Progress = Progress::default();
		elsewhere.current_set = "other-set".into();
		elsewhere.current = "q1".into();
		elsewhere.mode = mode;
		let recorded: Progress = proved(&chain, &CHAIN[..1], elsewhere);
		assert!(recorded.points_at("mixed-set", "chain"));
		assert_eq!(recorded.mode, mode);
		assert_eq!(recorded.commands(&chain.proof().unwrap()), &CHAIN[..1]);
	}

	let custom: ProofQuestion = written(&["P", "Q"], "(P ∧ Q) ∨ R");
	let recorded: Progress = proved(&custom, &["P ∧ Q ; and-intro ; 1,2"], Progress::default());
	assert_eq!(recorded.current_set, "");
	assert_eq!(recorded.current, "custom-proof");
	assert_eq!(recorded.commands(&custom.proof().unwrap()).len(), 1);
	let drawn: Question =
		stepwise::app::resume_or_generate(Language::Logic, &recorded, &[]).unwrap();
	assert!(drawn.name().starts_with("random-v1-logic-"));
	assert!(drawn.evaluation().is_some());
}

/// An ordered set opens at the first question still unfinished from where the pointer left
/// off, and a proof is judged by its own lines: finished ones are passed over, and one that
/// was opened and left with no line of the student's is unfinished.
#[test]
fn an_ordered_set_skips_a_finished_proof_and_stops_at_an_unfinished_one() {
	let set: QuestionSet = imported(WALK_SET);
	let questions: Vec<Question> = set.of_language(Language::Logic);
	let before: Exercise = evaluation_question(&set, "before");
	let after: Exercise = evaluation_question(&set, "after");
	let chain: ProofQuestion = proof_question(&set, "chain");
	// Logic evaluation questions are worked in the language's own strategy, the one the set
	// judges a question the pointer does not name in.
	let eager: EvaluationMode = Language::Logic.default_mode();
	let resume = |progress: &Progress| -> usize {
		stepwise::app::resume_in_set(progress, &questions, None).unwrap()
	};

	// The pointer on the proof itself.
	let opened: Progress = proved(&chain, &[], Progress::default());
	assert!(opened.points_at("walk-set", "chain"));
	assert_eq!(resume(&opened), 1);
	assert_eq!(resume(&proved(&chain, &CHAIN[..1], Progress::default())), 1);
	assert_eq!(resume(&proved(&chain, &CHAIN, Progress::default())), 2);

	// The pointer on the finished question before it: an unfinished proof is where the set
	// stops, whether it has a line, an empty record or no record at all.
	let first_done: Progress = stepped(&before, eager, true, Progress::default());
	assert!(first_done.points_at("walk-set", "before"));
	assert!(first_done.proofs.is_empty());
	assert_eq!(resume(&first_done), 1);
	assert_eq!(resume(&stepped(&before, eager, true, opened.clone())), 1);
	assert_eq!(
		resume(&stepped(
			&before,
			eager,
			true,
			proved(&chain, &CHAIN[..1], Progress::default())
		)),
		1
	);
	// A finished proof is passed over, to the evaluation question after it.
	let proof_done: Progress = proved(&chain, &CHAIN, Progress::default());
	assert_eq!(
		resume(&stepped(&before, eager, true, proof_done.clone())),
		2
	);

	// Everything finished: the last question stays in hand, from wherever the pointer is.
	let all_done: Progress = stepped(
		&after,
		eager,
		true,
		stepped(&before, eager, true, proof_done.clone()),
	);
	assert!(all_done.points_at("walk-set", "after"));
	assert_eq!(resume(&all_done), 2);
	let pointer_first: Progress = stepped(
		&before,
		eager,
		true,
		stepped(&after, eager, true, proof_done.clone()),
	);
	assert!(pointer_first.points_at("walk-set", "before"));
	assert_eq!(resume(&pointer_first), 2);

	// And when the last question is a finished proof, that proof stays in hand.
	let mixed: QuestionSet = imported(MIXED_SET);
	let mixed_questions: Vec<Question> = mixed.of_language(Language::Logic);
	let logic_one: Exercise = evaluation_question(&mixed, "logic-one");
	let mixed_chain: ProofQuestion = proof_question(&mixed, "chain");
	let finished: Progress = proved(
		&mixed_chain,
		&CHAIN,
		stepped(&logic_one, eager, true, Progress::default()),
	);
	assert!(finished.points_at("mixed-set", "chain"));
	assert_eq!(
		stepwise::app::resume_in_set(&finished, &mixed_questions, None).unwrap(),
		1
	);
}

/// The built-in proofs are questions of the embedded set, so a bare launch treats the one the
/// pointer names like any saved question: unfinished work reopens, and a finished proof, or
/// one opened and left without a line, gives way to a new random question. Proofs are logic,
/// so a Python launch never reopens one.
#[test]
fn a_bare_launch_reopens_an_unfinished_built_in_proof_and_replaces_a_finished_one() {
	let embedded: QuestionSet = exercises::builtin().unwrap();
	let logic: Vec<Question> = embedded.of_language(Language::Logic);
	let raa: ProofQuestion = builtin_proof("raa");
	assert_eq!(raa.set, "builtin");
	let started: [&str; 2] = ["~P ; assume", "False ; not-elim ; 1,2"];

	let two_lines: Progress = proved(&raa, &started, Progress::default());
	assert!(two_lines.points_at("builtin", "raa"));
	let reopened: Question =
		stepwise::app::resume_or_generate(Language::Logic, &two_lines, &logic).unwrap();
	assert_eq!(reopened.name(), "raa");
	assert!(reopened.evaluation().is_none());

	// The lesson it opens continues from the saved lines, and random practice draws an
	// evaluation question after it: the generator writes expressions, not proofs.
	let mode: EvaluationMode = stepwise::app::starting_mode(None, &two_lines, &reopened);
	let mut resumed: Lesson = Lesson::new(
		Course::random(vec![reopened], 0).unwrap(),
		two_lines.clone(),
		mode,
	)
	.unwrap();
	assert_eq!(proving(&resumed).proof().lines().len(), 3);
	let proof: &mut ProofPractice = proving_mut(&mut resumed);
	proof.paste("P ; raa ; 2,3");
	assert!(proof.submit());
	assert!(proof.is_finished());
	assert!(resumed.next_question().unwrap());
	assert!(
		evaluating(&resumed)
			.question()
			.name
			.starts_with("random-v1-logic-")
	);

	let finished: Progress = proved(
		&raa,
		&["~P ; assume", "False ; not-elim ; 1,2", "P ; raa ; 2,3"],
		Progress::default(),
	);
	let replaced: Question =
		stepwise::app::resume_or_generate(Language::Logic, &finished, &logic).unwrap();
	assert!(replaced.name().starts_with("random-v1-logic-"));
	assert!(replaced.evaluation().is_some());

	let untouched: Progress = proved(&raa, &[], Progress::default());
	assert!(untouched.points_at("builtin", "raa"));
	assert!(
		stepwise::app::resume_or_generate(Language::Logic, &untouched, &logic)
			.unwrap()
			.name()
			.starts_with("random-v1-logic-")
	);

	// Every question of the set is offered, so only the language rule keeps the proof out.
	let python: Question =
		stepwise::app::resume_or_generate(Language::Python, &two_lines, embedded.questions())
			.unwrap();
	assert!(python.name().starts_with("random-v1-python-"));
	assert_eq!(
		two_lines.proofs,
		proved(&raa, &started, Progress::default()).proofs
	);
}

/// A proof has no strategy of its own: the one asked for, else its language's default. A
/// saved strategy never reaches it, even when the pointer names the proof.
#[test]
fn a_proof_question_starts_in_the_requested_strategy_or_its_languages_default() {
	let mp: ProofQuestion = builtin_proof("mp");
	let question: Question = Question::Proof(mp.clone());
	assert_eq!(
		stepwise::app::starting_mode(None, &Progress::default(), &question),
		EvaluationMode::Eager
	);
	assert_eq!(
		stepwise::app::starting_mode(
			Some(EvaluationMode::ShortCircuit),
			&Progress::default(),
			&question
		),
		EvaluationMode::ShortCircuit
	);

	let mut pointed: Progress = proved(&mp, &[], Progress::default());
	pointed.mode = EvaluationMode::ShortCircuit;
	assert!(pointed.points_at("builtin", "mp"));
	assert_eq!(
		stepwise::app::starting_mode(None, &pointed, &question),
		EvaluationMode::Eager
	);
}

/// One record across both kinds: each question opens its own block, separated from the one
/// before by one blank line. An evaluation block ends on the expression as it was left; a
/// proof owes nothing when it closes, since each line is archived once as it is accepted.
#[test]
fn the_transcript_archives_evaluation_and_proof_blocks_across_one_lesson() {
	let set: QuestionSet = imported(WALK_SET);
	let mut walk: Lesson = lesson(
		set.of_language(Language::Logic),
		0,
		Progress::default(),
		EvaluationMode::Eager,
	);
	let mut transcript: Transcript = Transcript::default();
	assert_eq!(transcript.sync(&walk), ["命题逻辑", "P=True Q=False"]);

	step_once(evaluating_mut(&mut walk));
	assert_eq!(transcript.sync(&walk), ["P ∧ Q"]);
	assert_eq!(evaluating(&walk).session().render(), "True ∧ Q");

	assert!(walk.next_question().unwrap());
	assert_eq!(
		transcript.sync(&walk),
		[
			"True ∧ Q",
			"",
			"自然演绎 · 目标：R",
			"1 (P → Q) [premise ]",
			"2 (Q → R) [premise ]",
			"3 P [premise ]",
		]
	);
	assert!(transcript.sync(&walk).is_empty());

	let proof: &mut ProofPractice = proving_mut(&mut walk);
	proof.paste(CHAIN[0]);
	assert!(proof.submit());
	assert_eq!(transcript.sync(&walk), ["4 Q [mp 1,3]"]);
	assert!(transcript.sync(&walk).is_empty());
	let proof: &mut ProofPractice = proving_mut(&mut walk);
	proof.paste(CHAIN[1]);
	assert!(proof.submit());
	assert_eq!(transcript.sync(&walk), ["5 R [mp 2,4]"]);
	assert!(proving_mut(&mut walk).undo());
	assert_eq!(
		transcript.sync(&walk),
		["↶ 撤销第 5 行及其假设作用域变更。"]
	);
	assert!(transcript.sync(&walk).is_empty());

	assert!(walk.next_question().unwrap());
	assert_eq!(transcript.sync(&walk), ["", "命题逻辑", "P=False Q=True"]);
	assert_eq!(evaluating(&walk).session().render(), "P ∨ Q");

	// Going back opens each question afresh: the proof with every line it holds, and the
	// evaluation question with the steps already taken on it.
	assert!(walk.previous_question().unwrap());
	assert_eq!(
		transcript.sync(&walk),
		[
			"P ∨ Q",
			"",
			"自然演绎 · 目标：R",
			"1 (P → Q) [premise ]",
			"2 (Q → R) [premise ]",
			"3 P [premise ]",
			"4 Q [mp 1,3]",
		]
	);
	assert!(walk.previous_question().unwrap());
	assert_eq!(
		transcript.sync(&walk),
		["", "命题逻辑", "P=True Q=False", "P ∧ Q"]
	);
	assert!(transcript.sync(&walk).is_empty());
}
