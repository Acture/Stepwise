use crossterm::event::{
	Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::layout::{Position, Rect};

use super::viewport::Scroll;
use crate::{
	app::{Lesson, Practice, ProofPractice, Task},
	core::{NodeId, ParseError},
};

/// The keys this front end binds. Only the terminal knows them, so the sentence lives here
/// rather than in the app layer, next to the words [`super::say`] puts to every reason the
/// app layer reports.
pub(super) const HELP: &str = "点击当前式的子表达式 → 原位变成 ____ → 填值 → Enter 提交；填错不推进，Esc 取消。↑↓/j k 选择，PgUp/PgDn 滚动当前内容；历史使用终端自身滚动，s 短路开关，u 撤销，r 重做，n 下一题（随机练习出新题），p 上一题，q 或 Ctrl+C 退出。";

/// The terminal's own state around one lesson: where each node was drawn, how the active
/// area scrolls, and whether this run is ending. The [`Lesson`] is the single source of
/// teaching state — nothing here keeps a second copy of the session, the proof or the
/// progress.
pub(super) struct Screen {
	pub(super) lesson: Lesson,
	pub(super) expression_hits: Vec<(Rect, NodeId)>,
	pub(super) expression_area: Rect,
	pub(super) view_scroll: Scroll,
	pub(super) follow_tail: bool,
	/// How a proof's feedback, the rule reference included, scrolls.
	pub(super) proof_scroll: Scroll,
	pub(super) quit: bool,
}

impl Screen {
	pub(super) fn new(lesson: Lesson) -> Self {
		Self {
			lesson,
			expression_hits: Vec::new(),
			expression_area: Rect::default(),
			view_scroll: Scroll::default(),
			follow_tail: true,
			proof_scroll: Scroll::default(),
			quit: false,
		}
	}

	/// The expression in hand. Only the evaluation half of this adapter asks, and it only
	/// runs while the lesson is on an evaluation question.
	pub(super) fn practice(&self) -> &Practice {
		match self.lesson.task() {
			Task::Evaluation(practice) => practice,
			Task::Proof(_) => unreachable!("the expression view runs over an evaluation question"),
		}
	}
	pub(super) fn practice_mut(&mut self) -> &mut Practice {
		match self.lesson.task_mut() {
			Task::Evaluation(practice) => practice,
			Task::Proof(_) => unreachable!("the expression view runs over an evaluation question"),
		}
	}
	/// The proof in hand, under the same rule as [`Screen::practice`].
	pub(super) fn proof(&self) -> &ProofPractice {
		match self.lesson.task() {
			Task::Proof(practice) => practice,
			Task::Evaluation(_) => unreachable!("the proof view runs over a proof question"),
		}
	}
	pub(super) fn proof_mut(&mut self) -> &mut ProofPractice {
		match self.lesson.task_mut() {
			Task::Proof(practice) => practice,
			Task::Evaluation(_) => unreachable!("the proof view runs over a proof question"),
		}
	}

	/// One event, handled by whichever view the question in hand needs. True when the
	/// lesson changed in a way worth saving.
	pub(super) fn handle(&mut self, event: Event) -> Result<bool, ParseError> {
		match self.lesson.task() {
			Task::Evaluation(_) => self.evaluation_event(event),
			Task::Proof(_) => self.proof_event(event),
		}
	}

	/// Change question through the lesson, whichever kind either side is. The new question
	/// starts with its own area in view.
	pub(super) fn change_question(&mut self, forward: bool) -> Result<bool, ParseError> {
		self.follow_tail = true;
		self.proof_scroll.reset();
		if forward {
			self.lesson.next_question()
		} else {
			self.lesson.previous_question()
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
		self.practice_mut().select(node_id)
	}

	fn evaluation_key(&mut self, key: KeyEvent) -> Result<bool, ParseError> {
		if key.kind == KeyEventKind::Release {
			return Ok(false);
		}
		if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
			self.quit = true;
			return Ok(false);
		}
		match key.code {
			KeyCode::Esc => {
				self.practice_mut().cancel();
				return Ok(false);
			}
			KeyCode::PageUp | KeyCode::PageDown => {
				self.scroll(key.code == KeyCode::PageDown);
				return Ok(false);
			}
			_ => {}
		}
		if self.practice().draft().is_some() {
			match key.code {
				KeyCode::Enter => {
					self.follow_tail = true;
					return Ok(self.practice_mut().submit());
				}
				KeyCode::Backspace => {
					self.practice_mut().backspace();
					self.follow_tail = true;
				}
				KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
					self.practice_mut().clear_input();
				}
				KeyCode::Char(character)
					if !key
						.modifiers
						.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
				{
					// Follow the tail only when the draft actually took the character.
					self.follow_tail |= self.practice_mut().type_character(character);
				}
				_ => {}
			}
			return Ok(false);
		}
		match key.code {
			KeyCode::Char('q') => self.quit = true,
			KeyCode::Enter => {
				let selected: NodeId = self.practice().selected();
				return Ok(self.select(selected));
			}
			KeyCode::Up | KeyCode::Char('k') => {
				self.practice_mut().select_previous();
				self.follow_tail = true;
			}
			KeyCode::Down | KeyCode::Char('j') => {
				self.practice_mut().select_next();
				self.follow_tail = true;
			}
			KeyCode::Char('h') => self.practice_mut().hint(),
			KeyCode::Char('?') => self.practice_mut().note(HELP),
			KeyCode::Char('s') => {
				self.follow_tail = true;
				return self.practice_mut().toggle_mode();
			}
			KeyCode::Char('u') => {
				if self.practice_mut().undo() {
					self.follow_tail = true;
					return Ok(true);
				}
			}
			KeyCode::Char('r') => {
				self.follow_tail = true;
				return self.practice_mut().reset();
			}
			KeyCode::Char('n') => return self.change_question(true),
			KeyCode::Char('p') => return self.change_question(false),
			_ => {}
		}
		Ok(false)
	}

	fn evaluation_event(&mut self, event: Event) -> Result<bool, ParseError> {
		match event {
			Event::Key(key) => self.evaluation_key(key),
			Event::Paste(text) if self.practice().draft().is_some() => {
				self.practice_mut().paste(&text);
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
