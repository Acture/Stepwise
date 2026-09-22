//! The terminal front end: an adapter over [`crate::app`]. It owns the inline viewport,
//! key and mouse mapping, scrolling, the cursor, drawing — and the words for every reason
//! the app layer reports, since they name the keys only this front end binds. Teaching
//! state lives in [`Practice`]; this module never keeps a second copy of it, and never
//! decides what a step means.

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

use crate::app::{Practice, Transcript};

pub use proof_ui::run_proof;

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
	practice: Practice,
	progress_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
	if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
		return Err("交互练习需要终端。可使用 --list 查看题目，或 --trace 查看逐步演示。".into());
	}
	let mut screen: Screen = Screen::new(practice);
	let _guard: TerminalGuard = TerminalGuard::enter()?;
	let mut terminal: InlineTerminal = InlineTerminal::new()?;
	let mut transcript: Transcript = Transcript::default();
	loop {
		let added: Vec<String> = transcript.sync(&screen.practice);
		if !added.is_empty() {
			terminal.append(text(added))?;
		}
		terminal.draw(|frame| render::draw(frame, &mut screen))?;
		let changed: bool = screen.handle(event::read()?)?;
		if changed || screen.quit {
			screen.practice.record();
			if let Some(path) = &progress_path {
				screen.practice.progress().save(path)?;
			}
		}
		if screen.quit {
			let session: &crate::core::Session = screen.practice.session();
			let result: String = if let Some(error) = session.terminal_error() {
				format!("{}\n{}", session.render(), error.name().unwrap_or("异常"))
			} else {
				session.render().into()
			};
			terminal.finish(Text::raw(result))?;
			return Ok(());
		}
	}
}
