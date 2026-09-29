//! The desktop window's Rust half answers its page without a window: every command below goes
//! through the same `Desk` the shell drives, and every assertion reads the `View` the page
//! would draw.

use stepwise::{
	core::NodeId,
	desktop::{Blank, Choice, Command, Desk, EvaluationView, ProofView, Run, Tone, View, Written},
	exercises,
	progress::Progress,
};

/// A small set: a repeated variable, Unicode logic symbols, and a proof after them.
const SET: &str = r#"
version = 2
name = "desk-walk"
title = "桌面走查"

[[questions]]
kind = "evaluation"
name = "same-name"
title = "同名变量"
language = "python"
expression = "x + y * x"
[questions.bindings]
x = "2"
y = "3"

[[questions]]
kind = "evaluation"
name = "symbols"
title = "符号"
language = "logic"
expression = "(P → Q) ∧ ¬Q → ¬P"
[questions.bindings]
P = "true"
Q = "false"

[[questions]]
kind = "proof"
name = "chain"
title = "连锁推理"
premises = ["P -> Q", "Q -> R", "P"]
conclusion = "R"
"#;

fn desk(progress: Progress) -> Desk {
	Desk::new(progress, exercises::builtin().unwrap(), None)
}

fn evaluation(desk: &Desk) -> EvaluationView {
	match desk.view() {
		View::Evaluation(view) => view,
		other => panic!("expected an evaluation view, got {other:?}"),
	}
}

fn proving(desk: &Desk) -> ProofView {
	match desk.view() {
		View::Proof(view) => view,
		other => panic!("expected a proof view, got {other:?}"),
	}
}

fn text(runs: &[Run]) -> String {
	runs.iter().map(|run| run.text.as_str()).collect()
}

/// The node whose runs spell exactly `source` on the line in hand.
fn node(view: &EvaluationView, source: &str) -> NodeId {
	view.extents
		.iter()
		.find(|(_, [first, last])| text(&view.current[*first..=*last]) == source)
		.map(|(node, _)| *node)
		.unwrap_or_else(|| panic!("no node spells {source} in {}", text(&view.current)))
}

/// Random practice in `language`, then the embedded question named `name` from the catalog.
fn open_builtin(desk: &mut Desk, language: Choice, name: &str) {
	desk.handle(Command::Choose { language });
	let index: usize = match desk.view() {
		View::Evaluation(view) => view.catalog.questions,
		View::Proof(view) => view.catalog.questions,
		View::Entry(_) => panic!("still choosing a language"),
	}
	.iter()
	.position(|question| question.name == name)
	.unwrap_or_else(|| panic!("{name} is not in the catalog"));
	assert!(desk.handle(Command::Pick { index }));
}

/// Which state of the board the view shows.
fn edition(desk: &Desk) -> u32 {
	match desk.view() {
		View::Evaluation(view) => view.edition,
		View::Proof(view) => view.edition,
		View::Entry(_) => panic!("no board on the language choice"),
	}
}

/// Type `text` into the open blank or proof line, as the page sends it.
fn type_in(desk: &mut Desk, text: &str) -> bool {
	let edition: u32 = edition(desk);
	desk.handle(Command::Draft {
		text: text.into(),
		edition,
	})
}

fn answer(desk: &mut Desk, source: &str, value: &str) -> bool {
	let target: NodeId = node(&evaluation(desk), source);
	desk.handle(Command::Select { node: target });
	type_in(desk, value);
	desk.handle(Command::Submit)
}

#[test]
fn the_window_opens_on_the_language_choice_and_a_language_opens_practice_with_nothing_pointed_at() {
	let mut desk: Desk = desk(Progress::default());
	assert!(matches!(desk.view(), View::Entry(ref entry) if entry.set.is_none()));
	assert!(desk.handle(Command::Choose {
		language: Choice::Python
	}));
	let view: EvaluationView = evaluation(&desk);
	assert_eq!(view.course.language, "Python");
	assert_eq!(view.course.count, None);
	assert!(view.course.forward && !view.course.back);
	// The session starts selected on the whole expression, which nobody chose: nothing rings.
	assert_eq!(view.selected, None);
	assert!(
		view.draft.is_none()
			|| view
				.draft
				.as_ref()
				.is_some_and(|draft| draft.runs.len() == 1)
	);
	assert_eq!(view.feedback.tone, Tone::Plain);
	assert_eq!(view.catalog.title, "内置题库");
}

