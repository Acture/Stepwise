use crossterm::event::{
	Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{
	Terminal, TerminalOptions, Viewport,
	backend::{Backend, TestBackend},
	layout::Position,
};
use unicode_width::UnicodeWidthStr;

use super::{inline, keys::Screen, render, say};
use crate::{
	app::{Course, Notice, Practice, Report, Transcript},
	core::{EvaluationMode, Language, NodeId},
	exercises::{self, Exercise},
	progress::Progress,
};

/// The terminal under test always drives a real practice; nothing here fakes app state.
fn screen(
	questions: Vec<Exercise>,
	progress: Progress,
	index: usize,
	mode: EvaluationMode,
) -> Screen {
	Screen::new(Practice::new(Course::random(questions, index).unwrap(), progress, mode).unwrap())
}
fn app() -> Screen {
	screen(
		exercises::builtin().unwrap().exercises().cloned().collect(),
		Progress::default(),
		0,
		EvaluationMode::ShortCircuit,
	)
}
fn custom(source: &str, language: Language, mode: EvaluationMode) -> Screen {
	screen(
		vec![Exercise {
			set: String::new(),
			id: "test".into(),
			title: "test".into(),
			expression: source.into(),
			goal: String::new(),
			language,
			bindings: Default::default(),
			evaluation: None,
		}],
		Progress::default(),
		0,
		mode,
	)
}
fn key(code: KeyCode) -> Event {
	Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}
/// The sentence this front end actually draws for whatever the app layer reports.
fn feedback(app: &Screen) -> String {
	say::feedback(app.practice.report()).0
}
fn screen_text(terminal: &Terminal<TestBackend>) -> String {
	let buffer: &ratatui::buffer::Buffer = terminal.backend().buffer();
	if buffer.area.width == 0 {
		return String::new();
	}
	let mut text: String = String::new();
	for row in buffer.content.chunks(usize::from(buffer.area.width)) {
		let mut column: usize = 0;
		while column < row.len() {
			let symbol: &str = row[column].symbol();
			text.push_str(symbol);
			column += symbol.width().max(1);
		}
		text.push('\n');
	}
	text
}
fn draw(app: &mut Screen, width: u16, height: u16) -> Terminal<TestBackend> {
	let mut terminal: Terminal<TestBackend> =
		Terminal::new(TestBackend::new(width, height)).unwrap();
	terminal
		.draw(|frame| {
			render::draw(frame, app);
		})
		.unwrap();
	terminal
}
fn click(app: &mut Screen, column: u16, row: u16) -> bool {
	app.handle(Event::Mouse(MouseEvent {
		kind: MouseEventKind::Down(MouseButton::Left),
		column,
		row,
		modifiers: KeyModifiers::NONE,
	}))
	.unwrap()
}
/// Use actual displayed character coordinates, including Unicode cell widths.
fn click_text(app: &mut Screen, terminal: &Terminal<TestBackend>, token: &str) -> bool {
	let text: String = screen_text(terminal);
	let mut target: Option<Position> = None;
	for (row, line) in text.lines().enumerate() {
		for (index, _) in line.match_indices(token) {
			let position: Position = Position::new(line[..index].width() as u16, row as u16);
			if app
				.expression_hits
				.iter()
				.any(|(area, _)| area.contains(position))
			{
				target = Some(position);
			}
		}
	}
	let position: Position = target.expect("visible clickable token");
	click(app, position.x, position.y)
}
fn node(app: &Screen, source: &str) -> NodeId {
	app.practice
		.session()
		.root()
		.rows()
		.into_iter()
		.find(|(_, node)| node.render() == source)
		.unwrap()
		.1
		.id
}

#[test]
fn click_creates_inline_blank_and_only_correct_answer_appends_history() {
	let mut app: Screen = app();
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	assert!(screen_text(&terminal).contains("2 + (3 * 4)"));
	click_text(&mut app, &terminal, "*");
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	assert!(screen_text(&terminal).contains("2 + (____)"));
	assert!(!screen_text(&terminal).contains("12"));
	assert!(screen_text(&terminal).contains("2 + (3 * 4)"));
	assert_eq!(app.practice.session().root().render(), "2 + (3 * 4)");
	assert!(app.practice.session().history().is_empty());
	assert!(app.practice.session().attempts().is_empty());
	app.handle(Event::Paste("13".into())).unwrap();
	assert!(!app.handle(key(KeyCode::Enter)).unwrap());
	assert!(app.practice.session().history().is_empty());
	assert_eq!(app.practice.input(), "13");
	assert!(app.practice.draft().is_some());
	app.handle(key(KeyCode::Backspace)).unwrap();
	app.handle(key(KeyCode::Char('2'))).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert_eq!(app.practice.session().root().render(), "2 + (12)");
	assert_eq!(app.practice.session().history().len(), 1);
	assert!(app.practice.draft().is_none());
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	assert!(screen_text(&terminal).contains("2 + (12)"));
	assert_eq!(app.practice.session().render(), "2 + (12)");
	click_text(&mut app, &terminal, "+");
	assert!(app.practice.draft().is_none());
	assert!(feedback(&app).contains("不能跳过"));
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	assert!(click_text(&mut app, &terminal, "("));
	assert_eq!(
		app.practice.session().attempts().last().unwrap().input,
		None
	);
	assert_eq!(app.practice.session().render(), "2 + 12");
	assert_eq!(app.practice.draft(), Some(app.practice.session().root().id));
	assert!(app.practice.input().is_empty());
	let text: String = screen_text(&draw(&mut app, 40, 8));
	assert!(text.contains("2 + 12"));
	assert!(text.contains("____"));
	app.handle(Event::Paste("14".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert!(app.practice.session().is_finished());
	assert_eq!(app.practice.session().history().len(), 3);
	let text: String = screen_text(&draw(&mut app, 40, 8));
	assert!(text.contains("完成"));
}

#[test]
fn clicking_a_repeated_variable_blanks_and_substitutes_every_occurrence() {
	let exercise: Exercise = Exercise {
		set: String::new(),
		id: "repeated-variable".into(),
		title: "repeated variable".into(),
		expression: "x + y * x".into(),
		goal: String::new(),
		language: Language::Python,
		bindings: std::collections::BTreeMap::from([
			("x".into(), "2".into()),
			("y".into(), "3".into()),
		]),
		evaluation: None,
	};
	let mut app: Screen = screen(
		vec![exercise],
		Progress::default(),
		0,
		EvaluationMode::ShortCircuit,
	);
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	click_text(&mut app, &terminal, "x"); // Click the rightmost occurrence before y.
	let text: String = screen_text(&draw(&mut app, 40, 8));
	assert!(text.contains("x + y * x"));
	assert!(text.contains("____ + y * ____"));
	app.handle(Event::Paste("2".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert_eq!(app.practice.session().render(), "2 + y * 2");
	assert_eq!(app.practice.session().attempts().len(), 1);
	assert!(app.handle(key(KeyCode::Char('u'))).unwrap());
	assert_eq!(app.practice.session().render(), "x + y * x");
}

#[test]
fn final_pair_auto_blank_survives_resume_undo_and_reset_without_auto_solving() {
	let exercise: Exercise = Exercise {
		set: String::new(),
		id: "variables".into(),
		title: "variables".into(),
		expression: "x + 3".into(),
		goal: String::new(),
		language: Language::Python,
		bindings: std::collections::BTreeMap::from([("x".into(), "2".into())]),
		evaluation: None,
	};
	let mut app: Screen = screen(
		vec![exercise.clone()],
		Progress::default(),
		0,
		EvaluationMode::ShortCircuit,
	);
	assert!(app.practice.draft().is_none());
	let mut transcript: Transcript = Transcript::default();
	assert!(transcript.sync(&app.practice).join("\n").contains("x=2"));
	let terminal: Terminal<TestBackend> = draw(&mut app, 30, 8);
	click_text(&mut app, &terminal, "x");
	let text: String = screen_text(&draw(&mut app, 30, 8));
	assert!(text.contains("x + 3"));
	assert!(text.contains("____ + 3"));
	app.handle(Event::Paste("2".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert_eq!(app.practice.draft(), Some(app.practice.session().root().id));
	assert_eq!(app.practice.session().render(), "2 + 3");
	assert_eq!(app.practice.session().history().len(), 1);
	app.practice.record();
	let mut app: Screen = screen(
		vec![exercise],
		app.practice.progress().clone(),
		0,
		EvaluationMode::ShortCircuit,
	);
	assert_eq!(app.practice.draft(), Some(app.practice.session().root().id));
	assert!(app.practice.input().is_empty());
	app.handle(Event::Paste("6".into())).unwrap();
	assert!(!app.handle(key(KeyCode::Enter)).unwrap());
	assert_eq!(app.practice.session().history().len(), 1);
	app.handle(key(KeyCode::Backspace)).unwrap();
	app.handle(Event::Paste("5".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert!(app.practice.session().is_finished());
	assert!(app.handle(key(KeyCode::Char('u'))).unwrap());
	assert_eq!(app.practice.draft(), Some(app.practice.session().root().id));
	let text: String = screen_text(&draw(&mut app, 30, 8));
	assert!(text.contains("2 + 3"));
	assert!(text.contains("____"));
	app.handle(key(KeyCode::Esc)).unwrap();
	assert!(app.practice.draft().is_none());
	assert!(app.handle(key(KeyCode::Char('r'))).unwrap());
	assert_eq!(app.practice.session().render(), "x + 3");
	assert!(app.practice.draft().is_none());
	let app: Screen = custom("2 + 3", Language::Python, EvaluationMode::ShortCircuit);
	assert_eq!(app.practice.draft(), Some(app.practice.session().root().id));
	assert!(app.practice.session().history().is_empty());
}

#[test]
fn incorrect_selection_does_not_blank_or_advance_and_short_circuit_is_selectable() {
	let mut app: Screen = custom(
		"(2 + 3) * (4 + 5)",
		Language::Python,
		EvaluationMode::ShortCircuit,
	);
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	click_text(&mut app, &terminal, "*");
	assert!(app.practice.draft().is_none());
	assert!(feedback(&app).contains("不能跳过"));
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	click_text(&mut app, &terminal, "4 + 5");
	assert_eq!(app.practice.draft(), Some(node(&app, "4 + 5")));
	assert!(app.practice.session().history().is_empty());
	app.handle(Event::Paste("9".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert_eq!(app.practice.session().render(), "(2 + 3) * (9)");

	let mut app: Screen = custom(
		"False and (3 / 0 > 1)",
		Language::Python,
		EvaluationMode::ShortCircuit,
	);
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	click_text(&mut app, &terminal, "/");
	assert!(app.practice.draft().is_none());
	assert!(feedback(&app).contains("跳过"));
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	click_text(&mut app, &terminal, "and");
	assert_eq!(app.practice.draft(), Some(app.practice.session().root().id));
	assert_eq!(
		app.practice.session().root().render(),
		"False and ((3 / 0) > 1)"
	);
	app.handle(Event::Paste("0".into())).unwrap();
	assert!(!app.handle(key(KeyCode::Enter)).unwrap());
	assert!(app.practice.session().history().is_empty());
	app.handle(key(KeyCode::Esc)).unwrap();
	app.handle(key(KeyCode::Char('s'))).unwrap();
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	click_text(&mut app, &terminal, "/");
	assert_eq!(app.practice.draft(), Some(node(&app, "3 / 0")));
	app.handle(Event::Paste("ZeroDivisionError".into()))
		.unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert!(app.practice.session().terminal_error().is_some());
}

#[test]
fn keyboard_submission_undo_and_strategy_toggle_preserve_progress() {
	let mut app: Screen = app();
	app.handle(key(KeyCode::Down)).unwrap();
	app.handle(key(KeyCode::Down)).unwrap();
	app.handle(key(KeyCode::Enter)).unwrap();
	assert_eq!(app.practice.draft(), Some(node(&app, "3 * 4")));
	app.handle(Event::Paste("12".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert_eq!(app.practice.session().root().render(), "2 + (12)");
	assert!(app.handle(key(KeyCode::Char('s'))).unwrap());
	assert_eq!(app.practice.session().mode(), EvaluationMode::Eager);
	assert!(app.practice.session().history().is_empty());
	assert!(app.handle(key(KeyCode::Char('s'))).unwrap());
	assert_eq!(app.practice.session().root().render(), "2 + (12)");
	assert!(app.handle(key(KeyCode::Char('u'))).unwrap());
	assert_eq!(app.practice.session().root().render(), "2 + (3 * 4)");
	assert!(app.practice.session().history().is_empty());
}

#[test]
fn inline_unicode_draft_survives_tiny_resizes_and_tab_does_nothing() {
	let mut app: Screen = app();
	app.handle(key(KeyCode::Down)).unwrap();
	app.handle(key(KeyCode::Down)).unwrap();
	app.handle(key(KeyCode::Enter)).unwrap();
	let editing: Option<NodeId> = app.practice.draft();
	app.handle(Event::Paste("中文\nFalse".into())).unwrap();
	assert_eq!(app.practice.input(), "中文False");
	for (width, height) in [
		(40, 10),
		(20, 6),
		(7, 3),
		(1, 1),
		(0, 0),
		(0, 8),
		(12, 0),
		(100, 32),
	] {
		app.handle(Event::Resize(width, height)).unwrap();
		for _ in 0..3 {
			let mut terminal: Terminal<TestBackend> = draw(&mut app, width, height);
			assert!(!screen_text(&terminal).contains("扩大"));
			if width > 0 && height > 0 {
				let cursor: ratatui::layout::Position = terminal.get_cursor_position().unwrap();
				assert!(cursor.x < width && cursor.y < height);
			}
			assert_eq!(app.practice.input(), "中文False");
			assert_eq!(app.practice.draft(), editing);
			app.handle(key(KeyCode::Tab)).unwrap();
		}
	}
	assert!(app.practice.session().history().is_empty());
	app.handle(key(KeyCode::Esc)).unwrap();
	assert!(app.practice.draft().is_none());
	assert!(app.practice.input().is_empty());
	assert_eq!(app.practice.session().root().render(), "2 + (3 * 4)");
}

#[test]
fn wrapped_logic_expression_mouse_hits_use_display_cells() {
	let mut app: Screen = custom(
		"¬(True ∧ False) ∨ True",
		Language::Logic,
		EvaluationMode::Eager,
	);
	let before: String = app.practice.session().root().render();
	let terminal: Terminal<TestBackend> = draw(&mut app, 10, 12);
	click_text(&mut app, &terminal, "∧");
	assert_eq!(app.practice.draft(), Some(node(&app, "True ∧ False")));
	assert_eq!(app.practice.session().root().render(), before);
	let terminal: Terminal<TestBackend> = draw(&mut app, 10, 12);
	assert!(screen_text(&terminal).contains("____"));
	app.handle(Event::Paste("假".into())).unwrap();
	let terminal: Terminal<TestBackend> = draw(&mut app, 10, 12);
	click_text(&mut app, &terminal, "假");
	assert_eq!(app.practice.input(), "假"); // Clicking the draft again must not erase it.
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert_eq!(app.practice.session().history().len(), 1);
	assert!(app.practice.session().root().render().contains("¬(False)"));
}

#[test]
fn screenshot_groups_are_independent_and_ready_groups_need_only_a_click() {
	let source: &str = "(¬True) ∨ (True ∧ True ∧ (True ∧ True))";
	for token in ["¬", "True ∧ True"] {
		let mut app: Screen = custom(source, Language::Logic, EvaluationMode::Eager);
		let terminal: Terminal<TestBackend> = draw(&mut app, 100, 5);
		assert!(!click_text(&mut app, &terminal, token));
		assert!(
			app.practice.draft().is_some(),
			"{token}: {}",
			feedback(&app)
		);
		assert!(app.practice.session().history().is_empty());
	}
	let mut app: Screen = custom(source, Language::Logic, EvaluationMode::Eager);
	let terminal: Terminal<TestBackend> = draw(&mut app, 100, 5);
	click_text(&mut app, &terminal, "¬");
	app.handle(Event::Paste("False".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert_eq!(
		app.practice.session().render(),
		"(False) ∨ (True ∧ True ∧ (True ∧ True))"
	);
	let terminal: Terminal<TestBackend> = draw(&mut app, 100, 5);
	assert!(click_text(&mut app, &terminal, "False)"));
	assert_eq!(
		app.practice.session().render(),
		"False ∨ (True ∧ True ∧ (True ∧ True))"
	);
	assert!(app.practice.draft().is_none());
	assert_eq!(
		app.practice.session().attempts().last().unwrap().input,
		None
	);
}

#[test]
fn group_clicks_remove_one_pair_and_persist_without_a_typed_answer() {
	let mut app: Screen = custom("((3))", Language::Python, EvaluationMode::ShortCircuit);
	draw(&mut app, 40, 8);
	assert!(!click(&mut app, 0, 0));
	assert!(app.practice.session().history().is_empty());
	assert!(click(&mut app, 1, 0));
	assert_eq!(app.practice.session().render(), "(3)");
	assert!(app.practice.draft().is_none());
	assert!(app.practice.input().is_empty());
	app.practice.record();
	let mut restored: Screen = screen(
		app.practice.course().questions().to_vec(),
		app.practice.progress().clone(),
		0,
		EvaluationMode::ShortCircuit,
	);
	assert_eq!(restored.practice.session().render(), "(3)");
	draw(&mut restored, 40, 8);
	assert!(click(&mut restored, 0, 0));
	assert!(restored.practice.session().is_finished());
	assert!(
		restored
			.practice
			.session()
			.attempts()
			.iter()
			.all(|attempt| attempt.input.is_none())
	);
	assert!(restored.handle(key(KeyCode::Char('u'))).unwrap());
	assert_eq!(restored.practice.session().render(), "(3)");
	assert!(restored.handle(key(KeyCode::Enter)).unwrap());
	assert!(restored.practice.session().is_finished());
}

#[test]
fn long_logic_conjunction_click_is_accepted_before_disjunction_and_implication() {
	let exercise: Exercise = exercises::builtin()
		.unwrap()
		.exercises()
		.find(|exercise| exercise.id == "long-logic")
		.unwrap()
		.clone();
	let mut app: Screen = screen(
		vec![exercise],
		Progress::default(),
		0,
		EvaluationMode::Eager,
	);
	for answer in ["True", "False", "False"] {
		let id: NodeId = app.practice.session().next_step().unwrap().node_id;
		app.practice.select(id);
		app.handle(Event::Paste(answer.into())).unwrap();
		assert!(app.handle(key(KeyCode::Enter)).unwrap());
	}
	assert!(
		app.practice
			.session()
			.render()
			.starts_with("False ∧ False ∨ R → S ↔")
	);
	let terminal: Terminal<TestBackend> = draw(&mut app, 120, 5);
	click_text(&mut app, &terminal, "∨ R");
	assert!(app.practice.draft().is_none());
	assert!(feedback(&app).contains("False ∧ False"));
	let terminal: Terminal<TestBackend> = draw(&mut app, 120, 5);
	click_text(&mut app, &terminal, "∧ False");
	assert_eq!(app.practice.draft(), Some(node(&app, "False ∧ False")));
	app.handle(Event::Paste("False".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert!(
		app.practice
			.session()
			.render()
			.starts_with("False ∨ R → S ↔")
	);
}

#[test]
fn next_generates_questions_and_previous_restores_the_prior_attempts() {
	let exercise: Exercise = exercises::builtin()
		.unwrap()
		.exercises()
		.next()
		.unwrap()
		.clone();
	let mut app: Screen = screen(
		vec![exercise],
		Progress::default(),
		0,
		EvaluationMode::ShortCircuit,
	);
	let first: String = app.practice.session().source().into();
	let id: NodeId = app.practice.session().next_step().unwrap().node_id;
	app.practice.select(id);
	app.handle(Event::Paste("12".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert!(app.handle(key(KeyCode::Char('n'))).unwrap());
	assert!(app.practice.question().id.starts_with("random-v1-python-"));
	let generated: String = app.practice.session().source().into();
	assert_ne!(first, generated);
	assert!(!app.practice.question().bindings.is_empty());
	assert!(app.practice.session().history().is_empty());
	assert!(app.handle(key(KeyCode::Char('p'))).unwrap());
	assert_eq!(app.practice.session().source(), first);
	assert_eq!(app.practice.session().render(), "2 + (12)");
	assert_eq!(app.practice.session().history().len(), 1);
	assert!(app.handle(key(KeyCode::Char('n'))).unwrap());
	assert_eq!(app.practice.session().source(), generated);
	assert!(app.handle(key(KeyCode::Char('r'))).unwrap());
	assert_eq!(app.practice.session().source(), generated);
	assert!(app.handle(key(KeyCode::Char('s'))).unwrap());
	assert_eq!(app.practice.session().source(), generated);
	assert_eq!(app.practice.session().mode(), EvaluationMode::Eager);
}

#[test]
fn native_history_survives_redraw_and_app_starts_below_shell_output() {
	let mut backend: TestBackend = TestBackend::new(60, 14);
	let cells: Vec<ratatui::buffer::Cell> = "shell output"
		.chars()
		.map(|ch| {
			let mut cell: ratatui::buffer::Cell = ratatui::buffer::Cell::default();
			cell.set_char(ch);
			cell
		})
		.collect();
	backend
		.draw(
			cells
				.iter()
				.enumerate()
				.map(|(x, cell)| (x as u16, 0, cell)),
		)
		.unwrap();
	backend.set_cursor_position(Position::new(0, 2)).unwrap();
	let mut terminal: Terminal<TestBackend> = Terminal::with_options(
		backend,
		TerminalOptions {
			viewport: Viewport::Inline(inline::HEIGHT),
		},
	)
	.unwrap();
	let mut app: Screen = app();
	let mut transcript: Transcript = Transcript::default();
	inline::append(&mut terminal, super::text(transcript.sync(&app.practice))).unwrap();
	terminal
		.draw(|frame| {
			render::draw(frame, &mut app);
		})
		.unwrap();
	assert!(screen_text(&terminal).starts_with("shell output"));
	assert!(app.expression_area.y >= 3);
	assert_eq!(app.expression_area.height, inline::HEIGHT);
	click_text(&mut app, &terminal, "*");
	terminal
		.draw(|frame| {
			render::draw(frame, &mut app);
		})
		.unwrap();
	let text: String = screen_text(&terminal);
	assert!(text.contains("2 + (3 * 4)"));
	assert!(text.contains("2 + (____)"));
	assert!(transcript.sync(&app.practice).is_empty());
	app.handle(Event::Paste("12".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	inline::append(&mut terminal, super::text(transcript.sync(&app.practice))).unwrap();
	terminal
		.draw(|frame| {
			render::draw(frame, &mut app);
		})
		.unwrap();
	assert!(transcript.sync(&app.practice).is_empty());
	let text: String = screen_text(&terminal);
	assert_eq!(text.matches("2 + (3 * 4)").count(), 1);
	assert_eq!(text.matches("2 + (12)").count(), 1);
	// The archived source remains above the editable viewport and cannot be clicked.
	let row: usize = text
		.lines()
		.position(|line| line.contains("2 + (3 * 4)"))
		.unwrap();
	click(&mut app, 7, row as u16);
	assert!(app.practice.draft().is_none());
	app.handle(key(KeyCode::Char('u'))).unwrap();
	let marker: Vec<String> = transcript.sync(&app.practice);
	assert!(marker.join("\n").contains("撤销"));
	assert!(transcript.sync(&app.practice).is_empty());
}

#[test]
fn final_negative_result_finishes_without_another_answer_in_live_and_restored_history() {
	let mut app: Screen = custom("-(2 * 5)", Language::Python, EvaluationMode::ShortCircuit);
	let mut terminal: Terminal<TestBackend> = Terminal::with_options(
		TestBackend::new(60, 14),
		TerminalOptions {
			viewport: Viewport::Inline(inline::HEIGHT),
		},
	)
	.unwrap();
	let mut transcript: Transcript = Transcript::default();
	inline::append(&mut terminal, super::text(transcript.sync(&app.practice))).unwrap();
	for (answer, current) in [(Some("10"), "-(10)"), (None, "-10")] {
		let changed: bool = app
			.practice
			.select(app.practice.session().next_step().unwrap().node_id);
		if let Some(answer) = answer {
			assert!(!changed);
			app.handle(Event::Paste(answer.into())).unwrap();
			assert!(app.handle(key(KeyCode::Enter)).unwrap());
		} else {
			assert!(changed);
		}
		assert_eq!(app.practice.session().render(), current);
		let added: Vec<String> = transcript.sync(&app.practice);
		inline::append(&mut terminal, super::text(added)).unwrap();
		terminal
			.draw(|frame| {
				render::draw(frame, &mut app);
			})
			.unwrap();
		assert_eq!(
			screen_text(&terminal)
				.lines()
				.filter(|line| line.trim() == current)
				.count(),
			1
		);
	}
	assert!(app.practice.session().is_finished());
	assert!(app.practice.draft().is_none());
	assert!(app.practice.session().next_step().is_none());
	assert!(!screen_text(&terminal).contains("____"));
	assert_eq!(app.practice.session().history().len(), 2);
	assert_eq!(app.practice.session().attempts().len(), 2);
	app.practice.record();
	let mut restored: Screen = screen(
		app.practice.course().questions().to_vec(),
		app.practice.progress().clone(),
		0,
		EvaluationMode::ShortCircuit,
	);
	let mut transcript: Transcript = Transcript::default();
	let history: Vec<String> = transcript.sync(&restored.practice);
	assert_eq!(history.len(), 3); // Mode header, original source, explicit group.
	assert_eq!(history[1], "-(2 * 5)");
	assert_eq!(history[2], "-(10)");
	assert_eq!(restored.practice.session().render(), "-10");
	assert!(restored.practice.session().is_finished());
	assert!(restored.practice.draft().is_none());
	assert_eq!(restored.practice.session().history().len(), 2);
	assert!(restored.handle(key(KeyCode::Char('u'))).unwrap());
	assert!(!restored.practice.session().is_finished());
	assert!(
		transcript
			.sync(&restored.practice)
			.join("\n")
			.contains("撤销")
	);
	assert_eq!(restored.practice.session().render(), "-(10)");
	assert!(
		restored
			.practice
			.select(restored.practice.session().next_step().unwrap().node_id)
	);
	assert!(restored.practice.session().is_finished());
	assert_eq!(transcript.sync(&restored.practice), ["-(10)"]);
	restored
		.practice
		.select(restored.practice.session().root().id);
	assert!(restored.practice.draft().is_none());
	assert!(feedback(&restored).contains("已结束"));
}

#[test]
fn native_history_reaches_terminal_scrollback() {
	let mut app: Screen = custom(
		"1 + 2 + 3 + 4 + 5 + 6 + 7 + 8",
		Language::Python,
		EvaluationMode::ShortCircuit,
	);
	let mut terminal: Terminal<TestBackend> = Terminal::with_options(
		TestBackend::new(60, 7),
		TerminalOptions {
			viewport: Viewport::Inline(inline::HEIGHT),
		},
	)
	.unwrap();
	let mut transcript: Transcript = Transcript::default();
	inline::append(&mut terminal, super::text(transcript.sync(&app.practice))).unwrap();
	for answer in ["3", "6", "10", "15", "21", "28", "36"] {
		app.practice
			.select(app.practice.session().next_step().unwrap().node_id);
		app.handle(Event::Paste(answer.into())).unwrap();
		assert!(app.handle(key(KeyCode::Enter)).unwrap());
		inline::append(&mut terminal, super::text(transcript.sync(&app.practice))).unwrap();
		terminal
			.draw(|frame| {
				render::draw(frame, &mut app);
			})
			.unwrap();
	}
	let scrollback: String = terminal
		.backend()
		.scrollback()
		.content
		.iter()
		.map(|cell| cell.symbol())
		.collect();
	assert!(scrollback.contains("1 + 2 + 3"));
	assert!(screen_text(&terminal).contains("36"));
	assert!(app.practice.session().is_finished());
}

/// The app layer reports reasons; these are the words this terminal has always shown for
/// them. `say` matches [`Notice`] exhaustively, so a new reason cannot reach a front end
/// unworded — this pins what the worded ones say.
#[test]
fn every_reported_reason_keeps_the_sentence_this_terminal_showed_before() {
	for (notice, sentence) in [
		(Notice::Start, "点击一处 → ____ → 填值 → Enter。"),
		(Notice::DraftOpen, "在 ____ 处填值，Enter 检查；Esc 取消。"),
		(
			Notice::FinalPair,
			"只剩两个值，直接填入本步结果，Enter 检查。",
		),
		(
			Notice::NoNextStep,
			"本题已结束。可以按 n 进入下一题，或按 u 撤销。",
		),
		(Notice::Undone, "已撤销上一步。"),
		(Notice::Restarted, "已重新开始本题。"),
		(
			Notice::ModeSwitched(EvaluationMode::ShortCircuit),
			"已切换：短路开启。进度分别保存。",
		),
		(
			Notice::ModeSwitched(EvaluationMode::Eager),
			"已切换：短路关闭 · 全部求值。进度分别保存。",
		),
		(Notice::CourseEnded, "已经是本题集的最后一题。"),
		(
			Notice::ProofStart,
			"下一行：公式 ; 规则 ; 引用行。Enter 检查，F1 查看规则。",
		),
		(
			Notice::ProofUndone,
			"已撤销上一行，并恢复对应的假设作用域。",
		),
	] {
		let (shown, good): (String, bool) = say::feedback(&Report::Notice(notice));
		assert_eq!(shown, sentence);
		assert!(!good, "a reason is never good news: {notice:?}");
	}
	// A sentence the rules or this front end already worded passes through unchanged.
	assert_eq!(
		say::feedback(&Report::Taught {
			message: "正确。".into(),
			accepted: true,
		}),
		("正确。".into(), true)
	);
	assert_eq!(
		say::feedback(&Report::Note(super::keys::HELP.into())),
		(super::keys::HELP.into(), false)
	);
}

#[test]
fn feedback_scrolls_in_place_without_a_panel() {
	let mut app: Screen = app();
	app.handle(key(KeyCode::Char('?'))).unwrap();
	draw(&mut app, 20, 6);
	assert!(app.view_scroll.maximum > 0);
	while app.view_scroll.offset < app.view_scroll.maximum {
		app.handle(key(KeyCode::PageDown)).unwrap();
	}
	assert!(screen_text(&draw(&mut app, 20, 6)).contains("退出"));
}
