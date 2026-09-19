use super::selection::available_steps;
use super::surface::Surface;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, ops::Range};

use super::{
	EvalError, EvaluationMode, Expr, ExprKind, NextStep, NodeId, ParseError, UnaryOp, Value,
	next_step, parse_bound_expression, parse_value,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedbackKind {
	Correct,
	AlreadyValue,
	NeedsInner,
	OutOfOrder,
	Skipped,
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
	mode: EvaluationMode,
	context: String,
	logical: bool,
	source: String,
	root: Expr,
	surface: Surface,
	history: Vec<HistoryEntry>,
	attempts: Vec<RecordedAttempt>,
	terminal_error: Option<EvalError>,
}

impl Session {
	pub fn new(source: &str, mode: EvaluationMode) -> Result<Self, ParseError> {
		Self::with_bindings(source, &BTreeMap::new(), mode)
	}

	pub fn with_bindings(
		source: &str,
		bindings: &BTreeMap<String, Value>,
		mode: EvaluationMode,
	) -> Result<Self, ParseError> {
		let source: String = source.trim().into();
		let root: Expr = parse_bound_expression(&source, bindings)?;
		let surface: Surface = Surface::new(&source, &root);
		let mut session: Self = Self {
			mode,
			context: format!("python\n{bindings:?}\n{source}"),
			logical: false,
			root,
			surface,
			source,
			history: Vec::new(),
			attempts: Vec::new(),
			terminal_error: None,
		};
		session.finish_negative_literal();
		Ok(session)
	}

	pub fn logic(
		source: &str,
		bindings: &std::collections::BTreeMap<String, bool>,
		mode: EvaluationMode,
	) -> Result<Self, ParseError> {
		let source: &str = source.trim();
		let root: Expr = crate::logic::parse_teaching_formula(source, bindings)?;
		let surface: Surface = Surface::new(source, &root);
		Ok(Self {
			mode,
			context: format!("logic\n{bindings:?}\n{}", source.trim()),
			logical: true,
			source: source.trim().into(),
			root,
			surface,
			history: Vec::new(),
			attempts: Vec::new(),
			terminal_error: None,
		})
	}

	pub fn progress_key(&self) -> String {
		let rules: &str = "flexible-substitution-v3";
		format!("{rules}\n{}\n{}", self.mode.key(), self.context)
	}

	/// A final minus sign and unsigned number already spell a complete numeric answer.
	/// Do not collapse groups, inner negations, bool conversion, or double negatives.
	fn finish_negative_literal(&mut self) {
		let ExprKind::Unary(UnaryOp::Negative, operand) = &self.root.kind else {
			return;
		};
		let Some(value @ (Value::Int(_) | Value::Float(_))) = operand.value() else {
			return;
		};
		if value.to_string().starts_with('-') {
			return;
		}
		let value: Value = value
			.unary(UnaryOp::Negative)
			.expect("negating a checked finite number stays in range");
		self.root.replace(self.root.id, &value);
		self.surface
			.replace(self.root.id, &value.to_string(), &self.root);
	}
	pub fn mode_label(&self) -> &'static str {
		if self.logical {
			"命题逻辑"
		} else {
			"Python 运算练习"
		}
	}

	pub fn is_logic(&self) -> bool {
		self.logical
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
	pub fn mode(&self) -> EvaluationMode {
		self.mode
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
		use super::ExprKind;
		let binary: bool = matches!(
			&self.root.kind,
			ExprKind::Binary(_, _, _) | ExprKind::Compare(_, _, _) | ExprKind::Logic(_, _, _)
		) || matches!(&self.root.kind, ExprKind::Bool(_, values) if values.len() == 2);
		(binary
			&& !self.is_finished()
			&& self
				.root
				.children()
				.iter()
				.all(|child| child.value().is_some()))
		.then_some(self.root.id)
	}
	pub fn next_step(&self) -> Option<NextStep> {
		if self.is_finished() {
			None
		} else {
			let mut choices: Vec<NextStep> = available_steps(&self.root, self.mode);
			let preferred: Option<NextStep> = next_step(&self.root, self.mode);
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
					(ExprKind::Variable(name, _), ExprKind::Variable(other, _))
					| (ExprKind::Proposition(name, _), ExprKind::Proposition(other, _)) => name == other,
					_ => node.id == node_id,
				};
				matches.then_some(node.id)
			})
			.collect()
	}

	pub fn hint(&self) -> String {
		match self.next_step() {
			Some(step) => format!(
				"下一步选择：{}。先判断应用哪条规则，再填写值或异常名。",
				self.root.find(step.node_id).expect("step exists").render()
			),
			None => "本题已结束。可以按 n 进入下一题，或按 u 撤销。".into(),
		}
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
		if let Some(choice) = available_steps(&self.root, self.mode)
			.into_iter()
			.find(|choice| choice.node_id == node_id)
		{
			return Ok(choice);
		}
		if node_id != step.node_id {
			if step.skipped.iter().any(|id| {
				self.root
					.find(*id)
					.is_some_and(|tree| tree.find(node_id).is_some())
			}) {
				return Err(Feedback::new(
					FeedbackKind::Skipped,
					format!(
						"这棵子树会被跳过。{} 请选择整个 {} 表达式。",
						step.explanation,
						self.root.find(step.node_id).expect("step exists").render()
					),
				));
			}
			if selected.value().is_some() {
				return Err(Feedback::new(
					FeedbackKind::AlreadyValue,
					"这里已经是一个值，无需再算。请选择一个运算表达式。",
				));
			}
			let inner: Option<NextStep> = next_step(selected, self.mode);
			return Err(
				if let Some(inner) = inner.filter(|inner| inner.node_id != node_id) {
					let expected: &Expr = selected.find(inner.node_id).expect("inner step exists");
					Feedback::new(
						FeedbackKind::NeedsInner,
						format!(
							"这一步不能跳过内部运算。请先计算 {}，再回到当前表达式。",
							expected.render()
						),
					)
				} else {
					let expected: &Expr = self.root.find(step.node_id).expect("step exists");
					Feedback::new(
						FeedbackKind::OutOfOrder,
						format!(
							"这里目前还不能计算，请先处理 {}。同优先级且不受短路限制的独立子式可以任选。",
							expected.render()
						),
					)
				},
			);
		}
		Ok(step)
	}

	/// Checks location before reading the answer; never advances the session.
	pub fn check_attempt(&self, node_id: NodeId, input: &str) -> Feedback {
		let step: NextStep = match self.selected_step(node_id) {
			Ok(step) => step,
			Err(feedback) => return feedback,
		};
		match &step.outcome {
			Err(EvalError::Limit(reason)) => Feedback::new(
				FeedbackKind::Unsupported,
				format!("超出首版支持范围：{reason} 这不是 Python 求值结果，也不计作答错。"),
			),
			Err(error) => {
				if Some(input.trim()) == error.name() {
					Feedback::new(
						FeedbackKind::Correct,
						format!("正确。{} 求值在这里终止。", step.explanation),
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
				let parsed: Result<Value, ParseError> = if self.logical {
					crate::logic::parse_truth(input).map(Value::Bool)
				} else {
					parse_value(input)
				};
				let actual: Value = match parsed {
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
					let protect_negative_base: bool = value.to_string().starts_with('-') && self.root.rows().iter().any(|(_, parent)| matches!(&parent.kind, super::ExprKind::Binary(super::BinaryOp::Power, left, _) if left.id == node_id));
					let replacement: String = if protect_negative_base {
						format!("({value})")
					} else {
						value.to_string()
					};
					self.root.replace(node_id, &value);
					self.surface.replace(node_id, &replacement, &self.root);
					if protect_negative_base {
						let reason: &str =
							" 负数作为幂的底数时，显示保留必要括号以免改变含义；该值已完成本步。";
						feedback.message.push_str(reason);
						self.history
							.last_mut()
							.expect("recorded step")
							.explanation
							.push_str(reason);
					}
				}
				self.finish_negative_literal();
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
