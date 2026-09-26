//! The proof half of the terminal adapter: the keys a proof binds and how it is drawn. Every
//! character a student types belongs to the proof line, so changing question takes a control
//! key here where the expression view uses a letter; either way it is the lesson that moves.

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use ratatui::{
	Frame,
	layout::{Position, Rect},
	style::{Color, Style},
	text::Text,
	widgets::Paragraph,
};

use super::{keys::Screen, say, viewport};
use crate::{core::ParseError, logic::proof::RULES};

impl Screen {
	pub(super) fn proof_event(&mut self, event: Event) -> Result<bool, ParseError> {
		match event {
			Event::Key(key) if key.kind != KeyEventKind::Release => {
				if key.modifiers.contains(KeyModifiers::CONTROL) {
					match key.code {
						KeyCode::Char('c' | 'q') => self.quit = true,
						KeyCode::Char('z') if self.proof_mut().undo() => {
							self.proof_scroll.reset();
							return Ok(true);
						}
						KeyCode::Char('u') => self.proof_mut().clear_input(),
						KeyCode::Char('n') => return self.change_question(true),
						KeyCode::Char('p') => return self.change_question(false),
						_ => {}
					}
					return Ok(false);
				}
				match key.code {
					KeyCode::Enter => {
						self.proof_scroll.reset();
						return Ok(self.proof_mut().submit());
					}
					KeyCode::Backspace => self.proof_mut().backspace(),
					KeyCode::Esc => self.proof_mut().clear_input(),
					KeyCode::F(1) => {
						self.proof_mut().note(RULES);
						self.proof_scroll.reset();
					}
					KeyCode::Up | KeyCode::PageUp => self.proof_scroll.up(),
					KeyCode::Down | KeyCode::PageDown => self.proof_scroll.down(),
					KeyCode::Char(character) if !key.modifiers.contains(KeyModifiers::ALT) => {
						self.proof_mut().type_character(character);
					}
					_ => {}
				}
			}
			Event::Paste(text) => self.proof_mut().paste(&text),
			Event::Mouse(mouse) => match mouse.kind {
				MouseEventKind::ScrollUp => self.proof_scroll.up(),
				MouseEventKind::ScrollDown => self.proof_scroll.down(),
				_ => {}
			},
			_ => {}
		}
		Ok(false)
	}

	pub(super) fn draw_proof(&mut self, frame: &mut Frame<'_>) -> Option<Position> {
		let [header, body, input, footer]: [Rect; 4] = viewport::compact_rows(frame.area(), true);
		frame.render_widget(
			Paragraph::new(format!(
				"目标：{}{}",
				self.proof().proof().goal,
				if self.proof().is_finished() {
					" · 完成"
				} else {
					""
				}
			))
			.style(Style::default().fg(Color::Cyan)),
			header,
		);
		let (feedback, good): (String, bool) = say::feedback(self.proof().report());
		let text: Text<'_> = Text::raw(feedback).style(Style::default().fg(if good {
			Color::Green
		} else {
			Color::Yellow
		}));
		viewport::draw_text(frame, body, text, &mut self.proof_scroll);
		let cursor: Option<Position> = viewport::draw_input(frame, input, self.proof().input());
		frame.render_widget(
			Paragraph::new("Enter检查 F1规则 Ctrl+Z撤销 Ctrl+N/P换题 Ctrl+C退出")
				.style(Style::default().fg(Color::DarkGray)),
			footer,
		);
		cursor
	}
}

#[cfg(test)]
mod tests {
	use crossterm::event::KeyEvent;
	use ratatui::{Terminal, backend::TestBackend};

	use super::*;
	use crate::{
		app::{Course, Lesson},
		core::EvaluationMode,
		exercises::{ProofQuestion, Question},
		progress::Progress,
	};

	fn screen(premises: &[&str], goal: &str) -> Screen {
		let question: ProofQuestion = ProofQuestion {
			set: String::new(),
			name: "test".into(),
			title: "test".into(),
			premises: premises.iter().map(|premise| (*premise).into()).collect(),
			conclusion: goal.into(),
			note: None,
		};
		Screen::new(
			Lesson::new(
				Course::random(vec![Question::Proof(question)], 0).unwrap(),
				Progress::default(),
				EvaluationMode::Eager,
			)
			.unwrap(),
		)
	}

	#[test]
	fn rules_scroll_without_switching_views_and_drafts_survive_resizing() {
		let mut app: Screen = screen(&["P", "P -> Q"], "Q");
		app.handle(Event::Paste("Q ; mp ; 1,2".into())).unwrap();
		for (width, height) in [(40, 10), (20, 6), (1, 1), (0, 0), (80, 24)] {
			let mut terminal: Terminal<TestBackend> =
				Terminal::new(TestBackend::new(width, height)).unwrap();
			terminal
				.draw(|frame| {
					app.draw_proof(frame);
				})
				.unwrap();
			assert_eq!(app.proof().input(), "Q ; mp ; 1,2");
		}
		let mut terminal: Terminal<TestBackend> = Terminal::new(TestBackend::new(30, 8)).unwrap();
		app.handle(Event::Key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)))
			.unwrap();
		terminal
			.draw(|frame| {
				app.draw_proof(frame);
			})
			.unwrap();
		assert!(app.proof_scroll.maximum > 0);
		while app.proof_scroll.offset < app.proof_scroll.maximum {
			app.handle(Event::Key(KeyEvent::new(
				KeyCode::PageDown,
				KeyModifiers::NONE,
			)))
			.unwrap();
		}
		terminal
			.draw(|frame| {
				app.draw_proof(frame);
			})
			.unwrap();
		let text: String = terminal
			.backend()
			.buffer()
			.content
			.iter()
			.filter(|cell| cell.symbol() != " ")
			.map(|cell| cell.symbol())
			.collect();
		assert!(text.contains("内部的行"));
		app.handle(Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)))
			.unwrap();
		assert_eq!(app.proof().input(), "Q ; mp ; 1,2");
		assert!(
			app.handle(Event::Key(KeyEvent::new(
				KeyCode::Enter,
				KeyModifiers::NONE
			)))
			.unwrap()
		);
		assert!(app.proof().is_finished());
	}
}
