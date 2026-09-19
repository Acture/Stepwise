use crossterm::event::{
	Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::layout::Rect;

use super::viewport::Scroll;
use crate::{
	core::{EvaluationMode, ExprKind, Feedback, NodeId, ParseError, Session},
	exercises::Exercise,
	generate,
	progress::Progress,
};

pub struct App {
	pub(super) exercises: Vec<Exercise>,
	pub(super) index: usize,
	pub(super) session: Session,
	pub(super) selected: NodeId,
	pub(super) input: String,
	pub(super) editing: Option<NodeId>,
	pub(super) view_scroll: Scroll,
	pub(super) follow_tail: bool,
	pub(super) expression_hits: Vec<(Rect, NodeId)>,
	pub(super) feedback: String,
	pub(super) feedback_good: bool,
	pub(super) expression_area: Rect,
	pub(super) quit: bool,
	pub(super) progress: Progress,
}

impl App {
	pub fn new(
		exercises: Vec<Exercise>,
		progress: Progress,
		index: usize,
		mode: EvaluationMode,
	) -> Result<Self, ParseError> {
		let exercise: &Exercise = exercises
			.get(index)
			.ok_or_else(|| ParseError("没有这道题。".into()))?;
		let initial: Session = exercise.session(mode)?;
		let attempts: &[crate::core::RecordedAttempt] = progress.attempts(&initial);
		let session: Session = initial.replay(attempts)?;
		let selected: NodeId = session.root().id;
		let editing: Option<NodeId> = session.final_binary_step();
		Ok(Self {
			exercises,
			index,
			session,
			selected,
			input: String::new(),
			editing,
			view_scroll: Scroll::default(),
			follow_tail: true,
			expression_hits: Vec::new(),
			feedback: if editing.is_some() {
				"只剩两个值，直接填入本步结果，Enter 检查。"
			} else {
				"点击一处 → ____ → 填值 → Enter。"
			}
			.into(),
			feedback_good: false,
			expression_area: Rect::default(),
			quit: false,
			progress,
		})
	}

	pub(super) fn record(&mut self) {
		self.progress
			.record(&self.exercises[self.index].id, &self.session);
	}

	fn cancel_edit(&mut self) {
		self.editing = None;
		self.input.clear();
	}

	fn edit_final_pair(&mut self) {
		if let Some(id) = self.session.final_binary_step() {
			self.selected = id;
			self.editing = Some(id);
		}
	}

	fn navigate(&mut self, forward: bool) {
		let mut ids: Vec<NodeId> = self
			.session
			.root()
			.rows()
			.iter()
			.filter(|(_, node)| node.value().is_none())
			.map(|(_, node)| node.id)
			.collect();
		if ids.is_empty() {
			ids.push(self.session.root().id);
		}
		let index: usize = ids.iter().position(|id| *id == self.selected).unwrap_or(0);
		self.selected = ids[if forward {
			(index + 1).min(ids.len() - 1)
		} else {
			index.saturating_sub(1)
		}];
		self.cancel_edit();
		self.follow_tail = true;
	}

	pub(super) fn begin_edit(&mut self, id: NodeId) -> bool {
		if self.editing == Some(id) {
			self.follow_tail = true;
			return false;
		}
		self.selected = id;
		self.cancel_edit();
		self.feedback_good = false;
		self.follow_tail = true;
		if matches!(
			self.session.root().find(id).map(|node| &node.kind),
			Some(ExprKind::Group(_))
		) {
			let feedback: Feedback = self.session.remove_group(id);
			let accepted: bool = feedback.accepted();
			self.feedback_good = accepted;
			self.feedback = feedback.message;
			if accepted {
				self.edit_final_pair();
			}
			return accepted;
		}
		match self.session.check_selection(id) {
			Ok(()) => {
				self.editing = Some(id);
				self.feedback = "在 ____ 处填值，Enter 检查；Esc 取消。".into();
			}
			Err(feedback) => {
				self.feedback = feedback.message;
			}
		}
		false
	}

	fn switch(&mut self, forward: bool) -> Result<(), ParseError> {
		self.record();
		if forward && self.index + 1 == self.exercises.len() {
			let exercise: Exercise =
				generate::generate(self.exercises[self.index].language, generate::fresh_seed())?;
			self.exercises.push(exercise);
		}
		let index: usize = if forward {
			self.index + 1
		} else {
			self.index.saturating_sub(1)
		};
		*self = Self::new(
			self.exercises.clone(),
			self.progress.clone(),
			index,
			self.session.mode(),
		)?;
		Ok(())
	}

	fn submit(&mut self) -> bool {
		let Some(id) = self.editing else {
			return self.begin_edit(self.selected);
		};
		let feedback: Feedback = self.session.submit(id, &self.input);
		let accepted: bool = feedback.accepted();
		self.feedback = feedback.message;
		self.feedback_good = accepted;
		self.follow_tail = true;
		if accepted {
			self.cancel_edit();
			self.edit_final_pair();
		}
		accepted
	}

	fn scroll(&mut self, down: bool) {
		let scroll: &mut Scroll = &mut self.view_scroll;
		if down {
			scroll.down();
		} else {
			scroll.up();
		}
		self.follow_tail = false;
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
				self.cancel_edit();
				return Ok(false);
			}
			KeyCode::PageUp | KeyCode::PageDown => {
				self.scroll(key.code == KeyCode::PageDown);
				return Ok(false);
			}
			_ => {}
		}
		if self.editing.is_some() {
			match key.code {
				KeyCode::Enter => return Ok(self.submit()),
				KeyCode::Backspace => {
					self.input.pop();
					self.follow_tail = true;
				}
				KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
					self.input.clear()
				}
				KeyCode::Char(character)
					if !key
						.modifiers
						.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
						&& !character.is_control()
						&& self.input.len() + character.len_utf8() <= 2048 =>
				{
					self.input.push(character);
					self.follow_tail = true;
				}
				_ => {}
			}
			return Ok(false);
		}
		match key.code {
			KeyCode::Char('q') => self.quit = true,
			KeyCode::Enter => return Ok(self.begin_edit(self.selected)),
			KeyCode::Up | KeyCode::Char('k') => self.navigate(false),
			KeyCode::Down | KeyCode::Char('j') => self.navigate(true),
			KeyCode::Char('h') => {
				self.feedback = self.session.hint();
				self.feedback_good = false;
			}
			KeyCode::Char('?') => {
				self.feedback = "点击当前式的子表达式 → 原位变成 ____ → 填值 → Enter 提交；填错不推进，Esc 取消。↑↓/j k 选择，PgUp/PgDn 滚动当前内容；历史使用终端自身滚动，s 短路开关，u 撤销，r 重做，n 下一道随机题，p 上一题，q 或 Ctrl+C 退出。".into();
				self.feedback_good = false;
			}
			KeyCode::Char('s') => {
				self.record();
				let mode: EvaluationMode = self.session.mode().toggled();
				*self = Self::new(
					self.exercises.clone(),
					self.progress.clone(),
					self.index,
					mode,
				)?;
				self.feedback = format!("已切换：{}。进度分别保存。", mode.label());
				return Ok(true);
			}
			KeyCode::Char('u') => {
				if self.session.undo() {
					self.selected = self.session.root().id;
					self.cancel_edit();
					self.edit_final_pair();
					self.follow_tail = true;
					self.feedback = "已撤销上一步。".into();
					self.feedback_good = false;
					return Ok(true);
				}
			}
			KeyCode::Char('r') => {
				self.session = self.exercises[self.index].session(self.session.mode())?;
				self.selected = self.session.root().id;
				self.cancel_edit();
				self.edit_final_pair();
				self.follow_tail = true;
				self.feedback = "已重新开始本题。".into();
				self.feedback_good = false;
				return Ok(true);
			}
			KeyCode::Char('n') => {
				self.switch(true)?;
				return Ok(true);
			}
			KeyCode::Char('p') => {
				self.switch(false)?;
				return Ok(true);
			}
			_ => {}
		}
		Ok(false)
	}

	pub(super) fn handle(&mut self, event: Event) -> Result<bool, ParseError> {
		match event {
			Event::Key(key) => self.key(key),
			Event::Paste(text) if self.editing.is_some() => {
				for character in text.chars().filter(|character| !character.is_control()) {
					if self.input.len() + character.len_utf8() > 2048 {
						break;
					}
					self.input.push(character);
				}
				self.follow_tail = true;
				Ok(false)
			}
			Event::Mouse(mouse) => {
				let position: ratatui::layout::Position = (mouse.column, mouse.row).into();
				if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
					if let Some(id) = self
						.expression_hits
						.iter()
						.find(|(area, _)| area.contains(position))
						.map(|(_, id)| *id)
					{
						return Ok(self.begin_edit(id));
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
