mod app;
mod inline;
mod proof_ui;
mod render;
#[cfg(test)]
mod tests;
mod transcript;
mod viewport;

use std::{
	io::{self, IsTerminal},
	path::PathBuf,
};

use crossterm::event;
use ratatui::text::Text;

use inline::{InlineTerminal, TerminalGuard};
use transcript::Transcript;

pub use app::App;
pub use proof_ui::run_proof;

pub fn run(mut app: App, progress_path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
	if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
		return Err("交互练习需要终端。可使用 --list 查看题目，或 --trace 查看逐步演示。".into());
	}
	let _guard: TerminalGuard = TerminalGuard::enter()?;
	let mut terminal: InlineTerminal = InlineTerminal::new()?;
	let mut transcript: Transcript = Transcript::default();
	loop {
		let added: Text<'static> = transcript.sync(&app);
		if !added.lines.is_empty() {
			terminal.append(added)?;
		}
		terminal.draw(|frame| render::draw(frame, &mut app))?;
		let changed: bool = app.handle(event::read()?)?;
		if changed || app.quit {
			app.record();
			if let Some(path) = &progress_path {
				app.progress.save(path)?;
			}
		}
		if app.quit {
			let result: String = if let Some(error) = app.session.terminal_error() {
				format!(
					"{}\n{}",
					app.session.render(),
					error.name().unwrap_or("异常")
				)
			} else {
				app.session.render().into()
			};
			terminal.finish(Text::raw(result))?;
			return Ok(());
		}
	}
}
