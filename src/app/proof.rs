use std::cmp::Ordering;

use super::{Notice, Report};
use crate::{
	core::ParseError,
	exercises::ProofQuestion,
	logic::proof::{Proof, ProofLine},
	progress::Progress,
};

/// The longest proof line a student can type; a front end never needs its own limit.
const DRAFT_LIMIT: usize = 2048;

/// One natural-deduction proof in progress, with the line being typed and the progress
/// snapshot its commands are saved in. Restoring, submitting, undoing and recording all
/// work with no terminal and no window; only writing the snapshot out is the caller's.
pub struct ProofPractice {
	question: ProofQuestion,
	proof: Proof,
	progress: Progress,
	input: String,
	report: Report,
}

impl ProofPractice {
	/// Opens this question, replaying the commands already saved for exactly these premises
	/// and this conclusion.
	pub fn new(question: ProofQuestion, progress: Progress) -> Result<Self, ParseError> {
		let initial: Proof = question.proof()?;
		let proof: Proof = initial.clone().replay(progress.commands(&initial))?;
		Ok(Self {
			question,
			proof,
			progress,
			input: String::new(),
			report: Report::Notice(Notice::ProofStart),
		})
	}

	pub fn question(&self) -> &ProofQuestion {
		&self.question
	}
	pub fn proof(&self) -> &Proof {
		&self.proof
	}
	pub fn progress(&self) -> &Progress {
		&self.progress
	}
	pub fn input(&self) -> &str {
		&self.input
	}
	/// What to show after the last operation; see [`super::Practice::report`].
	pub fn report(&self) -> &Report {
		&self.report
	}
	pub fn is_finished(&self) -> bool {
		self.proof.is_finished()
	}

	/// Show a sentence the front end owns, such as the rule reference or its key help.
	pub fn note(&mut self, message: impl Into<String>) {
		self.report = Report::Note(message.into());
	}

	/// Report a reason the lesson around this proof raised, such as the end of a course.
	pub(super) fn notify(&mut self, notice: Notice) {
		self.report = Report::Notice(notice);
	}

	/// Add one character to the draft. False when it is a control character or would pass
	/// the draft limit.
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

	/// Check the typed line. A rejected line keeps the draft and adds no proof line;
	/// semantic equivalence never stands in for a rule.
	pub fn submit(&mut self) -> bool {
		match self.proof.submit(&self.input) {
			Ok(message) => {
				self.input.clear();
				self.report = Report::Taught {
					message,
					accepted: true,
				};
				true
			}
			Err(error) => {
				self.report = Report::Taught {
					message: error.to_string(),
					accepted: false,
				};
				false
			}
		}
	}

	/// Take back the last line, reopening whatever assumption it had discharged.
	pub fn undo(&mut self) -> bool {
		if !self.proof.undo() {
			return false;
		}
		self.report = Report::Notice(Notice::ProofUndone);
		true
	}

	/// The header and every line already proved, for a front end opening its transcript.
	pub fn opening(&self) -> Vec<String> {
		let mut lines: Vec<String> = vec![format!("自然演绎 · 目标：{}", self.proof.goal)];
		lines.extend(self.lines_from(0));
		lines
	}

	/// What to archive after a step that changed a proof of `before` lines: the new lines,
	/// or the marker for a line taken back.
	pub fn appended(&self, before: usize) -> Vec<String> {
		match self.proof.lines().len().cmp(&before) {
			Ordering::Greater => self.lines_from(before),
			Ordering::Less => vec![format!("↶ 撤销第 {before} 行及其假设作用域变更。")],
			Ordering::Equal => Vec::new(),
		}
	}

	/// Proof lines from `start`, numbered and indented by how deep their assumptions run.
	pub fn lines_from(&self, start: usize) -> Vec<String> {
		self.proof
			.lines()
			.iter()
			.enumerate()
			.skip(start)
			.map(|(index, line): (usize, &ProofLine)| {
				format!(
					"{} {}{} [{} {}]",
					index + 1,
					"│ ".repeat(line.scope.len()),
					line.formula,
					line.rule,
					line.references
						.iter()
						.map(ToString::to_string)
						.collect::<Vec<String>>()
						.join(",")
				)
			})
			.collect()
	}

	/// Point the progress snapshot at this question and copy its commands in.
	pub fn record(&mut self) {
		self.progress
			.record_proof(&self.question.set, &self.question.name, &self.proof);
	}
}
