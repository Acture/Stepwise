use std::ops::Range;

use super::{Course, Notice, Report};
use crate::{
	core::{EvaluationMode, ExprKind, Feedback, NodeId, ParseError, RecordedAttempt, Session},
	exercises::Exercise,
	progress::Progress,
};

/// The longest draft a student can type; a front end never needs its own limit.
const DRAFT_LIMIT: usize = 2048;

/// One expression question in progress: the course it belongs to, the teaching session, the
/// node the student has selected, the answer being typed, and the progress snapshot.
///
/// Every operation here is input-independent — a click, a keystroke and a window button all
/// call the same method. Selecting only opens a draft and reports feedback; it never fills
/// in, reveals or submits an answer.
pub struct Practice {
	course: Course,
	session: Session,
	progress: Progress,
	selected: NodeId,
	input: String,
	editing: Option<NodeId>,
	report: Report,
}

impl Practice {
	/// Starts the course's current question, replaying whatever this exact question, mode
	/// and valuation already have saved.
	pub fn new(
		course: Course,
		progress: Progress,
		mode: EvaluationMode,
	) -> Result<Self, ParseError> {
		let initial: Session = course.current().session(mode)?;
		let attempts: &[RecordedAttempt] = progress.attempts(&initial);
		let session: Session = initial.replay(attempts)?;
		let selected: NodeId = session.root().id;
		let editing: Option<NodeId> = session.final_binary_step();
		Ok(Self {
			course,
			session,
			progress,
			selected,
			input: String::new(),
			editing,
			report: Report::Notice(if editing.is_some() {
				Notice::FinalPair
			} else {
				Notice::Start
			}),
		})
	}

	pub fn course(&self) -> &Course {
		&self.course
	}
	pub fn question(&self) -> &Exercise {
		self.course.current()
	}
	pub fn session(&self) -> &Session {
		&self.session
	}
	pub fn progress(&self) -> &Progress {
		&self.progress
	}
	pub fn selected(&self) -> NodeId {
		self.selected
	}
	/// The node whose blank is open, if any.
	pub fn draft(&self) -> Option<NodeId> {
		self.editing
	}
	pub fn input(&self) -> &str {
		&self.input
	}
	/// What to show after the last operation. A [`Notice`] arrives as a reason alone, so
	/// no key name and no input device is spelled anywhere below a front end.
	pub fn report(&self) -> &Report {
		&self.report
	}

	/// Every source range the open draft stands for. One typed answer replaces all of them,
	/// so a front end shows the blanks without re-deriving the substitution rule.
	pub fn draft_spans(&self) -> Vec<(NodeId, Range<usize>)> {
		let (_, ranges) = self.session.render_with_ranges();
		self.editing
			.into_iter()
			.flat_map(|id| self.session.replacement_ids(id))
			.map(|id| (id, ranges[&id].clone()))
			.collect()
	}

	/// The node each byte of the rendered expression belongs to: the deepest one still
	/// waiting for a step, so a part that already holds a value is not selectable. A front
	/// end maps its own coordinates through this instead of walking the tree itself, and
	/// one call answers a whole redraw.
	pub fn node_owners(&self) -> Vec<Option<NodeId>> {
		let (source, ranges) = self.session.render_with_ranges();
		let mut owners: Vec<Option<(usize, NodeId)>> = vec![None; source.len()];
		for (depth, node) in self.session.root().rows() {
			if node.value().is_some() {
				continue;
			}
			for index in ranges[&node.id].clone() {
				// A deeper node wins the byte, and the last of equal depth, as display order.
				if owners[index].is_none_or(|(covering, _)| depth >= covering) {
					owners[index] = Some((depth, node.id));
				}
			}
		}
		owners
			.into_iter()
			.map(|owner| owner.map(|(_, node_id)| node_id))
			.collect()
	}

	/// The nodes still waiting for a step, in display order; the root when none remain.
	fn selectable(&self) -> Vec<NodeId> {
		let mut ids: Vec<NodeId> = self
			.session
			.root()
			.rows()
			.into_iter()
			.filter(|(_, node)| node.value().is_none())
			.map(|(_, node)| node.id)
			.collect();
		if ids.is_empty() {
			ids.push(self.session.root().id);
		}
		ids
	}

	pub fn select_next(&mut self) {
		self.step_selection(true);
	}
	pub fn select_previous(&mut self) {
		self.step_selection(false);
	}

	fn step_selection(&mut self, forward: bool) {
		let ids: Vec<NodeId> = self.selectable();
		let index: usize = ids.iter().position(|id| *id == self.selected).unwrap_or(0);
		self.selected = ids[if forward {
			(index + 1).min(ids.len() - 1)
		} else {
			index.saturating_sub(1)
		}];
		self.cancel();
	}

