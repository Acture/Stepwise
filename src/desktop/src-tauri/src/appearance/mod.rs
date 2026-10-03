//! How the window looks: skins, colour themes from the editor ecosystem, and faces. It is a
//! preference of this window and of nothing else — the desk, the view and the progress never
//! see it — so it lives here, in the shell, with the files it reads.
//!
//! The built-in skins belong to the page; the shell knows only their ids, as a look names
//! them. What it finds on the machine — the editors' themes, the installed families, the
//! editor font — is found once per launch, the first time the page asks.

mod colour;
mod editors;
mod fonts;
mod look;
mod roles;
mod settings;
mod themes;

use std::{
	ffi::OsStr,
	io,
	path::{Path, PathBuf},
	sync::{Mutex, MutexGuard, OnceLock},
	thread::{self, ScopedJoinHandle},
	time::Instant,
};

pub use look::{Appearance, Font, FontPick, Look, Theme};
use roles::Kind;
use settings::Settings;

/// Where an imported theme says it came from.
const IMPORTED: &str = "导入";

/// What the machine offers: the editors' themes, the installed families and the editor font.
pub struct Catalog {
	themes: Vec<Theme>,
	fonts: Vec<Font>,
	editor_font: Option<String>,
}

impl Catalog {
	/// Looks at this machine. The families take longest, so the themes are read meanwhile.
	pub fn find() -> Self {
		let started: Instant = Instant::now();
		let catalog: Self = thread::scope(|scope| {
			let fonts: ScopedJoinHandle<'_, Vec<Font>> = scope.spawn(fonts::installed);
			let themes: Vec<Theme> = themes::discover(&editors::shelves());
			let editor_font: Option<String> = fonts::editor_font(&editors::settings_files());
			Self {
				themes,
				fonts: fonts.join().expect("finding the families does not panic"),
				editor_font,
			}
		});
		eprintln!(
			"Stepwise：找到 {} 个配色主题、{} 个字体家族，用时 {:.0?}",
			catalog.themes.len(),
			catalog.fonts.len(),
			started.elapsed()
		);
		catalog
	}
}

/// The appearance behind the page's settings.
pub struct Wardrobe {
	find: fn() -> Catalog,
	catalog: OnceLock<Catalog>,
	held: Mutex<Held>,
}

struct Held {
	settings: Settings,
	/// Where the settings are written, or the sentence saying why they are not: there was no
	/// place for them, or the file there could not be read and is left as it was.
	path: Result<PathBuf, String>,
	message: Option<String>,
}

impl Held {
	/// Writes the settings after a change, and says so when they are not written.
	fn save(&mut self) {
		self.message =
			match &self.path {
				Ok(path) => self.settings.save(path).err().map(|error: io::Error| {
					format!("外观设置未能保存：{}。", crate::reason(&error))
				}),
				Err(why) => Some(why.clone()),
			};
	}
}

impl Wardrobe {
	/// The settings at `path`, or none saved when `path` holds why. A file that cannot be read
	/// opens the defaults, is never written over, and the settings say so.
	pub fn open(path: Result<PathBuf, String>, find: fn() -> Catalog) -> Self {
		let (settings, path): (Settings, Result<PathBuf, String>) = match path {
			Ok(path) => match Settings::load(&path) {
				Ok(settings) => (settings, Ok(path)),
				Err(error) => (
					Settings::default(),
					Err(format!(
						"外观设置 {} 无法读取：{}。本次不保存外观，原文件未改动。",
						path.display(),
						crate::reason(&error)
					)),
				),
			},
			Err(why) => (Settings::default(), Err(why)),
		};
		Self {
			find,
			catalog: OnceLock::new(),
			held: Mutex::new(Held {
				message: path.as_ref().err().cloned(),
				settings,
				path,
			}),
		}
	}

