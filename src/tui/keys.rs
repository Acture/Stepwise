use crossterm::event::{
	Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::layout::{Position, Rect};

use super::viewport::Scroll;
use crate::{
	app::Practice,
	core::{NodeId, ParseError},
};

/// The keys this front end binds. Only the terminal knows them, so the sentence lives here
/// rather than in the app layer, next to the words [`super::say`] puts to every reason the
/// app layer reports.
pub(super) const HELP: &str = "点击当前式的子表达式 → 原位变成 ____ → 填值 → Enter 提交；填错不推进，Esc 取消。↑↓/j k 选择，PgUp/PgDn 滚动当前内容；历史使用终端自身滚动，s 短路开关，u 撤销，r 重做，n 下一道随机题，p 上一题，q 或 Ctrl+C 退出。";

/// The terminal's own state around one practice: where each node was drawn, how the active
/// area scrolls, and whether this run is ending. The [`Practice`] is the single source of
/// teaching state — nothing here keeps a second copy of the session or the progress.
pub(super) struct Screen {
	pub(super) practice: Practice,
	pub(super) expression_hits: Vec<(Rect, NodeId)>,
	pub(super) expression_area: Rect,
	pub(super) view_scroll: Scroll,
	pub(super) follow_tail: bool,
	pub(super) quit: bool,
}

impl Screen {
	pub(super) fn new(practice: Practice) -> Self {
		Self {
			practice,
			expression_hits: Vec::new(),
			expression_area: Rect::default(),
			view_scroll: Scroll::default(),
			follow_tail: true,
			quit: false,
		}
	}

	fn scroll(&mut self, down: bool) {
		if down {
			self.view_scroll.down();
		} else {
			self.view_scroll.up();
		}
		self.follow_tail = false;
	}

	/// Selecting always brings the active area back to the draft.
	pub(super) fn select(&mut self, node_id: NodeId) -> bool {
		self.follow_tail = true;
		self.practice.select(node_id)
	}

	fn key(&mut self, key: KeyEvent) -> Result<bool, ParseError> {
		if key.kind == KeyEventKind::Release {
			return Ok(false);
		}
		if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
			self.quit = true;
			return Ok(false);
		}
		match key.code {
			KeyCode::Esc => {
				self.practice.cancel();
				return Ok(false);
			}
			KeyCode::PageUp | KeyCode::PageDown => {
				self.scroll(key.code == KeyCode::PageDown);
				return Ok(false);
			}
			_ => {}
		}
		if self.practice.draft().is_some() {
			match key.code {
				KeyCode::Enter => {
					self.follow_tail = true;
					return Ok(self.practice.submit());
				}
				KeyCode::Backspace => {
					self.practice.backspace();
					self.follow_tail = true;
				}
				KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
					self.practice.clear_input();
				}
				KeyCode::Char(character)
					if !key
						.modifiers
						.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
				{
					// Follow the tail only when the draft actually took the character.
					self.follow_tail |= self.practice.type_character(character);
				}
				_ => {}
			}
			return Ok(false);
		}
		match key.code {
			KeyCode::Char('q') => self.quit = true,
			KeyCode::Enter => {
				let selected: NodeId = self.practice.selected();
				return Ok(self.select(selected));
			}
			KeyCode::Up | KeyCode::Char('k') => {
				self.practice.select_previous();
				self.follow_tail = true;
			}
			KeyCode::Down | KeyCode::Char('j') => {
				self.practice.select_next();
				self.follow_tail = true;
			}
			KeyCode::Char('h') => self.practice.hint(),
			KeyCode::Char('?') => self.practice.note(HELP),
			KeyCode::Char('s') => {
				self.follow_tail = true;
				return self.practice.toggle_mode();
			}
			KeyCode::Char('u') => {
				if self.practice.undo() {
					self.follow_tail = true;
					return Ok(true);
				}
			}
			KeyCode::Char('r') => {
				self.follow_tail = true;
				return self.practice.reset();
			}
			KeyCode::Char('n') => {
				self.follow_tail = true;
				return self.practice.next_question();
			}
			KeyCode::Char('p') => {
				self.follow_tail = true;
				return self.practice.previous_question();
			}
			_ => {}
		}
		Ok(false)
	}

	pub(super) fn handle(&mut self, event: Event) -> Result<bool, ParseError> {
		match event {
			Event::Key(key) => self.key(key),
			Event::Paste(text) if self.practice.draft().is_some() => {
				self.practice.paste(&text);
				self.follow_tail = true;
				Ok(false)
			}
			Event::Mouse(mouse) => {
				let position: Position = (mouse.column, mouse.row).into();
				if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
					if let Some(node_id) = self
						.expression_hits
						.iter()
						.find(|(area, _)| area.contains(position))
						.map(|(_, node_id)| *node_id)
					{
						return Ok(self.select(node_id));
					}
				} else if matches!(
					mouse.kind,
					MouseEventKind::ScrollDown | MouseEventKind::ScrollUp
				) && self.expression_area.contains(position)
				{
					self.scroll(mouse.kind == MouseEventKind::ScrollDown);
				}
				Ok(false)
			}
			_ => Ok(false),
		}
	}
}
