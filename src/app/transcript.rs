use super::{Notice, Practice, Report};
use crate::core::{HistoryEntry, RecordedAttempt};

/// A front end's cursor over the practice history: which display states it has already
/// archived. The rule is a teaching one — archive a state only when the displayed text
/// actually changed, and mark a step that was taken back — so a terminal and a window
/// archive exactly the same lines.
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
	key: String,
	attempts: Vec<RecordedAttempt>,
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
	pub fn sync(&mut self, practice: &Practice) -> Vec<String> {
		let key: String = practice.session().progress_key();
		let attempts: &[RecordedAttempt] = practice.session().attempts();
		let mut lines: Vec<String> = Vec::new();
		let from: usize = if self.key != key {
			if !self.current.is_empty() {
				lines.push(self.current.clone());
				lines.push(String::new());
			}
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
		self.key = key;
		self.attempts = attempts.to_vec();
		self.current = practice.session().render().into();
		lines
	}
}