	/// A command that panicked while holding the settings has already taken the window down.
	fn hold(&self) -> MutexGuard<'_, Held> {
		self.held
			.lock()
			.expect("no command panics while holding the appearance")
	}

	pub fn appearance(&self) -> Appearance {
		let catalog: &Catalog = self.catalog.get_or_init(self.find);
		let held: MutexGuard<'_, Held> = self.hold();
		Appearance {
			look: held.settings.look.clone(),
			themes: catalog
				.themes
				.iter()
				.chain(&held.settings.themes)
				.cloned()
				.collect(),
			fonts: catalog.fonts.clone(),
			editor_font: catalog.editor_font.clone(),
			message: held.message.clone(),
		}
	}

	pub fn set_look(&self, look: Look) -> Appearance {
		{
			let mut held: MutexGuard<'_, Held> = self.hold();
			held.settings.look = look;
			held.save();
		}
		self.appearance()
	}

	/// Converts the VS Code theme file at `path`, keeps it and puts it on. A theme of the same
	/// name imported before is replaced. A file that cannot be converted changes nothing.
	pub fn import(&self, path: &Path) -> Appearance {
		let converted: Result<Theme, String> = themes::load(path).map(|loaded: themes::Loaded| {
			let kind: Kind = loaded
				.kind
				.unwrap_or_else(|| Kind::by_surface(&loaded.colours));
			let name: String = loaded.name.unwrap_or_else(|| {
				path.file_stem().map_or_else(
					|| path.display().to_string(),
					|stem: &OsStr| stem.to_string_lossy().into_owned(),
				)
			});
			Theme {
				id: format!("file:{name}"),
				tokens: roles::roles(&loaded.colours, kind),
				dark: kind.dark(),
				source: IMPORTED.into(),
				name,
			}
		});
		{
			let mut held: MutexGuard<'_, Held> = self.hold();
			match converted {
				Ok(theme) => {
					held.settings.look.skin = theme.id.clone();
					held.settings
						.themes
						.retain(|kept: &Theme| kept.id != theme.id);
					held.settings.themes.push(theme);
					held.save();
				}
				Err(why) => {
					held.message = Some(format!("无法导入主题 {}：{why}", path.display()));
				}
			}
		}
		self.appearance()
	}
}

#[cfg(test)]
mod tests {
	use std::{
		fs,
		sync::atomic::{AtomicUsize, Ordering},
	};

	use tempfile::TempDir;

	use super::*;
	use crate::appearance::{roles::tests::assert_readable, themes::tests::write};

	fn nothing_found() -> Catalog {
		Catalog {
			themes: Vec::new(),
			fonts: Vec::new(),
			editor_font: None,
		}
	}

	fn opened(path: &Path) -> Wardrobe {
		Wardrobe::open(Ok(path.to_path_buf()), nothing_found)
	}

	/// The machine is looked at once, however often the page asks.
	#[test]
	fn the_machine_is_looked_at_once_per_launch() {
		static LOOKS: AtomicUsize = AtomicUsize::new(0);
		fn counted() -> Catalog {
			LOOKS.fetch_add(1, Ordering::SeqCst);
			nothing_found()
		}
		let scratch: TempDir = TempDir::new().expect("temporary directory");
		let wardrobe: Wardrobe =
			Wardrobe::open(Ok(scratch.path().join("appearance.json")), counted);
		assert_eq!(LOOKS.load(Ordering::SeqCst), 0);
		wardrobe.appearance();
		wardrobe.set_look(Settings::default().look);
		wardrobe.import(&scratch.path().join("missing.json"));
		wardrobe.appearance();
		assert_eq!(LOOKS.load(Ordering::SeqCst), 1);
	}

	#[test]
	fn the_first_launch_wears_the_blackboard_and_writes_nothing() {
		let scratch: TempDir = TempDir::new().expect("temporary directory");
		let path: PathBuf = scratch.path().join("appearance.json");
		let appearance: Appearance = opened(&path).appearance();
		assert_eq!(appearance.look, Settings::default().look);
		assert_eq!(appearance.message, None);
		assert!(!path.exists());
	}

	#[test]
	fn a_look_is_saved_and_worn_at_the_next_launch() {
		let scratch: TempDir = TempDir::new().expect("temporary directory");
		let path: PathBuf = scratch.path().join("appearance.json");
		let look: Look = Look {
			skin: "notebook".into(),
			prose: FontPick::Family {
				family: "Georgia".into(),
			},
			formula: FontPick::Editor,
		};
		let appearance: Appearance = opened(&path).set_look(look.clone());
		assert_eq!((appearance.look, appearance.message), (look.clone(), None));
		assert_eq!(opened(&path).appearance().look, look);
	}

