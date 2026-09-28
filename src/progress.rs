use std::{
	collections::BTreeMap,
	fs,
	io::{self, Write},
	path::{Path, PathBuf},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::{
	core::{RecordedAttempt, Session},
	logic::proof::Proof,
};

/// The progress format this build writes. Question sets did not change it: they added a name
/// beside the pointer, and a file written before them simply has no name there, which is
/// what a question belonging to no set says too. Version 3 dropped the evaluation strategy
/// the pointer was left in, because a student now short-circuits or keeps computing inside
/// one question instead of switching. Versions 1 and 2 both saved that strategy — 2 was
/// written for a short while when question sets first landed, then set back to 1 — so both
/// still load, see [`Strategic`].
const VERSION: u32 = 3;

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
	/// Keyed by teaching-rule version and content, so work recorded under other rules or for
	/// an edited question stays in the file without replaying into this one.
	pub sessions: BTreeMap<String, Vec<RecordedAttempt>>,
	pub proofs: BTreeMap<String, Vec<String>>,
}

/// A version-1 or version-2 file: the same pointer and records, plus the evaluation strategy
/// the pointer was left in. Loading one drops that strategy and keeps everything else
/// verbatim. Its evaluation records carry the strategy in their keys, written under earlier
/// teaching rules, so they stay in the file untouched rather than being re-keyed or merged, and
/// none of them replays: an evaluation question the pointer names has no record under today's
/// rules, so a random question, the question an ordered set resumes at, or one named with
/// `--exercise` opens from its start, while a bare launch whose pointer names a set question
/// draws a new random question, since nothing under today's rules says it was begun. Its proof
/// records replay as before.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Strategic {
	#[serde(rename = "version")]
	_version: u32,
	#[serde(default)]
	current_set: String,
	current: String,
	#[serde(rename = "mode")]
	_mode: Strategy,
	sessions: BTreeMap<String, Vec<RecordedAttempt>>,
	proofs: BTreeMap<String, Vec<String>>,
}

/// The two strategies a build that saved one could write. Read only so that a value no build ever
/// wrote is refused rather than guessed at; nothing keeps it.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Strategy {
	ShortCircuit,
	Eager,
}

impl From<Strategic> for Progress {
	fn from(file: Strategic) -> Self {
		Self {
			version: VERSION,
			current_set: file.current_set,
			current: file.current,
			sessions: file.sessions,
			proofs: file.proofs,
		}
	}
}

impl Default for Progress {
	fn default() -> Self {
		Self {
			version: VERSION,
			current_set: String::new(),
			current: String::new(),
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
		match declared.version {
			VERSION => serde_json::from_str(&text).map_err(io::Error::other),
			1 | 2 => serde_json::from_str::<Strategic>(&text)
				.map(Self::from)
				.map_err(io::Error::other),
			_ => Err(io::Error::other(
				"不支持的进度版本；请指定新的 --progress-file 或使用 --no-save。原文件未改动。",
			)),
		}
	}

	/// Point at this question and save its attempts. The set name travels with the ID, so
	/// the pointer names one question of one set and nothing else.
	pub fn record(&mut self, set: &str, question: &str, session: &Session) {
		self.current_set = set.into();
		self.current = question.into();
		self.sessions
			.insert(session.progress_key(), session.attempts().to_vec());
	}

	/// Point at this proof question and save its lines.
	pub fn record_proof(&mut self, set: &str, question: &str, proof: &Proof) {
		self.current_set = set.into();
		self.current = question.into();
		self.proofs
			.insert(proof.progress_key(), proof.commands().to_vec());
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

	/// The lines saved for exactly these premises and this conclusion.
	pub fn commands(&self, proof: &Proof) -> &[String] {
		self.proofs
			.get(&proof.progress_key())
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