#[test]
fn a_click_opens_the_line_below_with_the_node_blanked_and_a_wrong_answer_keeps_it() {
	let mut desk: Desk = desk(Progress::default());
	open_builtin(&mut desk, Choice::Python, "precedence");
	let view: EvaluationView = evaluation(&desk);
	assert_eq!(text(&view.current), "2 + (3 * 4)");
	assert!(view.history.is_empty());

	let multiply: NodeId = node(&view, "3 * 4");
	assert!(!desk.handle(Command::Select { node: multiply }));
	let draft = evaluation(&desk).draft.expect("a blank is open");
	assert_eq!(text(&draft.runs), "2 + (3 * 4)");
	let blanks: Vec<&Run> = draft
		.runs
		.iter()
		.filter(|run| run.blank.is_some())
		.collect();
	assert_eq!(blanks.len(), 1);
	assert_eq!(
		(blanks[0].text.as_str(), blanks[0].node, blanks[0].blank),
		("3 * 4", Some(multiply), Some(Blank::Input))
	);

	type_in(&mut desk, "13");
	assert!(!desk.handle(Command::Submit));
	let wrong: EvaluationView = evaluation(&desk);
	assert_eq!(wrong.feedback.tone, Tone::Bad);
	assert_eq!(
		wrong.draft.as_ref().map(|draft| draft.input.as_str()),
		Some("13")
	);
	assert_eq!(text(&wrong.current), "2 + (3 * 4)");

	// Editing the draft is not judged: the mistake is no longer the last word.
	type_in(&mut desk, "12");
	assert_eq!(evaluation(&desk).feedback.tone, Tone::Plain);
	assert!(desk.handle(Command::Submit));
	let right: EvaluationView = evaluation(&desk);
	assert_eq!(right.feedback.tone, Tone::Good);
	assert_eq!(text(&right.current), "2 + (12)");
	assert_eq!(right.history, [Written::Expression("2 + (3 * 4)".into())]);
	assert!(right.draft.is_none());
}

#[test]
fn a_finished_group_comes_off_at_a_click_and_the_final_pair_opens_its_own_blank() {
	let mut desk: Desk = desk(Progress::default());
	open_builtin(&mut desk, Choice::Python, "precedence");
	assert!(answer(&mut desk, "3 * 4", "12"));
	let group: NodeId = node(&evaluation(&desk), "(12)");
	assert!(desk.handle(Command::Select { node: group }));
	let pair: EvaluationView = evaluation(&desk);
	assert_eq!(text(&pair.current), "2 + 12");
	let draft = pair.draft.expect("the final pair opens its blank");
	assert_eq!(draft.runs.len(), 1);
	assert_eq!(
		(draft.runs[0].text.as_str(), draft.runs[0].blank),
		("2 + 12", Some(Blank::Input))
	);
	assert_eq!(draft.input, "");

	type_in(&mut desk, "14");
	assert!(desk.handle(Command::Submit));
	let done: EvaluationView = evaluation(&desk);
	assert_eq!(done.ending.as_deref(), Some("14"));
	assert!(done.draft.is_none());
}

#[test]
fn an_exception_ending_names_the_sub_expression_that_raised_it() {
	let mut desk: Desk = desk(Progress::default());
	open_builtin(&mut desk, Choice::Python, "exception");
	assert!(answer(&mut desk, "1 + 2", "3"));
	let group: NodeId = node(&evaluation(&desk), "(3)");
	assert!(desk.handle(Command::Select { node: group }));
	assert!(answer(&mut desk, "3 - 3", "0"));
	let group: NodeId = node(&evaluation(&desk), "(0)");
	assert!(desk.handle(Command::Select { node: group }));
	// Only two values and a division are left, so the blank opens by itself.
	assert!(evaluation(&desk).draft.is_some());
	type_in(&mut desk, "ZeroDivisionError");
	assert!(desk.handle(Command::Submit));

	let ended: EvaluationView = evaluation(&desk);
	assert_eq!(
		ended.ending.as_deref(),
		Some("3 / 0 引发 ZeroDivisionError")
	);
	assert_eq!(ended.feedback.tone, Tone::Good);
	// Nothing is left to name: the hint says so without judging anything.
	assert!(!desk.handle(Command::Hint));
	assert_eq!(evaluation(&desk).feedback.tone, Tone::Plain);
}

