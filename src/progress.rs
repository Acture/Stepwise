use std::{
	collections::BTreeMap,
	fs,
	io::{self, Write},
	path::{Path, PathBuf},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::core::{EvaluationMode, RecordedAttempt, Session};

/// The progress format this build reads. Question sets did not change it: they added a name
/// beside the pointer, and a file written before them simply has no name there, which is
/// what a question belonging to no set says too. The records themselves — the work — are
/// read either way.
const VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
	version: u32,
	/// The set [`Progress::current`] came from. Empty means the question in hand belongs to
	/// no set — a generated or custom one — and a pointer written before sets existed reads
	/// that way too, so it names nothing this build can reopen and practice starts on a new
	/// question. Paired with the name it makes the pointer unambiguous: two sets may both
	/// hold a question named `q1` without ever reopening each other's work.
	#[serde(default)]
	pub current_set: String,
	pub current: String,
	pub mode: EvaluationMode,
	/// Key includes both strategy and source; progress must not cross semantic modes.
	pub sessions: BTreeMap<String, Vec<RecordedAttempt>>,
	pub proofs: BTreeMap<String, Vec<String>>,
}

impl Default for Progress {
	fn default() -> Self {
		Self {
			version: VERSION,
			current_set: String::new(),
			current: String::new(),
			mode: EvaluationMode::ShortCircuit,
			sessions: BTreeMap::new(),
			proofs: BTreeMap::new(),
		}
	}
}

impl Progress {
	pub fn default_path() -> io::Result<PathBuf> {
		ProjectDirs::from("dev", "Stepwise", "Stepwise")
			.map(|dirs| dirs.data_local_dir().join("progress.json"))
			.ok_or_else(|| {
				io::Error::new(
					io::ErrorKind::NotFound,
					"无法定位进度目录，请使用 --progress-file 或 --no-save",
				)
			})
	}

	pub fn load(path: &Path) -> io::Result<Self> {
		let text: String = match fs::read_to_string(path) {
			Ok(text) => text,
			Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
			Err(error) => return Err(error),
		};
		// The version is read on its own first. A file from another build has fields this one
		// does not know, and a raw deserializer complaint would bury the reason.
		#[derive(Deserialize)]
		struct Declared {
			version: u32,
		}
		let declared: Declared = serde_json::from_str(&text).map_err(io::Error::other)?;
		if declared.version != VERSION {
			return Err(io::Error::other(
				"不支持的进度版本；请指定新的 --progress-file 或使用 --no-save。原文件未改动。",
			));
		}
		serde_json::from_str(&text).map_err(io::Error::other)
	}

	/// Point at this question and save its attempts. The set name travels with the ID, so
	/// the pointer names one question of one set and nothing else.
	pub fn record(&mut self, set: &str, question: &str, session: &Session) {
		self.current_set = set.into();
		self.current = question.into();
		self.mode = session.mode();
		self.sessions
			.insert(session.progress_key(), session.attempts().to_vec());
	}

	/// True when the saved pointer names exactly this question of exactly this set.
	pub fn points_at(&self, set: &str, question: &str) -> bool {
		self.current_set == set && self.current == question
	}

	pub fn attempts(&self, session: &Session) -> &[RecordedAttempt] {
		self.sessions
			.get(&session.progress_key())
			.map(Vec::as_slice)
			.unwrap_or_default()
	}

	pub fn save(&self, path: &Path) -> io::Result<()> {
		let parent: &Path = path
			.parent()
			.filter(|path| !path.as_os_str().is_empty())
			.unwrap_or(Path::new("."));
		fs::create_dir_all(parent)?;
		let mut temporary: NamedTempFile = NamedTempFile::new_in(parent)?;
		serde_json::to_writer_pretty(&mut temporary, self).map_err(io::Error::other)?;
		temporary.write_all(b"\n")?;
		temporary.as_file().sync_all()?;
		temporary.persist(path).map_err(io::Error::other)?;
		Ok(())
	}
}