	/// Select a node: a finished pair of brackets comes off at once and is recorded as a
	/// step with no answer, anything else opens a blank. True when the session advanced.
	pub fn select(&mut self, node_id: NodeId) -> bool {
		if self.editing == Some(node_id) {
			return false;
		}
		self.selected = node_id;
		self.cancel();
		if matches!(
			self.session.root().find(node_id).map(|node| &node.kind),
			Some(ExprKind::Group(_))
		) {
			let feedback: Feedback = self.session.remove_group(node_id);
			let accepted: bool = feedback.accepted();
			self.report = feedback.into();
			if accepted {
				self.open_final_pair();
			}
			return accepted;
		}
		match self.session.check_selection(node_id) {
			Ok(()) => {
				self.editing = Some(node_id);
				self.report = Report::Notice(Notice::DraftOpen);
			}
			Err(feedback) => self.report = feedback.into(),
		}
		false
	}

	/// Close the blank and drop the draft; the session is untouched.
	pub fn cancel(&mut self) {
		self.editing = None;
		self.input.clear();
	}

	/// Add one character to the draft. False when it is a control character or would pass
	/// the draft limit, so the front end can tell an ignored keystroke from an accepted one.
	pub fn type_character(&mut self, character: char) -> bool {
		if character.is_control() || self.input.len() + character.len_utf8() > DRAFT_LIMIT {
			return false;
		}
		self.input.push(character);
		true
	}

	/// Insert pasted text, dropping control characters and stopping at the draft limit.
	pub fn paste(&mut self, text: &str) {
		for character in text.chars().filter(|character| !character.is_control()) {
			if !self.type_character(character) {
				break;
			}
		}
	}

	pub fn backspace(&mut self) {
		self.input.pop();
	}
	pub fn clear_input(&mut self) {
		self.input.clear();
	}

	/// Check the typed answer where the blank is open, or select first when none is. A wrong
	/// answer keeps the draft and leaves the session exactly where it was.
	pub fn submit(&mut self) -> bool {
		let Some(node_id) = self.editing else {
			return self.select(self.selected);
		};
		let feedback: Feedback = self.session.submit(node_id, &self.input);
		let accepted: bool = feedback.accepted();
		self.report = feedback.into();
		if accepted {
			self.cancel();
			self.open_final_pair();
		}
		accepted
	}

	/// A whole binary expression over two known values opens its blank with no further
	/// selection. The student still types and submits the value.
	fn open_final_pair(&mut self) {
		if let Some(node_id) = self.session.final_binary_step() {
			self.selected = node_id;
			self.editing = Some(node_id);
		}
	}

	/// Name the next step to consider, or report that none is left to name.
	pub fn hint(&mut self) {
		self.report = match self.session.hint() {
			Some(message) => Report::Taught {
				message,
				accepted: false,
			},
			None => Report::Notice(Notice::NoNextStep),
		};
	}

	/// Show a sentence the front end owns, such as its own key help.
	pub fn note(&mut self, message: impl Into<String>) {
		self.report = Report::Note(message.into());
	}

	/// Take back the last student step, including a whole batch of substituted occurrences.
	pub fn undo(&mut self) -> bool {
		if !self.session.undo() {
			return false;
		}
		self.selected = self.session.root().id;
		self.cancel();
		self.open_final_pair();
		self.report = Report::Notice(Notice::Undone);
		true
	}

	/// Start this question again from its source; saved progress is rewritten on the next
	/// [`Practice::record`].
	pub fn reset(&mut self) -> Result<bool, ParseError> {
		let session: Session = self.course.current().session(self.session.mode())?;
		self.session = session;
		self.selected = self.session.root().id;
		self.cancel();
		self.open_final_pair();
		self.report = Report::Notice(Notice::Restarted);
		Ok(true)
	}

	/// Switch evaluation strategy. The two strategies keep separate progress, so the current
	/// attempts are recorded under the old key before the session is rebuilt.
	pub fn toggle_mode(&mut self) -> Result<bool, ParseError> {
		self.record();
		let mode: EvaluationMode = self.session.mode().toggled();
		let course: Course = self.course.clone();
		self.restart(course, mode)?;
		self.report = Report::Notice(Notice::ModeSwitched(mode));
		Ok(true)
	}

	/// Move to the next question the course supplies. False when an ordered set has ended.
	pub fn next_question(&mut self) -> Result<bool, ParseError> {
		self.record();
		let mut course: Course = self.course.clone();
		if !course.forward()? {
			self.report = Report::Notice(Notice::CourseEnded);
			return Ok(false);
		}
		self.restart(course, self.session.mode())?;
		Ok(true)
	}

	/// Go back to a question drawn earlier in this run, restoring its recorded attempts.
	pub fn previous_question(&mut self) -> Result<bool, ParseError> {
		self.record();
		let mut course: Course = self.course.clone();
		course.backward();
		self.restart(course, self.session.mode())?;
		Ok(true)
	}

	/// Replace the whole practice, keeping the progress snapshot. Leaves it untouched when
	/// the new question cannot be built.
	fn restart(&mut self, course: Course, mode: EvaluationMode) -> Result<(), ParseError> {
		*self = Self::new(course, self.progress.clone(), mode)?;
		Ok(())
	}

	/// Copy this question's attempts into the progress snapshot. Writing that snapshot out
	/// belongs to the caller, so the app layer never touches the file system.
	pub fn record(&mut self) {
		self.progress
			.record(&self.course.current().id, &self.session);
	}
}