#[test]
fn the_keyboard_reaches_the_node_a_click_reaches_and_opens_the_same_blank() {
	let mut clicked: Desk = desk(Progress::default());
	open_builtin(&mut clicked, Choice::Python, "precedence");
	let multiply: NodeId = node(&evaluation(&clicked), "3 * 4");
	clicked.handle(Command::Select { node: multiply });

	let mut typed: Desk = desk(Progress::default());
	open_builtin(&mut typed, Choice::Python, "precedence");
	let mut steps: usize = 0;
	while evaluation(&typed).selected != Some(multiply) {
		typed.handle(Command::Step { forward: true });
		steps += 1;
		assert!(steps < 8, "the keyboard never reached 3 * 4");
	}
	assert!(evaluation(&typed).draft.is_none());
	// Enter with no blank open opens one at the node pointed at.
	typed.handle(Command::Submit);
	assert_eq!(evaluation(&typed).draft, evaluation(&clicked).draft);
	// While a blank is open the arrows do nothing.
	typed.handle(Command::Step { forward: true });
	assert_eq!(evaluation(&typed).draft, evaluation(&clicked).draft);
}

#[test]
fn one_answer_fills_every_occurrence_of_a_name_and_undo_is_recorded_as_taken_back() {
	let mut desk: Desk = desk(Progress::default());
	assert!(!desk.load(SET, "desk-walk.toml"));
	assert!(
		matches!(desk.view(), View::Entry(ref entry) if entry.set.as_deref() == Some("桌面走查"))
	);
	assert!(desk.handle(Command::Choose {
		language: Choice::Python
	}));
	let view: EvaluationView = evaluation(&desk);
	assert_eq!(view.course.set.as_deref(), Some("桌面走查"));
	assert_eq!((view.course.position, view.course.count), (1, Some(1)));
	assert!(!view.course.forward);
	assert_eq!(view.bindings, ["x = 2", "y = 3"]);

	let first_x: NodeId = view
		.current
		.iter()
		.find(|run| run.text == "x")
		.and_then(|run| run.node)
		.unwrap();
	desk.handle(Command::Select { node: first_x });
	let draft = evaluation(&desk).draft.unwrap();
	let kinds: Vec<(&str, Option<Blank>)> = draft
		.runs
		.iter()
		.filter(|run| run.blank.is_some())
		.map(|run| (run.text.as_str(), run.blank))
		.collect();
	assert_eq!(
		kinds,
		[("x", Some(Blank::Input)), ("x", Some(Blank::Mirror))]
	);
	type_in(&mut desk, "2");
	assert!(desk.handle(Command::Submit));
	assert_eq!(text(&evaluation(&desk).current), "2 + y * 2");

	assert!(desk.handle(Command::Undo));
	let undone: EvaluationView = evaluation(&desk);
	assert_eq!(text(&undone.current), "x + y * x");
	assert_eq!(
		undone.history,
		[
			Written::Expression("x + y * x".into()),
			Written::TakenBack("↶ 已撤销上一步。".into())
		]
	);
	assert_eq!(undone.selected, None);
	assert_eq!(undone.feedback.tone, Tone::Plain);
}

#[test]
fn unicode_symbols_keep_their_runs_whole_and_a_hint_is_not_a_mistake() {
	let mut desk: Desk = desk(Progress::default());
	desk.load(SET, "desk-walk.toml");
	desk.handle(Command::Choose {
		language: Choice::Logic,
	});
	let view: EvaluationView = evaluation(&desk);
	assert_eq!(view.course.language, "命题逻辑");
	assert_eq!(text(&view.current), "(P → Q) ∧ ¬Q → ¬P");
	// Every run is whole characters and every extent spells a well-formed piece.
	assert!(view.current.iter().all(|run| !run.text.is_empty()));
	let negation: NodeId = node(&view, "¬P");
	desk.handle(Command::Select { node: negation });
	desk.handle(Command::Cancel);
	assert_eq!(evaluation(&desk).selected, Some(negation));

	desk.handle(Command::Hint);
	let hinted: EvaluationView = evaluation(&desk);
	assert_eq!(hinted.feedback.tone, Tone::Plain);
	assert!(!hinted.feedback.text.is_empty());

	let q: NodeId = hinted
		.current
		.iter()
		.find(|run| run.text == "Q")
		.and_then(|run| run.node)
		.unwrap();
	desk.handle(Command::Select { node: q });
	let mirrored: usize = evaluation(&desk)
		.draft
		.unwrap()
		.runs
		.iter()
		.filter(|run| run.text == "Q" && run.blank.is_some())
		.count();
	assert_eq!(mirrored, 2);
}

