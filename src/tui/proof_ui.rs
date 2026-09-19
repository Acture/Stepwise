use std::{
	io::{self, IsTerminal},
	path::PathBuf,
};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use ratatui::{
	Frame,
	layout::{Position, Rect},
	style::{Color, Style},
	text::{Line, Text},
	widgets::Paragraph,
};

use super::{
	inline::{InlineTerminal, TerminalGuard},
	viewport::{self, Scroll},
};
use crate::{
	logic::proof::{Proof, RULES},
	progress::Progress,
};

struct ProofApp {
	proof: Proof,
	input: String,
	feedback: String,
	good: bool,
	scroll: Scroll,
	quit: bool,
}

impl ProofApp {
	fn new(proof: Proof) -> Self {
		Self {
			proof,
			input: String::new(),
			feedback: "下一行：公式 ; 规则 ; 引用行。Enter 检查，F1 查看规则。".into(),
			good: false,
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
						KeyCode::Char('z') if self.proof.undo() => {
							self.feedback = "已撤销上一行，并恢复对应的假设作用域。".into();
							self.good = false;
							self.scroll.reset();
							return true;
						}
						KeyCode::Char('u') => self.input.clear(),
						_ => {}
					}
					return false;
				}
				match key.code {
					KeyCode::Enter => {
						self.scroll.reset();
						match self.proof.submit(&self.input) {
							Ok(message) => {
								self.input.clear();
								self.feedback = message;
								self.good = true;
								return true;
							}
							Err(error) => {
								self.feedback = error.to_string();
								self.good = false;
							}
						}
					}
					KeyCode::Backspace => {
						self.input.pop();
					}
					KeyCode::Esc => self.input.clear(),
					KeyCode::F(1) => {
						self.feedback = RULES.into();
						self.good = false;
						self.scroll.reset();
					}
					KeyCode::Up | KeyCode::PageUp => self.scroll.up(),
					KeyCode::Down | KeyCode::PageDown => self.scroll.down(),
					KeyCode::Char(character)
						if !key.modifiers.contains(KeyModifiers::ALT)
							&& !character.is_control()
							&& self.input.len() + character.len_utf8() <= 2048 =>
					{
						self.input.push(character);
					}
					_ => {}
				}
			}
			Event::Paste(text) => {
				for character in text.chars().filter(|character| !character.is_control()) {
					if self.input.len() + character.len_utf8() > 2048 {
						break;
					}
					self.input.push(character);
				}
			}
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
				self.proof.goal,
				if self.proof.is_finished() {
					" · 完成"
				} else {
					""
				}
			))
			.style(Style::default().fg(Color::Cyan)),
			header,
		);
		let text: Text<'_> =
			Text::raw(self.feedback.clone()).style(Style::default().fg(if self.good {
				Color::Green
			} else {
				Color::Yellow
			}));
		viewport::draw_text(frame, body, text, &mut self.scroll);
		let cursor: Option<Position> = viewport::draw_input(frame, input, &self.input);
		frame.render_widget(
			Paragraph::new("Enter检查 F1规则 Ctrl+Z撤销 Ctrl+C退出")
				.style(Style::default().fg(Color::DarkGray)),
			footer,
		);
		cursor
	}

	fn lines_from(&self, start: usize) -> Text<'static> {
		Text::from(
			self.proof
				.lines()
				.iter()
				.enumerate()
				.skip(start)
				.map(|(index, line)| {
					Line::from(format!(
						"{} {}{} [{} {}]",
						index + 1,
						"│ ".repeat(line.scope.len()),
						line.formula,
						line.rule,
						line.references
							.iter()
							.map(ToString::to_string)
							.collect::<Vec<_>>()
							.join(",")
					))
				})
				.collect::<Vec<_>>(),
		)
	}
}

pub fn run_proof(
	proof: Proof,
	mut progress: Progress,
	path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
	if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
		return Err("自然演绎练习需要交互终端；使用 --check-proof 检查证明文件。".into());
	}
	let key: String = proof.progress_key();
	let proof: Proof = proof.replay(
		progress
			.proofs
			.get(&key)
			.map(Vec::as_slice)
			.unwrap_or_default(),
	)?;
	let mut app: ProofApp = ProofApp::new(proof);
	let _guard: TerminalGuard = TerminalGuard::enter()?;
	let mut terminal: InlineTerminal = InlineTerminal::new()?;
	terminal.append(Text::raw(format!("自然演绎 · 目标：{}", app.proof.goal)))?;
	terminal.append(app.lines_from(0))?;
	loop {
		terminal.draw(|frame| app.draw(frame))?;
		let before: usize = app.proof.lines().len();
		let changed: bool = app.handle(event::read()?);
		if changed {
			let after: usize = app.proof.lines().len();
			if after > before {
				terminal.append(app.lines_from(before))?;
			} else {
				terminal.append(Text::raw(format!(
					"↶ 撤销第 {} 行及其假设作用域变更。",
					before
				)))?;
			}
		}
		if changed || app.quit {
			progress
				.proofs
				.insert(key.clone(), app.proof.commands().to_vec());
			if let Some(path) = &path {
				progress.save(path)?;
			}
		}
		if app.quit {
			terminal.finish(Text::raw(if app.proof.is_finished() {
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

	#[test]
	fn rules_scroll_without_switching_views_and_drafts_survive_resizing() {
		let mut app: ProofApp = ProofApp::new(Proof::new(
			vec![
				parse_formula("P").unwrap(),
				parse_formula("P -> Q").unwrap(),
			],
			parse_formula("Q").unwrap(),
		));
		app.handle(Event::Paste("Q ; mp ; 1,2".into()));
		for (width, height) in [(40, 10), (20, 6), (1, 1), (0, 0), (80, 24)] {
			let mut terminal: Terminal<TestBackend> =
				Terminal::new(TestBackend::new(width, height)).unwrap();
			terminal
				.draw(|frame| {
					app.draw(frame);
				})
				.unwrap();
			assert_eq!(app.input, "Q ; mp ; 1,2");
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
		assert_eq!(app.input, "Q ; mp ; 1,2");
		assert!(app.handle(Event::Key(KeyEvent::new(
			KeyCode::Enter,
			KeyModifiers::NONE
		))));
		assert!(app.proof.is_finished());
	}
}
