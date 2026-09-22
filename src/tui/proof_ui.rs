use std::{
	io::{self, IsTerminal},
	path::PathBuf,
};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use ratatui::{
	Frame,
	layout::{Position, Rect},
	style::{Color, Style},
	text::Text,
	widgets::Paragraph,
};

use super::{
	inline::{InlineTerminal, TerminalGuard},
	viewport::{self, Scroll},
};
use crate::{
	app::ProofPractice,
	logic::proof::{Proof, RULES},
	progress::Progress,
};

/// The terminal around one proof: how the rule reference scrolls and whether this run is
/// ending. Every proof step goes through the app layer.
struct ProofScreen {
	practice: ProofPractice,
	scroll: Scroll,
	quit: bool,
}

impl ProofScreen {
	fn new(practice: ProofPractice) -> Self {
		Self {
			practice,
			scroll: Scroll::default(),
			quit: false,
		}
	}

	fn handle(&mut self, event: Event) -> bool {
		match event {
			Event::Key(key) if key.kind != KeyEventKind::Release => {
				if key.modifiers.contains(KeyModifiers::CONTROL) {
					match key.code {
						KeyCode::Char('c' | 'q') => self.quit = true,
						KeyCode::Char('z') if self.practice.undo() => {
							self.scroll.reset();
							return true;
						}
						KeyCode::Char('u') => self.practice.clear_input(),
						_ => {}
					}
					return false;
				}
				match key.code {
					KeyCode::Enter => {
						self.scroll.reset();
						return self.practice.submit();
					}
					KeyCode::Backspace => self.practice.backspace(),
					KeyCode::Esc => self.practice.clear_input(),
					KeyCode::F(1) => {
						self.practice.note(RULES);
						self.scroll.reset();
					}
					KeyCode::Up | KeyCode::PageUp => self.scroll.up(),
					KeyCode::Down | KeyCode::PageDown => self.scroll.down(),
					KeyCode::Char(character) if !key.modifiers.contains(KeyModifiers::ALT) => {
						self.practice.type_character(character);
					}
					_ => {}
				}
			}
			Event::Paste(text) => self.practice.paste(&text),
			Event::Mouse(mouse) => match mouse.kind {
				MouseEventKind::ScrollUp => self.scroll.up(),
				MouseEventKind::ScrollDown => self.scroll.down(),
				_ => {}
			},
			_ => {}
		}
		false
	}

	fn draw(&mut self, frame: &mut Frame<'_>) -> Option<Position> {
		let [header, body, input, footer]: [Rect; 4] = viewport::compact_rows(frame.area(), true);
		frame.render_widget(
			Paragraph::new(format!(
				"目标：{}{}",
				self.practice.proof().goal,
				if self.practice.is_finished() {
					" · 完成"
				} else {
					""
				}
			))
			.style(Style::default().fg(Color::Cyan)),
			header,
		);
		let text: Text<'_> = Text::raw(self.practice.feedback().to_owned()).style(
			Style::default().fg(if self.practice.feedback_good() {
				Color::Green
			} else {
				Color::Yellow
			}),
		);
		viewport::draw_text(frame, body, text, &mut self.scroll);
		let cursor: Option<Position> = viewport::draw_input(frame, input, self.practice.input());
		frame.render_widget(
			Paragraph::new("Enter检查 F1规则 Ctrl+Z撤销 Ctrl+C退出")
				.style(Style::default().fg(Color::DarkGray)),
			footer,
		);
		cursor
	}
}

pub fn run_proof(
	proof: Proof,
	progress: Progress,
	path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
	if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
		return Err("自然演绎练习需要交互终端；使用 --check-proof 检查证明文件。".into());
	}
	let mut screen: ProofScreen = ProofScreen::new(ProofPractice::new(proof, progress)?);
	let _guard: TerminalGuard = TerminalGuard::enter()?;
	let mut terminal: InlineTerminal = InlineTerminal::new()?;
	terminal.append(super::text(screen.practice.opening()))?;
	loop {
		terminal.draw(|frame| screen.draw(frame))?;
		let before: usize = screen.practice.proof().lines().len();
		let changed: bool = screen.handle(event::read()?);
		if changed {
			terminal.append(super::text(screen.practice.appended(before)))?;
		}
		if changed || screen.quit {
			screen.practice.record();
			if let Some(path) = &path {
				screen.practice.progress().save(path)?;
			}
		}
		if screen.quit {
			terminal.finish(Text::raw(if screen.practice.is_finished() {
				"证明完成。"
			} else {
				"本次练习结束。"
			}))?;
			return Ok(());
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::logic::parse_formula;
	use crossterm::event::KeyEvent;
	use ratatui::{Terminal, backend::TestBackend};

	fn screen(premises: &[&str], goal: &str) -> ProofScreen {
		ProofScreen::new(
			ProofPractice::new(
				Proof::new(
					premises
						.iter()
						.map(|source| parse_formula(source).unwrap())
						.collect(),
					parse_formula(goal).unwrap(),
				),
				Progress::default(),
			)
			.unwrap(),
		)
	}

	#[test]
	fn rules_scroll_without_switching_views_and_drafts_survive_resizing() {
		let mut app: ProofScreen = screen(&["P", "P -> Q"], "Q");
		app.handle(Event::Paste("Q ; mp ; 1,2".into()));
		for (width, height) in [(40, 10), (20, 6), (1, 1), (0, 0), (80, 24)] {
			let mut terminal: Terminal<TestBackend> =
				Terminal::new(TestBackend::new(width, height)).unwrap();
			terminal
				.draw(|frame| {
					app.draw(frame);
				})
				.unwrap();
			assert_eq!(app.practice.input(), "Q ; mp ; 1,2");
		}
		let mut terminal: Terminal<TestBackend> = Terminal::new(TestBackend::new(30, 8)).unwrap();
		app.handle(Event::Key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)));
		terminal
			.draw(|frame| {
				app.draw(frame);
			})
			.unwrap();
		assert!(app.scroll.maximum > 0);
		while app.scroll.offset < app.scroll.maximum {
			app.handle(Event::Key(KeyEvent::new(
				KeyCode::PageDown,
				KeyModifiers::NONE,
			)));
		}
		terminal
			.draw(|frame| {
				app.draw(frame);
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
		app.handle(Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)));
		assert_eq!(app.practice.input(), "Q ; mp ; 1,2");
		assert!(app.handle(Event::Key(KeyEvent::new(
			KeyCode::Enter,
			KeyModifiers::NONE
		))));
		assert!(app.practice.is_finished());
	}
}