#[test]
fn a_set_walks_in_order_into_a_proof_and_ends_at_its_last_question() {
	let mut desk: Desk = desk(Progress::default());
	desk.load(SET, "desk-walk.toml");
	desk.handle(Command::Choose {
		language: Choice::Logic,
	});
	assert_eq!(evaluation(&desk).course.count, Some(2));
	assert!(desk.handle(Command::Next));

	let proof: ProofView = proving(&desk);
	assert_eq!(proof.course.language, "自然演绎");
	assert_eq!((proof.course.position, proof.course.forward), (2, false));
	assert_eq!(proof.goal, "R");
	assert_eq!(proof.lines.len(), 3);
	assert!(
		proof
			.lines
			.iter()
			.all(|line| line.premise && line.depth == 0)
	);
	assert_eq!((proof.open, proof.finished, proof.judged), (0, false, None));

	type_in(&mut desk, "R ; mp ; 1,3");
	assert!(!desk.handle(Command::Submit));
	let refused: ProofView = proving(&desk);
	assert_eq!(refused.feedback.tone, Tone::Bad);
	assert_eq!(refused.input, "R ; mp ; 1,3");
	assert_eq!(refused.judged, None);

	type_in(&mut desk, "Q ; mp ; 1,3");
	assert!(desk.handle(Command::Submit));
	let accepted: ProofView = proving(&desk);
	assert_eq!(accepted.feedback.tone, Tone::Good);
	assert_eq!(accepted.judged, Some(4));
	assert_eq!(accepted.lines[3].references, [1, 3]);
	assert_eq!(accepted.input, "");

	desk.handle(Command::Rules);
	assert!(proving(&desk).rules.is_some());
	// The rules are shown beside the verdict, not instead of it.
	assert_eq!(proving(&desk).feedback, accepted.feedback);
	desk.handle(Command::Rules);
	assert!(proving(&desk).rules.is_none());

	assert!(!desk.handle(Command::Next));
	assert_eq!(proving(&desk).feedback.text, "已经是本题集的最后一题。");
	assert!(desk.handle(Command::Undo));
	assert_eq!(proving(&desk).lines.len(), 3);
}

#[test]
fn a_set_that_fails_to_load_changes_nothing_and_says_which_file() {
	let mut desk: Desk = desk(Progress::default());
	open_builtin(&mut desk, Choice::Python, "precedence");
	assert!(answer(&mut desk, "3 * 4", "12"));
	let before: String = serde_json::to_string(desk.progress()).unwrap();
	let view: EvaluationView = evaluation(&desk);

	assert!(!desk.load("version = 99", "坏题集.toml"));
	let after: EvaluationView = evaluation(&desk);
	assert!(
		after
			.message
			.as_deref()
			.is_some_and(|message| message.contains("坏题集.toml"))
	);
	assert_eq!((after.current, after.course), (view.current, view.course));
	assert_eq!(serde_json::to_string(desk.progress()).unwrap(), before);

	// A set with nothing in the language being practised is refused the same way.
	assert!(!desk.load(
		&SET.replace("language = \"python\"", "language = \"logic\""),
		"logic-only.toml"
	));
	assert!(evaluation(&desk).message.is_some());
	assert_eq!(serde_json::to_string(desk.progress()).unwrap(), before);
}

#[test]
fn leaving_and_relaunching_resume_the_unfinished_question_with_its_record() {
	let mut first: Desk = desk(Progress::default());
	open_builtin(&mut first, Choice::Python, "precedence");
	assert!(answer(&mut first, "3 * 4", "12"));
	assert!(first.handle(Command::Leave));
	assert!(matches!(first.view(), View::Entry(_)));
	first.handle(Command::Choose {
		language: Choice::Python,
	});
	assert_eq!(text(&evaluation(&first).current), "2 + (12)");

	// A new window over the same progress reopens it the way a relaunch of the terminal does.
	let saved: Progress = first.progress().clone();
	let mut second: Desk = desk(saved);
	second.handle(Command::Choose {
		language: Choice::Python,
	});
	let resumed: EvaluationView = evaluation(&second);
	assert_eq!(resumed.course.title, "先乘后加");
	assert_eq!(text(&resumed.current), "2 + (12)");
	assert_eq!(resumed.history, [Written::Expression("2 + (3 * 4)".into())]);
}

