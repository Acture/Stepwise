use ratatui::text::{Line, Text};

use super::App;
use crate::core::{HistoryEntry, RecordedAttempt};

#[derive(Default)]
pub(super) struct Transcript {
	key: String,
	attempts: Vec<RecordedAttempt>,
	current: String,
}

impl Transcript {
	/// Archive changed display states; retain all semantic steps in the session.
	pub fn sync(&mut self, app: &App) -> Text<'static> {
		let key: String = app.session.progress_key();
		let attempts: &[RecordedAttempt] = app.session.attempts();
		let mut lines: Vec<Line<'static>> = Vec::new();
		let from: usize = if self.key != key {
			if !self.current.is_empty() {
				lines.push(Line::from(self.current.clone()));
				lines.push(Line::default());
			}
			lines.push(Line::from(app.session.language().label()));
			let bindings: String = app.exercises[app.index].assignments();
			if !bindings.is_empty() {
				lines.push(Line::from(bindings));
			}
			0
		} else if attempts.starts_with(&self.attempts) {
			self.attempts.len()
		} else {
			lines.push(Line::from(format!("↶ {}", app.feedback)));
			attempts.len()
		};
		let history: &[HistoryEntry] = app.session.history();
		for (index, entry) in history.iter().enumerate().skip(from) {
			let after: &str = history
				.get(index + 1)
				.map_or(app.session.render(), |next| next.before.as_str());
			if entry.before != after {
				lines.push(Line::from(entry.before.clone()));
			}
		}
		self.key = key;
		self.attempts = attempts.to_vec();
		self.current = app.session.render().into();
		Text::from(lines)
	}
}
