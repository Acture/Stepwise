use super::Practice;
use crate::core::{HistoryEntry, RecordedAttempt};

/// A front end's cursor over the practice history: which display states it has already
/// archived. The rule is a teaching one — archive a state only when the displayed text
/// actually changed, and mark a step that was taken back — so a terminal and a window
/// archive exactly the same lines.
#[derive(Default)]
pub struct Transcript {
	key: String,
	attempts: Vec<RecordedAttempt>,
	current: String,
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
			lines.push(format!("↶ {}", practice.feedback()));
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
