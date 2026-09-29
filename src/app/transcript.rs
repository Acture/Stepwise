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

/// One line of the record, typed by what it records so a front end can lay it out — a
/// terminal prints every kind as a line, a window may set each apart. The words are the
/// record's own and the same for every front end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Archived {
	/// The blank line between two questions' blocks: nothing after it belongs to the question before.
	Break,
	/// A block's heading: the language of an expression, or a proof and its goal.
	Heading(String),
	/// The bindings an expression question gives.
	Bindings(String),
	/// An expression as it stood before a step changed it.
	Expression(String),
	/// The marker for a step, a proof line or the whole question taken back.
	TakenBack(String),
	/// An accepted proof line, numbered and indented by its assumptions.
	ProofLine(String),
}

impl Archived {
	/// The line as a terminal prints it.
	pub fn text(&self) -> &str {
		match self {
			Self::Break => "",
			Self::Heading(text)
			| Self::Bindings(text)
			| Self::Expression(text)
			| Self::TakenBack(text)
			| Self::ProofLine(text) => text,
		}
	}
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
	pub fn sync(&mut self, lesson: &Lesson) -> Vec<Archived> {
		match lesson.task() {
			Task::Evaluation(practice) => self.evaluation(practice),
			Task::Proof(practice) => self.proof(practice),
		}
	}

	/// Start a new block for the question keyed `key`: what the previous block still owed the
	/// record, then a blank line between the two. The first block needs neither.
	fn open(&mut self, key: String, lines: &mut Vec<Archived>) {
		if !self.key.is_empty() {
			if !self.current.is_empty() {
				lines.push(Archived::Expression(std::mem::take(&mut self.current)));
			}
			lines.push(Archived::Break);
		}
		self.key = key;
		self.attempts.clear();
		self.lines = 0;
		self.current.clear();
	}

	fn evaluation(&mut self, practice: &Practice) -> Vec<Archived> {
		let key: String = practice.session().progress_key();
		let attempts: &[RecordedAttempt] = practice.session().attempts();
		let mut lines: Vec<Archived> = Vec::new();
		let from: usize = if self.key != key {
			self.open(key, &mut lines);
			lines.push(Archived::Heading(
				practice.session().language().label().into(),
			));
			let bindings: String = practice.question().assignments();
			if !bindings.is_empty() {
				lines.push(Archived::Bindings(bindings));
			}
			0
		} else if attempts.starts_with(&self.attempts) {
			self.attempts.len()
		} else {
			lines.push(Archived::TakenBack(format!(
				"↶ {}",
				taken_back(practice.report())
			)));
			attempts.len()
		};
		let history: &[HistoryEntry] = practice.session().history();
		for (index, entry) in history.iter().enumerate().skip(from) {
			let after: &str = history
				.get(index + 1)
				.map_or(practice.session().render(), |next| next.before.as_str());
			if entry.before != after {
				lines.push(Archived::Expression(entry.before.clone()));
			}
		}
		self.attempts = attempts.to_vec();
		self.current = practice.session().render().into();
		lines
	}

	fn proof(&mut self, practice: &ProofPractice) -> Vec<Archived> {
		let key: String = practice.proof().progress_key();
		let now: usize = practice.proof().lines().len();
		let mut lines: Vec<Archived> = Vec::new();
		if self.key != key {
			self.open(key, &mut lines);
			let mut opening: std::vec::IntoIter<String> = practice.opening().into_iter();
			lines.extend(opening.next().map(Archived::Heading));
			lines.extend(opening.map(Archived::ProofLine));
		} else {
			// Fewer lines than archived means one was taken back, and `appended` says so.
			let kind: fn(String) -> Archived = if now < self.lines {
				Archived::TakenBack
			} else {
				Archived::ProofLine
			};
			lines.extend(practice.appended(self.lines).into_iter().map(kind));
		}
		self.lines = now;
		lines
	}
}
