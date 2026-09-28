use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, ops::Range};

use super::selection::{allowed_steps, reference_steps};
use super::surface::Surface;

use super::{
	EvalError, Expr, ExprKind, Language, NextStep, NodeId, ParseError, Path, Value, reference_step,
};

/// The teaching-rule version in every progress key. Bump it whenever the steps a student may
/// submit change, so records written under other rules stay apart instead of replaying wrongly.
const RULES: &str = "flexible-substitution-v4";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedbackKind {
	Correct,
	AlreadyValue,
	NeedsInner,
	OutOfOrder,
	WrongValue,
	WrongType,
	InvalidInput,
	Unsupported,
	Finished,
}

#[derive(Clone, Debug)]
pub struct Feedback {
	pub kind: FeedbackKind,
	pub message: String,
}

impl Feedback {
	fn new(kind: FeedbackKind, message: impl Into<String>) -> Self {
		Self {
			kind,
			message: message.into(),
		}
	}

	pub fn accepted(&self) -> bool {
		self.kind == FeedbackKind::Correct
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecordedAttempt {
	pub node_id: NodeId,
	/// None records a deliberate group-removal click without an answer.
	pub input: Option<String>,
}

#[derive(Clone, Debug)]
pub struct HistoryEntry {
	pub before: String,
	pub selected: String,
	pub result: String,
	pub explanation: String,
	root_before: Expr,
	surface_before: Surface,
}

#[derive(Clone, Debug)]
pub struct Session {
	context: String,
	language: Language,
	source: String,
	root: Expr,
	surface: Surface,
	history: Vec<HistoryEntry>,
	attempts: Vec<RecordedAttempt>,
	terminal_error: Option<EvalError>,
}

impl Session {
	/// Built by the language module that parsed `root`; `context` is part of the saved
	/// progress key, so each language spells its own name and binding rendering.
	pub(crate) fn new(language: Language, source: String, root: Expr, context: String) -> Self {
		let surface: Surface = Surface::new(&source, &root);
		let mut session: Self = Self {
			context,
			language,
			root,
			surface,
			source,
			history: Vec::new(),
			attempts: Vec::new(),
			terminal_error: None,
		};
		session.finish_completed_expression();
		session
	}

	pub fn progress_key(&self) -> String {
		format!("{RULES}\n{}", self.context)
	}

	/// A whole expression that already spells a complete answer finishes without a step.
	/// The owning language decides; nothing inside the expression is collapsed.
	fn finish_completed_expression(&mut self) {
		let ExprKind::Operation(op, operands) = &self.root.kind else {
			return;
		};
		let Some(value) = op.rules().completed_value(operands) else {
			return;
		};
		self.root.replace(self.root.id, &value);
		self.surface
			.replace(self.root.id, &value.to_string(), &self.root);
	}

	pub fn language(&self) -> Language {
		self.language
	}

	pub fn replay(mut self, attempts: &[RecordedAttempt]) -> Result<Self, ParseError> {
		for attempt in attempts {
			let feedback: Feedback = match &attempt.input {
				Some(input) => self.submit(attempt.node_id, input),
				None => self.remove_group(attempt.node_id),
			};
			if !feedback.accepted() {
				return Err(ParseError(format!("进度无法重放：{}", feedback.message)));
			}
		}
		Ok(self)
	}

	pub fn source(&self) -> &str {
		&self.source
	}
	pub fn root(&self) -> &Expr {
		&self.root
	}
	pub fn render(&self) -> &str {
		&self.surface.text
	}
	pub fn render_with_ranges(&self) -> (&str, &BTreeMap<NodeId, Range<usize>>) {
		(&self.surface.text, &self.surface.ranges)
	}
	pub fn history(&self) -> &[HistoryEntry] {
		&self.history
	}
	pub fn attempts(&self) -> &[RecordedAttempt] {
		&self.attempts
	}
	pub fn terminal_error(&self) -> Option<&EvalError> {
		self.terminal_error.as_ref()
	}
	pub fn is_finished(&self) -> bool {
		self.root.value().is_some() || self.terminal_error.is_some()
	}

	/// Selection is redundant only for a whole binary expression with two known values.
	pub fn final_binary_step(&self) -> Option<NodeId> {
		let binary: bool =
			matches!(&self.root.kind, ExprKind::Operation(_, operands) if operands.len() == 2);
		(binary
			&& !self.is_finished()
			&& self
				.root
				.children()
				.iter()
				.all(|child| child.value().is_some()))
		.then_some(self.root.id)
	}

	/// Every step the student may submit now: wherever the operands already decide a short
	/// circuit it may be taken, and the operands it would skip may still be computed. Empty
	/// once the question is finished.
	pub fn allowed_steps(&self) -> Vec<NextStep> {
		if self.is_finished() {
			Vec::new()
		} else {
			allowed_steps(&self.root)
		}
	}

	/// The one step a hint names: the language's own short-circuit order, within the
	/// precedence the exercise teaches. It is one of [`Session::allowed_steps`], often not the
	/// only one, and a front end must not use it to preselect anything.
	pub fn next_step(&self) -> Option<NextStep> {
		if self.is_finished() {
			None
		} else {
			let mut choices: Vec<NextStep> = reference_steps(&self.root);
			let preferred: Option<NextStep> = reference_step(&self.root, Path::ShortCircuit);
			let index: usize = preferred
				.and_then(|step| {
					choices
						.iter()
						.position(|choice| choice.node_id == step.node_id)
				})
				.unwrap_or(0);
			(!choices.is_empty()).then(|| choices.remove(index))
		}
	}

	/// A single binding answer replaces every occurrence of the selected name.
	pub fn replacement_ids(&self, node_id: NodeId) -> Vec<NodeId> {
		let Some(selected) = self.root.find(node_id) else {
			return Vec::new();
		};
		self.root
			.rows()
			.into_iter()
			.filter_map(|(_, node)| {
				let matches: bool = match (&selected.kind, &node.kind) {
					(
						ExprKind::Binding { name, .. },
						ExprKind::Binding {
							name: occurrence, ..
						},
					) => name == occurrence,
					_ => node.id == node_id,
				};
				matches.then_some(node.id)
			})
			.collect()
	}

	/// The next step worth naming, or `None` where none is left. A question with no next step
	/// is a notice about the practice rather than something core judged, so core says nothing
	/// about it and the layer above carries that reason instead.
	pub fn hint(&self) -> Option<String> {
		self.next_step().map(|step| {
			format!(
				"下一步选择：{}。先判断应用哪条规则，再填写值或异常名。",
				self.root.find(step.node_id).expect("step exists").render()
			)
		})
	}

	/// Validate the student's selection without supplying or revealing a replacement value.
	pub fn check_selection(&self, node_id: NodeId) -> Result<(), Feedback> {
		self.selected_step(node_id).map(|_| ())
	}

	fn selected_step(&self, node_id: NodeId) -> Result<NextStep, Feedback> {
		let Some(step) = self.next_step() else {
			return Err(Feedback::new(
				FeedbackKind::Finished,
				"本题已结束；最终值或异常只是本次求值的完成标志。",
			));
		};
		let Some(selected) = self.root.find(node_id) else {
			return Err(Feedback::new(
				FeedbackKind::InvalidInput,
				"这个节点已不存在，请重新选择。",
			));
		};
		let allowed: Vec<NextStep> = allowed_steps(&self.root);
		if let Some(choice) = allowed.iter().find(|choice| choice.node_id == node_id) {
			return Ok(choice.clone());
		}
		if selected.value().is_some() {
			return Err(Feedback::new(
				FeedbackKind::AlreadyValue,
				"这里已经是一个值，无需再算。请选择一个运算表达式。",
			));
		}
		let inner: Option<NextStep> = reference_step(selected, Path::ShortCircuit);
		Err(
			if let Some(inner) = inner.filter(|inner| inner.node_id != node_id) {
				// Name inner work the student may do now: the reference's own step when it is
				// allowed, else the first allowed step inside, else the reference's step.
				let named: NodeId = allowed
					.iter()
					.map(|choice| choice.node_id)
					.filter(|id| *id != node_id && selected.find(*id).is_some())
					.min_by_key(|id| *id != inner.node_id)
					.unwrap_or(inner.node_id);
				Feedback::new(
					FeedbackKind::NeedsInner,
					format!(
						"这一步不能跳过内部运算。请先计算 {}，再回到当前表达式。",
						selected.find(named).expect("inner step exists").render()
					),
				)
			} else {
				let expected: &Expr = self
					.root
					.find(self.blocker(node_id, &allowed).unwrap_or(step.node_id))
					.expect("step exists");
				Feedback::new(
					FeedbackKind::OutOfOrder,
					format!(
						"这里目前还不能计算，请先处理 {}。同优先级的独立子式可以任选。",
						expected.render()
					),
				)
			},
		)
	}

	/// What holds a ready operation back: an allowed operation of higher precedence in the same
	/// bracket scope, outside the operation's own ancestors — naming an ancestor would tell the
	/// student to take a step that removes what they selected. The one nearest the selection
	/// wins, then the higher precedence.
	fn blocker(&self, node_id: NodeId, allowed: &[NextStep]) -> Option<NodeId> {
		let path: Vec<&Expr> = lineage(&self.root, node_id);
		let scope = |path: &[&Expr]| -> Option<NodeId> {
			path[..path.len() - 1]
				.iter()
				.rev()
				.find(|node| matches!(node.kind, ExprKind::Group(_)))
				.map(|node| node.id)
		};
		let precedence = |node: &Expr| -> Option<u8> {
			match &node.kind {
				ExprKind::Operation(op, _) => Some(op.rules().precedence()),
				_ => None,
			}
		};
		let own: u8 = precedence(path.last()?)?;
		allowed
			.iter()
			.filter_map(|choice| {
				let other: Vec<&Expr> = lineage(&self.root, choice.node_id);
				let rank: u8 = precedence(other.last()?)?;
				let shared: usize = path
					.iter()
					.zip(&other)
					.take_while(|(left, right)| left.id == right.id)
					.count();
				// Neither an ancestor nor inside the operand a short circuit here would skip.
				let apart: bool = shared < other.len() && shared < path.len();
				(rank > own && apart && scope(&other) == scope(&path)).then_some((
					shared,
					rank,
					choice.node_id,
				))
			})
			.max_by_key(|(shared, rank, _)| (*shared, *rank))
			.map(|(_, _, id)| id)
	}

	/// Whether running the question on from here in the language's own order raises at this
	/// very node: `Some(true)` when it does, `Some(false)` when that order finishes, raises
	/// somewhere else first or discards the node unevaluated, `None` when a limit of this
	/// program stops it before any of those.
	fn raised_in_order(&self, node_id: NodeId) -> Option<bool> {
		let mut root: Expr = self.root.clone();
		while let Some(step) = reference_step(&root, Path::ShortCircuit) {
			match step.outcome {
				Ok(value) => {
					root.replace(step.node_id, &value);
					if root.find(node_id).is_none() {
						return Some(false);
					}
				}
				Err(EvalError::Limit(_)) if step.node_id != node_id => return None,
				Err(_) => return Some(step.node_id == node_id),
			}
		}
		Some(false)
	}

	/// Checks location before reading the answer; never advances the session.
	pub fn check_attempt(&self, node_id: NodeId, input: &str) -> Feedback {
		let step: NextStep = match self.selected_step(node_id) {
			Ok(step) => step,
			Err(feedback) => return feedback,
		};
		match &step.outcome {
			// Only Python's numeric rules raise a limit, so the note names Python.
			Err(EvalError::Limit(reason)) => Feedback::new(
				FeedbackKind::Unsupported,
				format!("超出首版支持范围：{reason} 这不是 Python 求值结果，也不计作答错。"),
			),
			Err(error) => {
				if Some(input.trim()) == error.name() {
					let chosen: &str = &self.surface.text[self.surface.ranges[&node_id].clone()];
					// Only Python raises, so the note about running the original names Python.
					let origin: String = match self.raised_in_order(node_id) {
						Some(true) => String::new(),
						Some(false) => format!(
							"这个异常来自你选择计算的子式 {chosen}；Python 执行原式时不会在这里引发它。"
						),
						None => format!("这个异常来自你选择计算的子式 {chosen}。"),
					};
					Feedback::new(
						FeedbackKind::Correct,
						format!("正确。{} {origin}求值在这里终止。", step.explanation),
					)
				} else {
					Feedback::new(
						FeedbackKind::WrongValue,
						format!(
							"这一步不会产生普通值。{} 请填写异常名称。",
							step.explanation
						),
					)
				}
			}
			Ok(expected) => {
				let actual: Value = match self.language.parse_answer(input) {
					Ok(value) => value,
					Err(error) => {
						return Feedback::new(FeedbackKind::InvalidInput, error.to_string());
					}
				};
				if expected.type_name() != actual.type_name() {
					Feedback::new(
						FeedbackKind::WrongType,
						format!(
							"这里应返回 {}，你填写的是 {}。值相等也不代表类型相同。{}",
							expected.type_name(),
							actual.type_name(),
							step.explanation
						),
					)
				} else if expected.same_answer(&actual) {
					Feedback::new(FeedbackKind::Correct, format!("正确。{}", step.explanation))
				} else {
					Feedback::new(
						FeedbackKind::WrongValue,
						format!(
							"选对了位置，但结果不正确。{} 请再算一次。",
							step.explanation
						),
					)
				}
			}
		}
	}

	pub fn submit(&mut self, node_id: NodeId, input: &str) -> Feedback {
		let feedback: Feedback = self.check_attempt(node_id, input);
		if !feedback.accepted() {
			return feedback;
		}
		let step: NextStep = self.selected_step(node_id).expect("accepted step exists");
		self.apply_step(step, Some(input.trim().into()), feedback)
	}

	pub fn remove_group(&mut self, node_id: NodeId) -> Feedback {
		let step: NextStep = match self.selected_step(node_id) {
			Ok(step) => step,
			Err(feedback) => return feedback,
		};
		if !matches!(
			self.root.find(node_id).map(|node| &node.kind),
			Some(ExprKind::Group(_))
		) {
			return Feedback::new(
				FeedbackKind::InvalidInput,
				"只有已算完的括号可以直接去掉；运算仍需填写结果。",
			);
		}
		let feedback: Feedback = Feedback::new(FeedbackKind::Correct, "已去掉这一层括号。");
		self.apply_step(step, None, feedback)
	}

	/// The note to show when a negative value at this node would change its parent's meaning.
	fn negative_brackets(&self, node_id: NodeId) -> Option<&'static str> {
		self.root.rows().iter().find_map(|(_, parent)| {
			let ExprKind::Operation(op, operands) = &parent.kind else {
				return None;
			};
			operands
				.iter()
				.position(|operand| operand.id == node_id)
				.and_then(|index| op.rules().negative_brackets(index))
		})
	}

	fn apply_step(
		&mut self,
		step: NextStep,
		input: Option<String>,
		mut feedback: Feedback,
	) -> Feedback {
		let node_id: NodeId = step.node_id;
		let result: String = match &step.outcome {
			Ok(value) => value.to_string(),
			Err(error) => error.name().expect("supported exception").into(),
		};
		self.history.push(HistoryEntry {
			before: self.surface.text.clone(),
			selected: self.surface.text[self.surface.ranges[&node_id].clone()].into(),
			result,
			explanation: step.explanation,
			root_before: self.root.clone(),
			surface_before: self.surface.clone(),
		});
		self.attempts.push(RecordedAttempt { node_id, input });
		match step.outcome {
			Ok(value) => {
				let ids: Vec<NodeId> = self.replacement_ids(node_id);
				if ids.len() > 1 {
					let reason: String = format!(" 已将所有 {} 处同名变量一起代入。", ids.len());
					feedback.message.push_str(&reason);
					self.history
						.last_mut()
						.expect("recorded step")
						.explanation
						.push_str(&reason);
				}
				for node_id in ids {
					let brackets: Option<&str> = value
						.to_string()
						.starts_with('-')
						.then(|| self.negative_brackets(node_id))
						.flatten();
					let replacement: String = match brackets {
						Some(_) => format!("({value})"),
						None => value.to_string(),
					};
					self.root.replace(node_id, &value);
					self.surface.replace(node_id, &replacement, &self.root);
					if let Some(reason) = brackets {
						feedback.message.push_str(reason);
						self.history
							.last_mut()
							.expect("recorded step")
							.explanation
							.push_str(reason);
					}
				}
				self.finish_completed_expression();
			}
			Err(error) => self.terminal_error = Some(error),
		}
		feedback
	}

	pub fn undo(&mut self) -> bool {
		let Some(entry) = self.history.pop() else {
			return false;
		};
		self.root = entry.root_before;
		self.surface = entry.surface_before;
		self.attempts.pop();
		self.terminal_error = None;
		true
	}
}

/// The nodes from the root down to this one, both included; empty when it is not in the tree.
fn lineage(root: &Expr, node_id: NodeId) -> Vec<&Expr> {
	if root.id == node_id {
		return vec![root];
	}
	root.children()
		.into_iter()
		.find_map(|child| {
			let mut path: Vec<&Expr> = lineage(child, node_id);
			(!path.is_empty()).then(|| {
				path.insert(0, root);
				path
			})
		})
		.unwrap_or_default()
}
