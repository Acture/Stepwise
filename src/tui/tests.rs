use crossterm::event::{
	Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{
	Terminal, TerminalOptions, Viewport,
	backend::{Backend, TestBackend},
	layout::Position,
	text::Text,
};
use unicode_width::UnicodeWidthStr;

use super::{App, inline, render, transcript::Transcript};
use crate::{
	core::{EvaluationMode, NodeId},
	exercises::{self, Exercise, Language},
	progress::Progress,
};

fn app() -> App {
	App::new(
		exercises::builtin().unwrap(),
		Progress::default(),
		0,
		EvaluationMode::ShortCircuit,
	)
	.unwrap()
}
fn custom(source: &str, language: Language, mode: EvaluationMode) -> App {
	App::new(
		vec![Exercise {
			id: "test".into(),
			title: "test".into(),
			expression: source.into(),
			goal: String::new(),
			language,
			bindings: Default::default(),
		}],
		Progress::default(),
		0,
		mode,
	)
	.unwrap()
}
fn key(code: KeyCode) -> Event {
	Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}
fn screen(terminal: &Terminal<TestBackend>) -> String {
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
fn draw(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
	let mut terminal: Terminal<TestBackend> =
		Terminal::new(TestBackend::new(width, height)).unwrap();
	terminal
		.draw(|frame| {
			render::draw(frame, app);
		})
		.unwrap();
	terminal
}
fn click(app: &mut App, column: u16, row: u16) -> bool {
	app.handle(Event::Mouse(MouseEvent {
		kind: MouseEventKind::Down(MouseButton::Left),
		column,
		row,
		modifiers: KeyModifiers::NONE,
	}))
	.unwrap()
}
/// Use actual displayed character coordinates, including Unicode cell widths.
fn click_text(app: &mut App, terminal: &Terminal<TestBackend>, token: &str) -> bool {
	let text: String = screen(terminal);
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
fn node(app: &App, source: &str) -> NodeId {
	app.session
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
	let mut app: App = app();
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	assert!(screen(&terminal).contains("2 + (3 * 4)"));
	click_text(&mut app, &terminal, "*");
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	assert!(screen(&terminal).contains("2 + (____)"));
	assert!(!screen(&terminal).contains("12"));
	assert!(screen(&terminal).contains("2 + (3 * 4)"));
	assert_eq!(app.session.root().render(), "2 + (3 * 4)");
	assert!(app.session.history().is_empty());
	assert!(app.session.attempts().is_empty());
	app.handle(Event::Paste("13".into())).unwrap();
	assert!(!app.handle(key(KeyCode::Enter)).unwrap());
	assert!(app.session.history().is_empty());
	assert_eq!(app.input, "13");
	assert!(app.editing.is_some());
	app.handle(key(KeyCode::Backspace)).unwrap();
	app.handle(key(KeyCode::Char('2'))).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert_eq!(app.session.root().render(), "2 + (12)");
	assert_eq!(app.session.history().len(), 1);
	assert!(app.editing.is_none());
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	assert!(screen(&terminal).contains("2 + (12)"));
	assert_eq!(app.session.render(), "2 + (12)");
	click_text(&mut app, &terminal, "+");
	assert!(app.editing.is_none());
	assert!(app.feedback.contains("不能跳过"));
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	assert!(click_text(&mut app, &terminal, "("));
	assert_eq!(app.session.attempts().last().unwrap().input, None);
	assert_eq!(app.session.render(), "2 + 12");
	assert_eq!(app.editing, Some(app.session.root().id));
	assert!(app.input.is_empty());
	let text: String = screen(&draw(&mut app, 40, 8));
	assert!(text.contains("2 + 12"));
	assert!(text.contains("____"));
	app.handle(Event::Paste("14".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert!(app.session.is_finished());
	assert_eq!(app.session.history().len(), 3);
	let text: String = screen(&draw(&mut app, 40, 8));
	assert!(text.contains("完成"));
}

#[test]
fn clicking_a_repeated_variable_blanks_and_substitutes_every_occurrence() {
	let exercise: Exercise = Exercise {
		id: "repeated-variable".into(),
		title: "repeated variable".into(),
		expression: "x + y * x".into(),
		goal: String::new(),
		language: Language::Python,
		bindings: std::collections::BTreeMap::from([
			("x".into(), "2".into()),
			("y".into(), "3".into()),
		]),
	};
	let mut app: App = App::new(
		vec![exercise],
		Progress::default(),
		0,
		EvaluationMode::ShortCircuit,
	)
	.unwrap();
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	click_text(&mut app, &terminal, "x"); // Click the rightmost occurrence before y.
	let text: String = screen(&draw(&mut app, 40, 8));
	assert!(text.contains("x + y * x"));
	assert!(text.contains("____ + y * ____"));
	app.handle(Event::Paste("2".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert_eq!(app.session.render(), "2 + y * 2");
	assert_eq!(app.session.attempts().len(), 1);
	assert!(app.handle(key(KeyCode::Char('u'))).unwrap());
	assert_eq!(app.session.render(), "x + y * x");
}

#[test]
fn final_pair_auto_blank_survives_resume_undo_and_reset_without_auto_solving() {
	let exercise: Exercise = Exercise {
		id: "variables".into(),
		title: "variables".into(),
		expression: "x + 3".into(),
		goal: String::new(),
		language: Language::Python,
		bindings: std::collections::BTreeMap::from([("x".into(), "2".into())]),
	};
	let mut app: App = App::new(
		vec![exercise.clone()],
		Progress::default(),
		0,
		EvaluationMode::ShortCircuit,
	)
	.unwrap();
	assert!(app.editing.is_none());
	let mut transcript: Transcript = Transcript::default();
	assert!(transcript.sync(&app).to_string().contains("x=2"));
	let terminal: Terminal<TestBackend> = draw(&mut app, 30, 8);
	click_text(&mut app, &terminal, "x");
	let text: String = screen(&draw(&mut app, 30, 8));
	assert!(text.contains("x + 3"));
	assert!(text.contains("____ + 3"));
	app.handle(Event::Paste("2".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert_eq!(app.editing, Some(app.session.root().id));
	assert_eq!(app.session.render(), "2 + 3");
	assert_eq!(app.session.history().len(), 1);
	app.record();
	let mut app: App = App::new(
		vec![exercise],
		app.progress,
		0,
		EvaluationMode::ShortCircuit,
	)
	.unwrap();
	assert_eq!(app.editing, Some(app.session.root().id));
	assert!(app.input.is_empty());
	app.handle(Event::Paste("6".into())).unwrap();
	assert!(!app.handle(key(KeyCode::Enter)).unwrap());
	assert_eq!(app.session.history().len(), 1);
	app.handle(key(KeyCode::Backspace)).unwrap();
	app.handle(Event::Paste("5".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert!(app.session.is_finished());
	assert!(app.handle(key(KeyCode::Char('u'))).unwrap());
	assert_eq!(app.editing, Some(app.session.root().id));
	let text: String = screen(&draw(&mut app, 30, 8));
	assert!(text.contains("2 + 3"));
	assert!(text.contains("____"));
	app.handle(key(KeyCode::Esc)).unwrap();
	assert!(app.editing.is_none());
	assert!(app.handle(key(KeyCode::Char('r'))).unwrap());
	assert_eq!(app.session.render(), "x + 3");
	assert!(app.editing.is_none());
	let app: App = custom("2 + 3", Language::Python, EvaluationMode::ShortCircuit);
	assert_eq!(app.editing, Some(app.session.root().id));
	assert!(app.session.history().is_empty());
}

#[test]
fn incorrect_selection_does_not_blank_or_advance_and_short_circuit_is_selectable() {
	let mut app: App = custom(
		"(2 + 3) * (4 + 5)",
		Language::Python,
		EvaluationMode::ShortCircuit,
	);
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	click_text(&mut app, &terminal, "*");
	assert!(app.editing.is_none());
	assert!(app.feedback.contains("不能跳过"));
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	click_text(&mut app, &terminal, "4 + 5");
	assert_eq!(app.editing, Some(node(&app, "4 + 5")));
	assert!(app.session.history().is_empty());
	app.handle(Event::Paste("9".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert_eq!(app.session.render(), "(2 + 3) * (9)");

	let mut app: App = custom(
		"False and (3 / 0 > 1)",
		Language::Python,
		EvaluationMode::ShortCircuit,
	);
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	click_text(&mut app, &terminal, "/");
	assert!(app.editing.is_none());
	assert!(app.feedback.contains("跳过"));
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	click_text(&mut app, &terminal, "and");
	assert_eq!(app.editing, Some(app.session.root().id));
	assert_eq!(app.session.root().render(), "False and ((3 / 0) > 1)");
	app.handle(Event::Paste("0".into())).unwrap();
	assert!(!app.handle(key(KeyCode::Enter)).unwrap());
	assert!(app.session.history().is_empty());
	app.handle(key(KeyCode::Esc)).unwrap();
	app.handle(key(KeyCode::Char('s'))).unwrap();
	let terminal: Terminal<TestBackend> = draw(&mut app, 40, 8);
	click_text(&mut app, &terminal, "/");
	assert_eq!(app.editing, Some(node(&app, "3 / 0")));
	app.handle(Event::Paste("ZeroDivisionError".into()))
		.unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert!(app.session.terminal_error().is_some());
}

#[test]
fn keyboard_submission_undo_and_strategy_toggle_preserve_progress() {
	let mut app: App = app();
	app.handle(key(KeyCode::Down)).unwrap();
	app.handle(key(KeyCode::Down)).unwrap();
	app.handle(key(KeyCode::Enter)).unwrap();
	assert_eq!(app.editing, Some(node(&app, "3 * 4")));
	app.handle(Event::Paste("12".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert_eq!(app.session.root().render(), "2 + (12)");
	assert!(app.handle(key(KeyCode::Char('s'))).unwrap());
	assert_eq!(app.session.mode(), EvaluationMode::Eager);
	assert!(app.session.history().is_empty());
	assert!(app.handle(key(KeyCode::Char('s'))).unwrap());
	assert_eq!(app.session.root().render(), "2 + (12)");
	assert!(app.handle(key(KeyCode::Char('u'))).unwrap());
	assert_eq!(app.session.root().render(), "2 + (3 * 4)");
	assert!(app.session.history().is_empty());
}

#[test]
fn inline_unicode_draft_survives_tiny_resizes_and_tab_does_nothing() {
	let mut app: App = app();
	app.handle(key(KeyCode::Down)).unwrap();
	app.handle(key(KeyCode::Down)).unwrap();
	app.handle(key(KeyCode::Enter)).unwrap();
	let editing: Option<NodeId> = app.editing;
	app.handle(Event::Paste("中文\nFalse".into())).unwrap();
	assert_eq!(app.input, "中文False");
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
			assert!(!screen(&terminal).contains("扩大"));
			if width > 0 && height > 0 {
				let cursor: ratatui::layout::Position = terminal.get_cursor_position().unwrap();
				assert!(cursor.x < width && cursor.y < height);
			}
			assert_eq!(app.input, "中文False");
			assert_eq!(app.editing, editing);
			app.handle(key(KeyCode::Tab)).unwrap();
		}
	}
	assert!(app.session.history().is_empty());
	app.handle(key(KeyCode::Esc)).unwrap();
	assert!(app.editing.is_none());
	assert!(app.input.is_empty());
	assert_eq!(app.session.root().render(), "2 + (3 * 4)");
}

#[test]
fn wrapped_logic_expression_mouse_hits_use_display_cells() {
	let mut app: App = custom(
		"¬(True ∧ False) ∨ True",
		Language::Logic,
		EvaluationMode::Eager,
	);
	let before: String = app.session.root().render();
	let terminal: Terminal<TestBackend> = draw(&mut app, 10, 12);
	click_text(&mut app, &terminal, "∧");
	assert_eq!(app.editing, Some(node(&app, "True ∧ False")));
	assert_eq!(app.session.root().render(), before);
	let terminal: Terminal<TestBackend> = draw(&mut app, 10, 12);
	assert!(screen(&terminal).contains("____"));
	app.handle(Event::Paste("假".into())).unwrap();
	let terminal: Terminal<TestBackend> = draw(&mut app, 10, 12);
	click_text(&mut app, &terminal, "假");
	assert_eq!(app.input, "假"); // Clicking the draft again must not erase it.
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert_eq!(app.session.history().len(), 1);
	assert!(app.session.root().render().contains("¬(False)"));
}

#[test]
fn screenshot_groups_are_independent_and_ready_groups_need_only_a_click() {
	let source: &str = "(¬True) ∨ (True ∧ True ∧ (True ∧ True))";
	for token in ["¬", "True ∧ True"] {
		let mut app: App = custom(source, Language::Logic, EvaluationMode::Eager);
		let terminal: Terminal<TestBackend> = draw(&mut app, 100, 5);
		assert!(!click_text(&mut app, &terminal, token));
		assert!(app.editing.is_some(), "{token}: {}", app.feedback);
		assert!(app.session.history().is_empty());
	}
	let mut app: App = custom(source, Language::Logic, EvaluationMode::Eager);
	let terminal: Terminal<TestBackend> = draw(&mut app, 100, 5);
	click_text(&mut app, &terminal, "¬");
	app.handle(Event::Paste("False".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert_eq!(
		app.session.render(),
		"(False) ∨ (True ∧ True ∧ (True ∧ True))"
	);
	let terminal: Terminal<TestBackend> = draw(&mut app, 100, 5);
	assert!(click_text(&mut app, &terminal, "False)"));
	assert_eq!(
		app.session.render(),
		"False ∨ (True ∧ True ∧ (True ∧ True))"
	);
	assert!(app.editing.is_none());
	assert_eq!(app.session.attempts().last().unwrap().input, None);
}

#[test]
fn group_clicks_remove_one_pair_and_persist_without_a_typed_answer() {
	let mut app: App = custom("((3))", Language::Python, EvaluationMode::ShortCircuit);
	draw(&mut app, 40, 8);
	assert!(!click(&mut app, 0, 0));
	assert!(app.session.history().is_empty());
	assert!(click(&mut app, 1, 0));
	assert_eq!(app.session.render(), "(3)");
	assert!(app.editing.is_none());
	assert!(app.input.is_empty());
	app.record();
	let mut restored: App =
		App::new(app.exercises, app.progress, 0, EvaluationMode::ShortCircuit).unwrap();
	assert_eq!(restored.session.render(), "(3)");
	draw(&mut restored, 40, 8);
	assert!(click(&mut restored, 0, 0));
	assert!(restored.session.is_finished());
	assert!(
		restored
			.session
			.attempts()
			.iter()
			.all(|attempt| attempt.input.is_none())
	);
	assert!(restored.handle(key(KeyCode::Char('u'))).unwrap());
	assert_eq!(restored.session.render(), "(3)");
	assert!(restored.handle(key(KeyCode::Enter)).unwrap());
	assert!(restored.session.is_finished());
}

#[test]
fn long_logic_conjunction_click_is_accepted_before_disjunction_and_implication() {
	let exercise: Exercise = exercises::builtin()
		.unwrap()
		.into_iter()
		.find(|exercise| exercise.id == "long-logic")
		.unwrap();
	let mut app: App = App::new(
		vec![exercise],
		Progress::default(),
		0,
		EvaluationMode::Eager,
	)
	.unwrap();
	for answer in ["True", "False", "False"] {
		let id: NodeId = app.session.next_step().unwrap().node_id;
		app.begin_edit(id);
		app.handle(Event::Paste(answer.into())).unwrap();
		assert!(app.handle(key(KeyCode::Enter)).unwrap());
	}
	assert!(app.session.render().starts_with("False ∧ False ∨ R → S ↔"));
	let terminal: Terminal<TestBackend> = draw(&mut app, 120, 5);
	click_text(&mut app, &terminal, "∨ R");
	assert!(app.editing.is_none());
	assert!(app.feedback.contains("False ∧ False"));
	let terminal: Terminal<TestBackend> = draw(&mut app, 120, 5);
	click_text(&mut app, &terminal, "∧ False");
	assert_eq!(app.editing, Some(node(&app, "False ∧ False")));
	app.handle(Event::Paste("False".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert!(app.session.render().starts_with("False ∨ R → S ↔"));
}

#[test]
fn next_generates_questions_and_previous_restores_the_prior_attempts() {
	let exercise: Exercise = exercises::builtin().unwrap().remove(0);
	let mut app: App = App::new(
		vec![exercise],
		Progress::default(),
		0,
		EvaluationMode::ShortCircuit,
	)
	.unwrap();
	let first: String = app.session.source().into();
	let id: NodeId = app.session.next_step().unwrap().node_id;
	app.begin_edit(id);
	app.handle(Event::Paste("12".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	assert!(app.handle(key(KeyCode::Char('n'))).unwrap());
	assert!(app.exercises[app.index].id.starts_with("random-v1-python-"));
	let generated: String = app.session.source().into();
	assert_ne!(first, generated);
	assert!(!app.exercises[app.index].bindings.is_empty());
	assert!(app.session.history().is_empty());
	assert!(app.handle(key(KeyCode::Char('p'))).unwrap());
	assert_eq!(app.session.source(), first);
	assert_eq!(app.session.render(), "2 + (12)");
	assert_eq!(app.session.history().len(), 1);
	assert!(app.handle(key(KeyCode::Char('n'))).unwrap());
	assert_eq!(app.session.source(), generated);
	assert!(app.handle(key(KeyCode::Char('r'))).unwrap());
	assert_eq!(app.session.source(), generated);
	assert!(app.handle(key(KeyCode::Char('s'))).unwrap());
	assert_eq!(app.session.source(), generated);
	assert_eq!(app.session.mode(), EvaluationMode::Eager);
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
	let mut app: App = app();
	let mut transcript: Transcript = Transcript::default();
	inline::append(&mut terminal, transcript.sync(&app)).unwrap();
	terminal
		.draw(|frame| {
			render::draw(frame, &mut app);
		})
		.unwrap();
	assert!(screen(&terminal).starts_with("shell output"));
	assert!(app.expression_area.y >= 3);
	assert_eq!(app.expression_area.height, inline::HEIGHT);
	click_text(&mut app, &terminal, "*");
	terminal
		.draw(|frame| {
			render::draw(frame, &mut app);
		})
		.unwrap();
	let text: String = screen(&terminal);
	assert!(text.contains("2 + (3 * 4)"));
	assert!(text.contains("2 + (____)"));
	assert!(transcript.sync(&app).lines.is_empty());
	app.handle(Event::Paste("12".into())).unwrap();
	assert!(app.handle(key(KeyCode::Enter)).unwrap());
	inline::append(&mut terminal, transcript.sync(&app)).unwrap();
	terminal
		.draw(|frame| {
			render::draw(frame, &mut app);
		})
		.unwrap();
	assert!(transcript.sync(&app).lines.is_empty());
	let text: String = screen(&terminal);
	assert_eq!(text.matches("2 + (3 * 4)").count(), 1);
	assert_eq!(text.matches("2 + (12)").count(), 1);
	// The archived source remains above the editable viewport and cannot be clicked.
	let row: usize = text
		.lines()
		.position(|line| line.contains("2 + (3 * 4)"))
		.unwrap();
	click(&mut app, 7, row as u16);
	assert!(app.editing.is_none());
	app.handle(key(KeyCode::Char('u'))).unwrap();
	let marker: Text<'static> = transcript.sync(&app);
	assert!(marker.to_string().contains("撤销"));
	assert!(transcript.sync(&app).lines.is_empty());
}

#[test]
fn final_negative_result_finishes_without_another_answer_in_live_and_restored_history() {
	let mut app: App = custom("-(2 * 5)", Language::Python, EvaluationMode::ShortCircuit);
	let mut terminal: Terminal<TestBackend> = Terminal::with_options(
		TestBackend::new(60, 14),
		TerminalOptions {
			viewport: Viewport::Inline(inline::HEIGHT),
		},
	)
	.unwrap();
	let mut transcript: Transcript = Transcript::default();
	inline::append(&mut terminal, transcript.sync(&app)).unwrap();
	for (answer, current) in [(Some("10"), "-(10)"), (None, "-10")] {
		let changed: bool = app.begin_edit(app.session.next_step().unwrap().node_id);
		if let Some(answer) = answer {
			assert!(!changed);
			app.handle(Event::Paste(answer.into())).unwrap();
			assert!(app.handle(key(KeyCode::Enter)).unwrap());
		} else {
			assert!(changed);
		}
		assert_eq!(app.session.render(), current);
		let added: Text<'static> = transcript.sync(&app);
		inline::append(&mut terminal, added).unwrap();
		terminal
			.draw(|frame| {
				render::draw(frame, &mut app);
			})
			.unwrap();
		assert_eq!(
			screen(&terminal)
				.lines()
				.filter(|line| line.trim() == current)
				.count(),
			1
		);
	}
	assert!(app.session.is_finished());
	assert!(app.editing.is_none());
	assert!(app.session.next_step().is_none());
	assert!(!screen(&terminal).contains("____"));
	assert_eq!(app.session.history().len(), 2);
	assert_eq!(app.session.attempts().len(), 2);
	app.record();
	let mut restored: App = App::new(
		app.exercises.clone(),
		app.progress.clone(),
		0,
		EvaluationMode::ShortCircuit,
	)
	.unwrap();
	let mut transcript: Transcript = Transcript::default();
	let history: Text<'static> = transcript.sync(&restored);
	assert_eq!(history.lines.len(), 3); // Mode header, original source, explicit group.
	assert_eq!(history.lines[1].to_string(), "-(2 * 5)");
	assert_eq!(history.lines[2].to_string(), "-(10)");
	assert_eq!(restored.session.render(), "-10");
	assert!(restored.session.is_finished());
	assert!(restored.editing.is_none());
	assert_eq!(restored.session.history().len(), 2);
	assert!(restored.handle(key(KeyCode::Char('u'))).unwrap());
	assert!(!restored.session.is_finished());
	assert!(transcript.sync(&restored).to_string().contains("撤销"));
	assert_eq!(restored.session.render(), "-(10)");
	assert!(restored.begin_edit(restored.session.next_step().unwrap().node_id));
	assert!(restored.session.is_finished());
	assert_eq!(transcript.sync(&restored).to_string(), "-(10)");
	restored.begin_edit(restored.session.root().id);
	assert!(restored.editing.is_none());
	assert!(restored.feedback.contains("已结束"));
}

#[test]
fn native_history_reaches_terminal_scrollback() {
	let mut app: App = custom(
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
	inline::append(&mut terminal, transcript.sync(&app)).unwrap();
	for answer in ["3", "6", "10", "15", "21", "28", "36"] {
		app.begin_edit(app.session.next_step().unwrap().node_id);
		app.handle(Event::Paste(answer.into())).unwrap();
		assert!(app.handle(key(KeyCode::Enter)).unwrap());
		inline::append(&mut terminal, transcript.sync(&app)).unwrap();
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
	assert!(screen(&terminal).contains("36"));
	assert!(app.session.is_finished());
}

#[test]
fn feedback_scrolls_in_place_without_a_panel() {
	let mut app: App = app();
	app.handle(key(KeyCode::Char('?'))).unwrap();
	draw(&mut app, 20, 6);
	assert!(app.view_scroll.maximum > 0);
	while app.view_scroll.offset < app.view_scroll.maximum {
		app.handle(key(KeyCode::PageDown)).unwrap();
	}
	assert!(screen(&draw(&mut app, 20, 6)).contains("退出"));
}
