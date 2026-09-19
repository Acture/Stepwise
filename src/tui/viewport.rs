use ratatui::{
	Frame,
	layout::{Position, Rect},
	text::{Line, Span, Text},
	widgets::Paragraph,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Default)]
pub(super) struct Scroll {
	pub offset: u16,
	pub maximum: u16,
	page: u16,
}

impl Scroll {
	pub fn update(&mut self, lines: usize, height: u16) {
		self.maximum = lines
			.saturating_sub(usize::from(height))
			.min(usize::from(u16::MAX)) as u16;
		self.offset = self.offset.min(self.maximum);
		self.page = height.saturating_sub(1).max(1);
	}
	pub fn reset(&mut self) {
		self.offset = 0;
	}
	pub fn down(&mut self) {
		self.offset = self
			.offset
			.saturating_add(self.page.max(1))
			.min(self.maximum);
	}
	pub fn up(&mut self) {
		self.offset = self.offset.saturating_sub(self.page.max(1));
	}
}

/// Wrap by terminal cell width, preserving source styles and explicit line breaks.
pub(super) fn wrap(text: &Text<'_>, width: u16) -> Vec<Line<'static>> {
	if width == 0 {
		return Vec::new();
	}
	let mut lines: Vec<Line<'static>> = Vec::new();
	for source in &text.lines {
		let mut line: Line<'static> = Line::default().style(source.style);
		let mut used: usize = 0;
		for span in &source.spans {
			for character in span.content.chars() {
				let cells: usize = character.width().unwrap_or(0);
				if used + cells > usize::from(width) && used > 0 {
					lines.push(line);
					line = Line::default().style(source.style);
					used = 0;
				}
				line.spans
					.push(Span::styled(character.to_string(), span.style));
				used += cells;
			}
		}
		lines.push(line);
	}
	lines
}

pub(super) fn draw_text(frame: &mut Frame<'_>, area: Rect, text: Text<'_>, scroll: &mut Scroll) {
	if area.is_empty() {
		return;
	}
	let lines: Vec<Line<'static>> = wrap(&text, area.width);
	scroll.update(lines.len(), area.height);
	frame.render_widget(
		Paragraph::new(lines)
			.style(text.style)
			.scroll((scroll.offset, 0)),
		area,
	);
}

pub(super) fn draw_input(frame: &mut Frame<'_>, area: Rect, input: &str) -> Option<Position> {
	if area.is_empty() {
		return None;
	}
	let available: usize = usize::from(area.width.saturating_sub(1));
	let mut start: usize = 0;
	while input[start..].width() > available {
		start += input[start..]
			.chars()
			.next()
			.expect("nonempty input tail")
			.len_utf8();
	}
	let tail: &str = &input[start..];
	frame.render_widget(Paragraph::new(tail), area);
	let position: Position = Position::new(area.x + tail.width() as u16, area.y);
	frame.set_cursor_position(position);
	Some(position)
}

/// At tiny heights prioritize the current panel or focused input; never require resizing.
pub(super) fn compact_rows(area: Rect, input_focused: bool) -> [Rect; 4] {
	let header: u16 = u16::from(area.height >= 4);
	let footer: u16 = u16::from(area.height >= 5);
	let input: u16 = u16::from(area.height >= 2 || input_focused && area.height > 0);
	let body: u16 = area.height.saturating_sub(header + footer + input);
	[
		Rect::new(area.x, area.y, area.width, header),
		Rect::new(area.x, area.y + header, area.width, body),
		Rect::new(area.x, area.y + header + body, area.width, input),
		Rect::new(area.x, area.y + header + body + input, area.width, footer),
	]
}
