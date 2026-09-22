use std::cmp::Ordering;

use crate::{
	core::ParseError,
	logic::proof::{Proof, ProofLine},
	progress::Progress,
};

/// The longest proof line a student can type; a front end never needs its own limit.
const DRAFT_LIMIT: usize = 2048;

/// One natural-deduction proof in progress, with the line being typed and the progress
/// snapshot its commands are saved in. Restoring, submitting, undoing and recording all
/// work with no terminal and no window; only writing the snapshot out is the caller's.
pub struct ProofPractice {
	key: String,
	proof: Proof,
	progress: Progress,
	input: String,
	feedback: String,
	feedback_good: bool,
}

impl ProofPractice {
	/// Replays the commands already saved for exactly these premises and this goal.
	pub fn new(proof: Proof, progress: Progress) -> Result<Self, ParseError> {
		let key: String = proof.progress_key();
		let commands: Vec<String> = progress.proofs.get(&key).cloned().unwrap_or_default();
		let proof: Proof = proof.replay(&commands)?;
		Ok(Self {
			key,
			proof,
			progress,
			input: String::new(),
			feedback: "下一行：公式 ; 规则 ; 引用行。Enter 检查，F1 查看规则。".into(),
			feedback_good: false,
		})
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
	pub fn feedback(&self) -> &str {
		&self.feedback
	}
	pub fn feedback_good(&self) -> bool {
		self.feedback_good
	}
	pub fn is_finished(&self) -> bool {
		self.proof.is_finished()
	}

	/// Show a message the front end owns, such as the rule reference or its key help.
	pub fn note(&mut self, message: impl Into<String>) {
		self.feedback = message.into();
		self.feedback_good = false;
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
				self.feedback = message;
				self.feedback_good = true;
				true
			}
			Err(error) => {
				self.feedback = error.to_string();
				self.feedback_good = false;
				false
			}
		}
	}

	/// Take back the last line, reopening whatever assumption it had discharged.
	pub fn undo(&mut self) -> bool {
		if !self.proof.undo() {
			return false;
		}
		self.feedback = "已撤销上一行，并恢复对应的假设作用域。".into();
		self.feedback_good = false;
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

	/// Copy this proof's commands into the progress snapshot.
	pub fn record(&mut self) {
		self.progress
			.proofs
			.insert(self.key.clone(), self.proof.commands().to_vec());
	}
}
