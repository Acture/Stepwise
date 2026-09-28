use super::{Lesson, Notice, Practice, ProofPractice, Report, Task};
use crate::core::{HistoryEntry, RecordedAttempt};

/// A front end's cursor over the lesson's history: which display states and proof lines it
/// has already archived. The rule is a teaching one — archive an expression only when the
/// displayed text actually changed, archive each proof line once, and mark a step that was
/// taken back — so a terminal and a window archive exactly the same lines, whichever kind of
/// question is in hand and however the student moves between them.
///
/// The marker for a step taken back is worded here, like the one
/// [`super::ProofPractice::appended`] writes. An archived line is the permanent record of
/// what happened, so every front end writes the same one; a [`Notice`] is a live reason a
/// front end answers in its own words. The two share their words with [`Notice::Undone`] and
/// [`Notice::Restarted`] today and are free to stop: taking the marker from whatever the
/// front end had just said would make the shared record front-end-specific, which is the one
/// thing the rule above forbids.
#[derive(Default)]
pub struct Transcript {
	/// The progress key of the question whose block is open; empty before the first one.
	key: String,
	attempts: Vec<RecordedAttempt>,
	/// Proof lines already archived for the open block.
	lines: usize,
	/// The expression on screen, owed to the record when its block closes; a proof owes
	/// nothing, since each of its lines is archived as it is accepted.
	current: String,
}

/// What the record says about a step that was taken back. Only an undo and a restart
/// shorten the attempts, so every other reason reads as an undo — spelled out rather than
/// waved through with a wildcard, because a reason added later has to be decided here too
/// instead of quietly inheriting a marker that would misreport the record.
fn taken_back(report: &Report) -> &'static str {
	match report {
		Report::Notice(Notice::Restarted) => "已重新开始本题。",
		Report::Notice(
			Notice::Undone
			| Notice::Start
			| Notice::DraftOpen
			| Notice::FinalPair
			| Notice::NoNextStep
			| Notice::ModeSwitched(_)
			| Notice::CourseEnded
			| Notice::ProofStart
			| Notice::ProofUndone,
		)
		| Report::Taught { .. }
		| Report::Note(_) => "已撤销上一步。",
	}
}

impl Transcript {
	/// The lines to archive since the last call; empty when nothing changed.
	pub fn sync(&mut self, lesson: &Lesson) -> Vec<String> {
		match lesson.task() {
			Task::Evaluation(practice) => self.evaluation(practice),
			Task::Proof(practice) => self.proof(practice),
		}
	}

	/// Start a new block for the question keyed `key`: what the previous block still owed the
	/// record, then a blank line between the two. The first block needs neither.
	fn open(&mut self, key: String, lines: &mut Vec<String>) {
		if !self.key.is_empty() {
			if !self.current.is_empty() {
				lines.push(std::mem::take(&mut self.current));
			}
			lines.push(String::new());
		}
		self.key = key;
		self.attempts.clear();
		self.lines = 0;
		self.current.clear();
	}

	fn evaluation(&mut self, practice: &Practice) -> Vec<String> {
		let key: String = practice.session().progress_key();
		let attempts: &[RecordedAttempt] = practice.session().attempts();
		let mut lines: Vec<String> = Vec::new();
		let from: usize = if self.key != key {
			self.open(key, &mut lines);
			lines.push(practice.session().language().label().into());
			let bindings: String = practice.question().assignments();
			if !bindings.is_empty() {
				lines.push(bindings);
			}
			0
		} else if attempts.starts_with(&self.attempts) {
			self.attempts.len()
		} else {
			lines.push(format!("↶ {}", taken_back(practice.report())));
			attempts.len()
		};
		let history: &[HistoryEntry] = practice.session().history();
		for (index, entry) in history.iter().enumerate().skip(from) {
			let after: &str = history
				.get(index + 1)
				.map_or(practice.session().render(), |next| next.before.as_str());
			if entry.before != after {
				lines.push(entry.before.clone());
			}
		}
		self.attempts = attempts.to_vec();
		self.current = practice.session().render().into();
		lines
	}

	fn proof(&mut self, practice: &ProofPractice) -> Vec<String> {
		let key: String = practice.proof().progress_key();
		let mut lines: Vec<String> = Vec::new();
		if self.key != key {
			self.open(key, &mut lines);
			lines.extend(practice.opening());
		} else {
			lines.extend(practice.appended(self.lines));
		}
		self.lines = practice.proof().lines().len();
		lines
	}
}
