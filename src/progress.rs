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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
	version: u32,
	pub current: String,
	pub mode: EvaluationMode,
	/// Key includes both strategy and source; progress must not cross semantic modes.
	pub sessions: BTreeMap<String, Vec<RecordedAttempt>>,
	pub proofs: BTreeMap<String, Vec<String>>,
}

impl Default for Progress {
	fn default() -> Self {
		Self {
			version: 1,
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
		let file: fs::File = match fs::File::open(path) {
			Ok(file) => file,
			Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
			Err(error) => return Err(error),
		};
		let progress: Self = serde_json::from_reader(file).map_err(io::Error::other)?;
		if progress.version != 1 {
			return Err(io::Error::other(
				"不支持的进度版本；请指定新的 --progress-file 或使用 --no-save。原文件未改动。",
			));
		}
		Ok(progress)
	}

	pub fn record(&mut self, exercise_id: &str, session: &Session) {
		self.current = exercise_id.into();
		self.mode = session.mode();
		self.sessions
			.insert(session.progress_key(), session.attempts().to_vec());
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