	/// A theme is kept as converted, so it survives its file moving away; importing one of the
	/// same name again replaces it.
	#[test]
	fn an_imported_theme_is_kept_worn_and_outlives_its_file() {
		let scratch: TempDir = TempDir::new().expect("temporary directory");
		let path: PathBuf = scratch.path().join("appearance.json");
		let file: PathBuf = scratch.path().join("download/night-color-theme.json");
		write(
			&scratch.path().join("download/base.json"),
			r##"{ "type": "hcDark", "colors": { "editor.background": "#101010" } }"##,
		);
		write(
			&file,
			r##"{
				// Exported from an editor.
				"name": "Night",
				"include": "./base.json",
				"colors": { "editor.foreground": "#e0e0e0", },
			}"##,
		);
		let wardrobe: Wardrobe = opened(&path);
		let appearance: Appearance = wardrobe.import(&file);
		assert_eq!(appearance.message, None);
		assert_eq!(appearance.look.skin, "file:Night");
		let theme: &Theme = &appearance.themes[0];
		assert_eq!(
			(
				theme.id.as_str(),
				theme.name.as_str(),
				theme.source.as_str(),
				theme.dark
			),
			("file:Night", "Night", "导入", true)
		);
		assert_eq!(theme.tokens["surface"], "#101010");
		assert_eq!(theme.tokens["ink"], "#e0e0e0");
		assert_readable(theme);

		fs::remove_dir_all(scratch.path().join("download")).expect("moved away");
		let reopened: Appearance = opened(&path).appearance();
		assert_eq!(reopened.look.skin, "file:Night");
		assert_eq!(reopened.themes, appearance.themes);

		// No type and no name: the kind from the ground, the name from the file.
		write(
			&scratch.path().join("Night.json"),
			r##"{ "colors": { "editor.background": "#fdf6e3" } }"##,
		);
		let again: Appearance = wardrobe.import(&scratch.path().join("Night.json"));
		assert_eq!(again.themes.len(), 1);
		assert!(!again.themes[0].dark);
		assert_readable(&again.themes[0]);
	}

	#[test]
	fn a_theme_that_cannot_be_imported_changes_nothing_and_says_why() {
		let scratch: TempDir = TempDir::new().expect("temporary directory");
		let path: PathBuf = scratch.path().join("appearance.json");
		write(&scratch.path().join("broken.json"), "{ \"colors\": ");
		write(&scratch.path().join("a.json"), r#"{ "include": "b.json" }"#);
		write(&scratch.path().join("b.json"), r#"{ "include": "a.json" }"#);
		let wardrobe: Wardrobe = opened(&path);
		for (file, reason) in [
			("missing.json", "missing.json："),
			("broken.json", "broken.json："),
			("a.json", "经 include 又回到了自己"),
		] {
			let file: PathBuf = scratch.path().join(file);
			let appearance: Appearance = wardrobe.import(&file);
			let said: String = appearance.message.expect("says why");
			assert!(
				said.starts_with(&format!("无法导入主题 {}：", file.display()))
					&& said.contains(reason),
				"{said}"
			);
			assert_eq!(appearance.look, Settings::default().look);
			assert!(appearance.themes.is_empty());
			assert!(!path.exists());
		}
	}

	/// A file this build does not understand is left alone for the whole launch, and every
	/// change says it is not saved.
	#[test]
	fn an_unreadable_settings_file_is_never_written_over() {
		let scratch: TempDir = TempDir::new().expect("temporary directory");
		let path: PathBuf = scratch.path().join("appearance.json");
		let text: &str = r#"{ "version": 99 }"#;
		fs::write(&path, text).expect("scratch file");
		let wardrobe: Wardrobe = opened(&path);
		let said: String = wardrobe.appearance().message.expect("says why");
		assert!(
			said.contains("不支持的外观设置版本 99")
				&& said.ends_with("本次不保存外观，原文件未改动。"),
			"{said}"
		);
		let changed: Appearance = wardrobe.set_look(Look {
			skin: "terminal".into(),
			..Settings::default().look
		});
		assert_eq!(changed.look.skin, "terminal");
		assert_eq!(changed.message, Some(said));
		assert_eq!(fs::read_to_string(&path).expect("still there"), text);
	}

	#[test]
	fn with_nowhere_to_save_a_change_still_shows_and_says_so() {
		let wardrobe: Wardrobe = Wardrobe::open(Err("无处保存。".into()), nothing_found);
		let changed: Appearance = wardrobe.set_look(Look {
			skin: "native".into(),
			..Settings::default().look
		});
		assert_eq!(
			(changed.look.skin.as_str(), changed.message.as_deref()),
			("native", Some("无处保存。"))
		);
	}
}