#[test]
fn enter_with_nothing_pointed_at_judges_nothing_and_the_first_arrow_starts_at_the_whole_expression()
{
	let mut desk: Desk = desk(Progress::default());
	open_builtin(&mut desk, Choice::Python, "precedence");
	let fresh: EvaluationView = evaluation(&desk);
	// The session's own selection is the whole expression, which nobody chose.
	assert!(!desk.handle(Command::Submit));
	let unmoved: EvaluationView = evaluation(&desk);
	assert_eq!(unmoved.feedback, fresh.feedback);
	assert!(unmoved.draft.is_none());

	desk.handle(Command::Step { forward: true });
	let whole: NodeId = node(&fresh, "2 + (3 * 4)");
	assert_eq!(evaluation(&desk).selected, Some(whole));
	desk.handle(Command::Step { forward: true });
	assert_ne!(evaluation(&desk).selected, Some(whole));
	desk.handle(Command::Step { forward: false });
	assert_eq!(evaluation(&desk).selected, Some(whole));

	// After an accepted step the node it answered is gone, and nothing is pointed at again.
	assert!(answer(&mut desk, "3 * 4", "12"));
	assert_eq!(evaluation(&desk).selected, None);
	assert!(!desk.handle(Command::Submit));
	assert_ne!(evaluation(&desk).feedback.tone, Tone::Bad);
	assert!(evaluation(&desk).draft.is_none());

	// Nor does Enter on a finished question call anything a mistake.
	let group: NodeId = node(&evaluation(&desk), "(12)");
	desk.handle(Command::Select { node: group });
	type_in(&mut desk, "14");
	assert!(desk.handle(Command::Submit));
	assert!(evaluation(&desk).ending.is_some());
	assert!(!desk.handle(Command::Submit));
	assert_eq!(evaluation(&desk).feedback.tone, Tone::Good);
}

#[test]
fn clicking_the_blank_already_open_judges_nothing() {
	let mut desk: Desk = desk(Progress::default());
	open_builtin(&mut desk, Choice::Python, "precedence");
	let multiply: NodeId = node(&evaluation(&desk), "3 * 4");
	desk.handle(Command::Select { node: multiply });
	type_in(&mut desk, "12");
	desk.handle(Command::Hint);
	assert_eq!(evaluation(&desk).feedback.tone, Tone::Plain);
	// The hint's sentence is still the last word; a click on the open blank must not turn
	// it into a mistake, and must not touch the draft.
	assert!(!desk.handle(Command::Select { node: multiply }));
	let clicked: EvaluationView = evaluation(&desk);
	assert_eq!(clicked.feedback.tone, Tone::Plain);
	assert_eq!(clicked.draft.map(|draft| draft.input), Some("12".into()));
}

#[test]
fn a_draft_typed_for_an_earlier_board_is_dropped_and_only_drafts_keep_the_edition() {
	let mut desk: Desk = desk(Progress::default());
	open_builtin(&mut desk, Choice::Python, "precedence");
	let multiply: NodeId = node(&evaluation(&desk), "3 * 4");
	desk.handle(Command::Select { node: multiply });
	let typed_on: u32 = edition(&desk);
	type_in(&mut desk, "12");
	assert_eq!(edition(&desk), typed_on);
	assert!(desk.handle(Command::Submit));
	let next: u32 = edition(&desk);
	assert_ne!(next, typed_on);

	// Keys typed into the old blank while the submit was on its way arrive late.
	let group: NodeId = node(&evaluation(&desk), "(12)");
	desk.handle(Command::Select { node: group });
	let pair: EvaluationView = evaluation(&desk);
	assert_eq!(
		pair.draft.as_ref().map(|draft| draft.input.as_str()),
		Some("")
	);
	assert!(!desk.handle(Command::Draft {
		text: "123".into(),
		edition: typed_on,
	}));
	assert_eq!(
		evaluation(&desk).draft.map(|draft| draft.input),
		Some(String::new())
	);

	// Leaving a proof and coming back reopens it with an empty line, as in the terminal: an
	// unsubmitted line is not saved work. The edition moves on, so the page takes that empty
	// line instead of keeping what its field still shows.
	let mut proofs: Desk = desk_with_set();
	proofs.handle(Command::Choose {
		language: Choice::Logic,
	});
	proofs.handle(Command::Next);
	type_in(&mut proofs, "Q ; mp");
	let before: u32 = edition(&proofs);
	proofs.handle(Command::Previous);
	proofs.handle(Command::Next);
	assert_ne!(edition(&proofs), before);
	assert_eq!(proving(&proofs).input, "");
}

fn desk_with_set() -> Desk {
	let mut desk: Desk = desk(Progress::default());
	desk.load(SET, "desk-walk.toml");
	desk
}
