//! The look a student chose and the themes they imported, kept in `appearance.json` beside
//! the progress file. It is written when the student changes the look or imports a theme, and
//! at no other time.

use std::{
	fs,
	io::{self, Write},
	path::Path,
};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use super::{FontPick, Look, Theme};

const VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
	version: u32,
	pub look: Look,
	/// Imported themes, kept as they were converted, so a theme outlives its file.
	pub themes: Vec<Theme>,
}

impl Default for Settings {
	/// The blackboard in its own faces.
	fn default() -> Self {
		Self {
			version: VERSION,
			look: Look {
				skin: "blackboard".into(),
				prose: FontPick::Skin,
				formula: FontPick::Skin,
			},
			themes: Vec::new(),
		}
	}
}

impl Settings {
	/// The settings at `path`, or the defaults where there is no file yet. A file from a build
	/// with another version is refused rather than guessed at.
	pub fn load(path: &Path) -> io::Result<Self> {
		let text: String = match fs::read_to_string(path) {
			Ok(text) => text,
			Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
			Err(error) => return Err(error),
		};
		// The version is read on its own first, so a file from another build is refused for
		// its version and not for a field this build does not know.
		#[derive(Deserialize)]
		struct Declared {
			version: u32,
		}
		let declared: Declared = serde_json::from_str(&text).map_err(io::Error::other)?;
		if declared.version != VERSION {
			return Err(io::Error::other(format!(
				"不支持的外观设置版本 {}",
				declared.version
			)));
		}
		serde_json::from_str(&text).map_err(io::Error::other)
	}

	/// Writes the settings whole, so a write that fails halfway leaves the last file standing.
	pub fn save(&self, path: &Path) -> io::Result<()> {
		let parent: &Path = path
			.parent()
			.filter(|parent: &&Path| !parent.as_os_str().is_empty())
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

#[cfg(test)]
mod tests {
	use std::{collections::BTreeMap, path::PathBuf};

	use tempfile::TempDir;

	use super::*;

	#[test]
	fn settings_come_back_as_they_were_written() {
		let scratch: TempDir = TempDir::new().expect("temporary directory");
		let path: PathBuf = scratch.path().join("nested/appearance.json");
		assert_eq!(
			Settings::load(&path).expect("no file yet"),
			Settings::default()
		);
		let settings: Settings = Settings {
			look: Look {
				skin: "file:Night".into(),
				prose: FontPick::Editor,
				formula: FontPick::Family {
					family: "JetBrains Mono".into(),
				},
			},
			themes: vec![Theme {
				id: "file:Night".into(),
				name: "Night".into(),
				source: "导入".into(),
				dark: true,
				tokens: BTreeMap::from([("surface".into(), "#000000".into())]),
			}],
			..Settings::default()
		};
		settings.save(&path).expect("written");
		assert_eq!(Settings::load(&path).expect("read back"), settings);
		let written: String = fs::read_to_string(&path).expect("written");
		assert!(written.contains(r#""version": 1"#), "{written}");
		assert!(written.contains(r#""kind": "family""#), "{written}");
	}

	#[test]
	fn a_file_of_another_version_or_shape_is_refused() {
		let scratch: TempDir = TempDir::new().expect("temporary directory");
		let path: PathBuf = scratch.path().join("appearance.json");
		for (text, reason) in [
			(r#"{ "version": 2, "look": 1 }"#, "不支持的外观设置版本 2"),
			(r#"{ "version": 1, "look": {} }"#, "missing field"),
			(r#"{ "version": 1, "extra": 0 }"#, "unknown field"),
			("not json", "expected"),
		] {
			fs::write(&path, text).expect("scratch file");
			let error: String = Settings::load(&path).expect_err(text).to_string();
			assert!(error.contains(reason), "{text}: {error}");
		}
	}
}
