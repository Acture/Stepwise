//! The terminal front end: an adapter over [`crate::app`]. It owns the inline viewport,
//! key and mouse mapping, scrolling, the cursor, drawing — and the words for every reason
//! the app layer reports, since they name the keys only this front end binds. Teaching
//! state lives in the [`Lesson`]; this module never keeps a second copy of it, and never
//! decides what a step means. One loop runs the whole lesson, drawing an expression or a
//! proof as the question in hand requires.

mod inline;
mod keys;
mod proof_ui;
mod render;
mod say;
#[cfg(test)]
mod tests;
mod viewport;

use std::{
	io::{self, IsTerminal},
	path::PathBuf,
};

use crossterm::event;
use ratatui::text::{Line, Text};

use inline::{InlineTerminal, TerminalGuard};
use keys::Screen;

use crate::app::{Lesson, Task, Transcript};

/// Archived lines as the inline terminal wants them; the app layer names no widget type.
fn text(lines: Vec<String>) -> Text<'static> {
	Text::from(
		lines
			.into_iter()
			.map(Line::from)
			.collect::<Vec<Line<'static>>>(),
	)
}

pub fn run(
	lesson: Lesson,
	progress_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
	if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
		return Err(match lesson.task() {
			Task::Evaluation(_) => {
				"交互练习需要终端。可使用 --list 查看题目，或 --trace 查看逐步演示。"
			}
			Task::Proof(_) => "自然演绎练习需要交互终端；使用 --check-proof 检查证明文件。",
		}
		.into());
	}
	let mut screen: Screen = Screen::new(lesson);
	let _guard: TerminalGuard = TerminalGuard::enter()?;
	let mut terminal: InlineTerminal = InlineTerminal::new()?;
	let mut transcript: Transcript = Transcript::default();
	loop {
		let added: Vec<String> = transcript.sync(&screen.lesson);
		if !added.is_empty() {
			terminal.append(text(added))?;
		}
		terminal.draw(|frame| render::draw(frame, &mut screen))?;
		let changed: bool = screen.handle(event::read()?)?;
		if changed || screen.quit {
			screen.lesson.record();
			if let Some(path) = &progress_path {
				screen.lesson.progress().save(path)?;
			}
		}
		if screen.quit {
			terminal.finish(Text::raw(closing(&screen.lesson)))?;
			return Ok(());
		}
	}
}

/// The last line left in the terminal: where the expression stands, or whether the proof
/// was finished.
fn closing(lesson: &Lesson) -> String {
	match lesson.task() {
		Task::Evaluation(practice) => {
			let session: &crate::core::Session = practice.session();
			match session.terminal_error() {
				Some(error) => format!("{}\n{}", session.render(), error.name().unwrap_or("异常")),
				None => session.render().into(),
			}
		}
		Task::Proof(practice) => if practice.is_finished() {
			"证明完成。"
		} else {
			"本次练习结束。"
		}
		.into(),
	}
}
