use std::{collections::BTreeMap, ops::Range};

use ratatui::{
	Frame,
	layout::{Position, Rect},
	style::{Color, Style},
	text::{Line, Span},
	widgets::Paragraph,
};
use unicode_width::UnicodeWidthChar;

use super::{keys::Screen, say};
use crate::{app::Task, core::NodeId};

/// Draw whichever view the question in hand needs.
pub(super) fn draw(frame: &mut Frame<'_>, screen: &mut Screen) -> Option<Position> {
	screen.expression_area = Rect::default();
	screen.expression_hits.clear();
	match screen.lesson.task() {
		Task::Evaluation(_) => draw_expression(frame, screen, frame.area()),
		Task::Proof(_) => screen.draw_proof(frame),
	}
}

/// Each rendered character keeps the node the app layer puts under it. Inline drafts are
/// drawn from `draft_spans`; nothing here decides what a draft replaces.
fn draw_expression(frame: &mut Frame<'_>, screen: &mut Screen, area: Rect) -> Option<Position> {
	if area.is_empty() {
		return None;
	}
	screen.expression_area = area;
	let editing: Option<NodeId> = screen.practice().draft();
	let input: String = screen.practice().input().into();
	let edited: Vec<(NodeId, Range<usize>)> = screen.practice().draft_spans();
	let (source, ranges): (&str, &BTreeMap<NodeId, Range<usize>>) =
		screen.practice().session().render_with_ranges();
	let mut cells: Vec<(char, Style, Option<NodeId>)> = Vec::new();
	if editing.is_some() {
		cells.extend(
			format!("{source}\n")
				.chars()
				.map(|character| (character, Style::default().fg(Color::DarkGray), None)),
		);
	}
	let mut cursor_index: Option<usize> = None;
	let owners: Vec<Option<NodeId>> = screen.practice().node_owners();
	let selected: Option<&Range<usize>> = ranges.get(&screen.practice().selected());
	let mut index: usize = 0;
	while index < source.len() {
		if let Some((node_id, range)) = edited.iter().find(|(_, range)| index == range.start) {
			let style: Style = Style::default().fg(Color::Yellow).underlined().bold();
			let draft: &str = if input.is_empty() { "____" } else { &input };
			if input.is_empty() && editing == Some(*node_id) {
				cursor_index = Some(cells.len());
			}
			cells.extend(draft.chars().map(|character| (character, style, editing)));
			if !input.is_empty() && editing == Some(*node_id) {
				cursor_index = Some(cells.len());
				cells.push((' ', style, editing));
			}
			index = range.end;
			continue;
		}
		let character: char = source[index..].chars().next().expect("inside expression");
		let node_id: Option<NodeId> = owners[index];
		let style: Style =
			if selected.is_some_and(|range| range.contains(&index)) && edited.is_empty() {
				Style::default().fg(Color::Cyan)
			} else {
				Style::default()
			};
		cells.push((character, style, node_id));
		index += character.len_utf8();
	}
	if let Some(error) = screen.practice().session().terminal_error() {
		cells.extend(
			format!("\n完成：{}", error.name().unwrap_or("异常"))
				.chars()
				.map(|character| (character, Style::default().fg(Color::Green), None)),
		);
	} else if screen.practice().session().is_finished() {
		cells.extend(
			"\n完成 · n 下一题 / u 撤销"
				.chars()
				.map(|character| (character, Style::default().fg(Color::Green), None)),
		);
	}
	let (feedback, good): (String, bool) = say::feedback(screen.practice().report());
	let style: Style = Style::default().fg(if good { Color::Green } else { Color::Yellow });
	cells.extend(
		format!("\n{feedback}")
			.chars()
			.map(|character| (character, style, None)),
	);
	cells.extend(
		"\nEnter确认 Esc取消 ?帮助 Ctrl+C退出"
			.chars()
			.map(|character| (character, Style::default().fg(Color::DarkGray), None)),
	);
	let mut lines: Vec<Line<'static>> = vec![Line::default()];
	let mut hits: Vec<(u16, u16, u16, NodeId)> = Vec::new();
	let mut column: u16 = 0;
	let mut cursor: Option<(u16, u16)> = None;
	for (index, (character, style, node)) in cells.into_iter().enumerate() {
		let width: u16 = character.width().unwrap_or(0) as u16;
		if character == '\n' || column.saturating_add(width) > area.width && column > 0 {
			lines.push(Line::default());
			column = 0;
		}
		let row: u16 = (lines.len() - 1) as u16;
		if cursor_index == Some(index) {
			cursor = Some((column, row));
		}
		if character == '\n' {
			continue;
		}
		if let Some(id) = node {
			hits.push((
				column,
				row,
				width.min(area.width.saturating_sub(column)),
				id,
			));
		}
		lines
			.last_mut()
			.expect("one line")
			.spans
			.push(Span::styled(character.to_string(), style));
		column = column.saturating_add(width);
	}
	screen.view_scroll.update(lines.len(), area.height);
	if screen.follow_tail {
		screen.view_scroll.offset = 0;
	}
	if screen.follow_tail
		&& let Some((_, row)) = cursor
	{
		if row < screen.view_scroll.offset {
			screen.view_scroll.offset = row;
		}
		if row >= screen.view_scroll.offset.saturating_add(area.height) {
			screen.view_scroll.offset = row.saturating_sub(area.height - 1);
		}
	}
	let offset: u16 = screen.view_scroll.offset;
	for (column, row, width, id) in hits {
		if row >= offset && row - offset < area.height && width > 0 {
			screen.expression_hits.push((
				Rect::new(area.x + column, area.y + row - offset, width, 1),
				id,
			));
		}
	}
	frame.render_widget(Paragraph::new(lines).scroll((offset, 0)), area);
	if let Some((column, row)) = cursor
		&& row >= offset
		&& row - offset < area.height
		&& column < area.width
	{
		let position: Position = Position::new(area.x + column, area.y + row - offset);
		frame.set_cursor_position(position);
		return Some(position);
	}
	None
}
