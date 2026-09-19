use std::io::{self, Stdout};

use crossterm::{
	cursor::{MoveTo, Show},
	event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
	execute,
	style::ResetColor,
	terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
	Frame, Terminal, TerminalOptions, Viewport,
	backend::{Backend, CrosstermBackend},
	layout::{Position, Rect, Size},
	text::{Line, Text},
	widgets::{Paragraph, Widget},
};

use super::viewport;

pub(super) const HEIGHT: u16 = 5;

/// Restore input modes without entering or leaving the terminal's alternate screen.
pub(super) struct TerminalGuard;

impl TerminalGuard {
	pub fn enter() -> io::Result<Self> {
		enable_raw_mode()?;
		let guard: Self = Self;
		execute!(io::stdout(), EnableMouseCapture, EnableBracketedPaste)?;
		Ok(guard)
	}
}

impl Drop for TerminalGuard {
	fn drop(&mut self) {
		let _cleanup: io::Result<()> = execute!(
			io::stdout(),
			DisableMouseCapture,
			DisableBracketedPaste,
			ResetColor,
			Show
		);
		let _raw: io::Result<()> = disable_raw_mode();
	}
}

pub(super) fn append<B: Backend>(
	terminal: &mut Terminal<B>,
	text: Text<'_>,
) -> Result<(), B::Error> {
	let area: Rect = terminal.get_frame().area();
	if area.is_empty() {
		return Ok(());
	}
	let lines: Vec<Line<'static>> = viewport::wrap(&text, area.width);
	for chunk in lines.chunks(usize::from(u16::MAX)) {
		terminal.insert_before(chunk.len() as u16, |buffer| {
			Paragraph::new(chunk.to_vec())
				.style(text.style)
				.render(buffer.area, buffer);
		})?;
	}
	Ok(())
}

pub(super) struct InlineTerminal {
	terminal: Terminal<CrosstermBackend<Stdout>>,
	size: Size,
	cursor_offset: u16,
}

impl InlineTerminal {
	pub fn new() -> io::Result<Self> {
		let terminal: Terminal<CrosstermBackend<Stdout>> = Terminal::with_options(
			CrosstermBackend::new(io::stdout()),
			TerminalOptions {
				viewport: Viewport::Inline(HEIGHT),
			},
		)?;
		let size: Size = terminal.size()?;
		Ok(Self {
			terminal,
			size,
			cursor_offset: 0,
		})
	}

	fn prepare(&mut self) -> io::Result<()> {
		let size: Size = self.terminal.size()?;
		if size != self.size && size.width > 0 && size.height > 0 {
			// Ratatui's inline resize clears the whole screen on width shrink. Re-anchor
			// only our active lines instead, preserving shell output and scrollback.
			let cursor: Position = self.terminal.get_cursor_position()?;
			let row: u16 = cursor
				.y
				.saturating_sub(self.cursor_offset)
				.min(size.height - 1);
			execute!(
				io::stdout(),
				MoveTo(0, row),
				Clear(ClearType::FromCursorDown)
			)?;
			*self = Self::new()?;
		}
		Ok(())
	}

	pub fn draw(
		&mut self,
		draw: impl FnOnce(&mut Frame<'_>) -> Option<Position>,
	) -> io::Result<()> {
		self.prepare()?;
		let mut cursor: Option<Position> = None;
		let mut area: Rect = Rect::default();
		self.terminal.draw(|frame| {
			area = frame.area();
			cursor = draw(frame);
		})?;
		let position: Position = cursor.unwrap_or(area.as_position());
		self.cursor_offset = position.y.saturating_sub(area.y);
		// Keep a known anchor even when the input cursor is hidden.
		if cursor.is_none() && !area.is_empty() {
			self.terminal.set_cursor_position(position)?;
		}
		Ok(())
	}

	pub fn append(&mut self, text: Text<'_>) -> io::Result<()> {
		self.prepare()?;
		append(&mut self.terminal, text)?;
		let area: Rect = self.terminal.get_frame().area();
		self.terminal.set_cursor_position(area.as_position())?;
		self.cursor_offset = 0;
		Ok(())
	}

	pub fn finish(&mut self, text: Text<'_>) -> io::Result<()> {
		self.append(text)?;
		let area: Rect = self.terminal.get_frame().area();
		execute!(
			io::stdout(),
			MoveTo(area.x, area.y),
			Clear(ClearType::FromCursorDown),
			Show
		)?;
		Ok(())
	}
}
