//! Colour themes as VS Code and its forks keep them: an extension's `package.json` names each
//! theme it contributes, and a theme file is JSON with comments that may `include` another,
//! its own colours laid over the included ones. Discovery reads what the editors on this
//! machine have installed; nothing here writes to an editor's folders.

use std::{
	collections::{BTreeMap, HashMap},
	ffi::OsStr,
	fs::{self, DirEntry},
	io,
	path::{Path, PathBuf},
};

use jsonc_parser::ParseOptions;
use serde::{Deserialize, de::DeserializeOwned};

use super::{
	Theme,
	editors::Shelf,
	roles::{self, Kind},
};

/// How many includes a chain may follow; VS Code's own follow three.
const DEPTH: usize = 8;

/// A file as VS Code reads it: JSON with comments and trailing commas.
pub fn jsonc<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
	let text: String = fs::read_to_string(path).map_err(|error: io::Error| error.to_string())?;
	jsonc_parser::parse_to_serde_value(&text, &ParseOptions::default())
		.map_err(|error| error.to_string())
}

#[derive(Deserialize)]
struct ThemeFile {
	name: Option<String>,
	#[serde(rename = "type")]
	kind: Option<String>,
	include: Option<String>,
	/// `null` leaves a colour as the included file set it, and `"default"` takes it back to
	/// VS Code's default, as VS Code reads them.
	#[serde(default)]
	colors: BTreeMap<String, Option<String>>,
}

/// A theme file with its include chain followed.
#[derive(Debug)]
pub struct Loaded {
	/// The name the file itself gives, never one from a file it includes.
	pub name: Option<String>,
	/// The kind the nearest file in the chain declares.
	pub kind: Option<Kind>,
	pub colours: BTreeMap<String, String>,
}

/// Reads the theme file at `path` and every file it includes, each include relative to the
/// file naming it. Says why when a file cannot be read or the chain loops or runs too deep.
pub fn load(path: &Path) -> Result<Loaded, String> {
	follow(path, &mut Vec::new())
}

fn follow(path: &Path, chain: &mut Vec<PathBuf>) -> Result<Loaded, String> {
	let file: PathBuf = fs::canonicalize(path)
		.map_err(|error: io::Error| format!("{}：{error}", path.display()))?;
	if chain.contains(&file) {
		return Err(format!("{} 经 include 又回到了自己", path.display()));
	}
	if chain.len() > DEPTH {
		return Err(format!("include 嵌套超过 {DEPTH} 层"));
	}
	chain.push(file);
	let theme: ThemeFile =
		jsonc(path).map_err(|error: String| format!("{}：{error}", path.display()))?;
	let mut loaded: Loaded = match &theme.include {
		Some(include) => follow(
			&path.parent().unwrap_or(Path::new(".")).join(include),
			chain,
		)?,
		None => Loaded {
			name: None,
			kind: None,
			colours: BTreeMap::new(),
		},
	};
	loaded.name = theme.name;
	if let Some(kind) = theme.kind.as_deref().and_then(Kind::declared) {
		loaded.kind = Some(kind);
	}
	for (key, value) in theme.colors {
		match value.as_deref() {
			Some("default") => {
				loaded.colours.remove(&key);
			}
			Some(colour) => {
				loaded.colours.insert(key, colour.into());
			}
			None => {}
		}
	}
	Ok(loaded)
}

#[derive(Deserialize)]
struct Manifest {
	name: String,
	publisher: Option<String>,
	contributes: Option<Contributes>,
}

#[derive(Deserialize)]
struct Contributes {
	#[serde(default)]
	themes: Vec<Contribution>,
}

/// Each field is optional here so that one entry missing one leaves out only itself.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Contribution {
	label: Option<String>,
	ui_theme: Option<String>,
	path: Option<String>,
}

/// A `package.nls.json` value: a string, or a string with a translator's comment.
#[derive(Deserialize)]
#[serde(untagged)]
enum Localized {
	Text(String),
	Commented { message: String },
}

/// A label, with a `%key%` placeholder looked up in the extension's `package.nls.json`.
fn label(text: &str, strings: &HashMap<String, Localized>) -> String {
	let key: Option<&str> = text
		.strip_prefix('%')
		.and_then(|rest: &str| rest.strip_suffix('%'));
	match key.and_then(|key: &str| strings.get(key)) {
		Some(Localized::Text(text) | Localized::Commented { message: text }) => text.clone(),
		None => text.into(),
	}
}

/// Every colour theme the extensions on these shelves contribute, each once, under the first
/// editor that ships it: the settings shelve themes by their source, and a fork's copy of VS
/// Code's own themes would otherwise part them from the ones only VS Code has. A theme is known
/// by its extension and its label, not its id, which the forks rename (Cursor's "Default
/// Dark+" is VS Code's "Dark+"). A theme that cannot be read is left out and said on stderr.
pub fn discover(shelves: &[Shelf]) -> Vec<Theme> {
	let mut found: Vec<Theme> = Vec::new();
	for shelf in shelves {
		for (id, name, kind, file) in contributions(&shelf.dir) {
			if found.iter().any(|theme: &Theme| theme.id == id) {
				continue;
			}
			match load(&file) {
				Ok(loaded) => found.push(Theme {
					id,
					name,
					source: shelf.editor.into(),
					dark: kind.dark(),
					tokens: roles::roles(&loaded.colours, kind),
				}),
				Err(error) => eprintln!("Stepwise：主题 {name} 无法读取：{error}"),
			}
		}
	}
	found
}

/// The themes the extensions in one folder contribute: id, label, kind and file, in the
/// folders' name order. An extension the editor has replaced by a newer version is listed in
/// the folder's `.obsolete` until the editor deletes it, and is skipped.
fn contributions(dir: &Path) -> Vec<(String, String, Kind, PathBuf)> {
	let Ok(entries) = fs::read_dir(dir) else {
		return Vec::new();
	};
	let obsolete: HashMap<String, bool> = jsonc(&dir.join(".obsolete")).unwrap_or_default();
	let mut extensions: Vec<PathBuf> = entries
		.filter_map(|entry: io::Result<DirEntry>| entry.ok().map(|entry: DirEntry| entry.path()))
		.filter(|path: &PathBuf| {
			let name: String = path
				.file_name()
				.map(|name: &OsStr| name.to_string_lossy().into_owned())
				.unwrap_or_default();
			!name.starts_with('.') && !obsolete.get(&name).copied().unwrap_or(false)
		})
		.collect();
	extensions.sort();
	let mut listed: Vec<(String, String, Kind, PathBuf)> = Vec::new();
	for extension in extensions {
		// Most extensions contribute no theme; the text says so before it is parsed.
		let manifest: PathBuf = extension.join("package.json");
		let Ok(text) = fs::read_to_string(&manifest) else {
			continue;
		};
		if !text.contains("\"themes\"") {
			continue;
		}
		let manifest: Manifest =
			match jsonc_parser::parse_to_serde_value(&text, &ParseOptions::default()) {
				Ok(manifest) => manifest,
				Err(error) => {
					eprintln!("Stepwise：扩展 {} 无法读取：{error}", manifest.display());
					continue;
				}
			};
		let Some(themes) = manifest
			.contributes
			.map(|contributes: Contributes| contributes.themes)
		else {
			continue;
		};
		let strings: HashMap<String, Localized> =
			jsonc(&extension.join("package.nls.json")).unwrap_or_default();
		let identity: String = match &manifest.publisher {
			Some(publisher) => format!("{publisher}.{}", manifest.name),
			None => manifest.name.clone(),
		};
		for theme in themes {
			// A theme for a kind this window does not know is left to the editor.
			let (Some(text), Some(kind), Some(path)) = (
				theme.label,
				theme.ui_theme.as_deref().and_then(Kind::ui_theme),
				theme.path,
			) else {
				continue;
			};
			let name: String = label(&text, &strings);
			listed.push((
				format!("{identity}/{name}"),
				name,
				kind,
				extension.join(path),
			));
		}
	}
	listed
}

#[cfg(test)]
pub(super) mod tests {
	use tempfile::TempDir;

	use super::*;
	use crate::appearance::roles::tests::assert_readable;

	pub fn write(path: &Path, text: &str) {
		fs::create_dir_all(path.parent().expect("a folder")).expect("scratch folder");
		fs::write(path, text).expect("scratch file");
	}

	/// VS Code's default themes as a fork might ship them: a label from `package.nls.json`
	/// (in both of its forms), a chain of includes with comments and trailing commas, and a
	/// pair of files that include each other.
	fn defaults(folder: &Path, id: &str) {
		write(
			&folder.join("package.json"),
			&format!(
				r#"{{
					"name": "theme-defaults",
					"publisher": "vscode",
					"contributes": {{
						"themes": [
							{{ "id": "{id}", "label": "%darkPlus%", "uiTheme": "vs-dark", "path": "./themes/dark_plus.json" }},
							{{ "label": "%light%", "uiTheme": "vs", "path": "./themes/light.json" }},
							{{ "label": "Loop", "uiTheme": "vs-dark", "path": "./themes/a.json" }},
							{{ "label": "Pathless", "uiTheme": "vs-dark" }},
							{{ "label": "Unknown kind", "uiTheme": "vs-sepia", "path": "./themes/light.json" }},
						],
						"iconThemes": [],
					}},
				}}"#
			),
		);
		write(
			&folder.join("package.nls.json"),
			r#"{ "darkPlus": "Dark+", "light": { "message": "Light", "comment": ["a label"] } }"#,
		);
		write(
			&folder.join("themes/dark_plus.json"),
			r##"{
				// Dark+ lays its colours over Dark.
				"name": "Dark+",
				"include": "./base/dark_vs.json",
				"colors": {
					"editor.foreground": "#d4d4d4", /* over Dark's */
					"editorCursor.foreground": "default",
					"focusBorder": null,
				},
			}"##,
		);
		write(
			&folder.join("themes/base/dark_vs.json"),
			r##"{
				"type": "dark",
				"colors": {
					"editor.background": "#1e1e1e",
					"editor.foreground": "#bbbbbb",
					"editorCursor.foreground": "#ffffff",
					"focusBorder": "#5ec4d6",
				},
			}"##,
		);
		write(
			&folder.join("themes/light.json"),
			r##"{ "colors": { "editor.background": "#fffffe" } }"##,
		);
		write(&folder.join("themes/a.json"), r#"{ "include": "b.json" }"#);
		write(
			&folder.join("themes/b.json"),
			r#"{ "include": "./a.json" }"#,
		);
	}

	#[test]
	fn a_theme_is_its_own_colours_over_those_it_includes() {
		let scratch: TempDir = TempDir::new().expect("temporary directory");
		defaults(scratch.path(), "Dark+");
		let loaded: Loaded = load(&scratch.path().join("themes/dark_plus.json")).expect("loads");
		assert_eq!(loaded.name.as_deref(), Some("Dark+"));
		assert_eq!(loaded.kind, Some(Kind::Dark));
		let colours: Vec<(&str, &str)> = loaded
			.colours
			.iter()
			.map(|(key, value): (&String, &String)| (key.as_str(), value.as_str()))
			.collect();
		assert_eq!(
			colours,
			[
				("editor.background", "#1e1e1e"),
				("editor.foreground", "#d4d4d4"),
				("focusBorder", "#5ec4d6"),
			]
		);
	}

	#[test]
	fn an_include_chain_that_loops_or_runs_deep_is_refused() {
		let scratch: TempDir = TempDir::new().expect("temporary directory");
		defaults(scratch.path(), "Dark+");
		let looped: String = load(&scratch.path().join("themes/a.json")).expect_err("a loop");
		assert!(
			looped.ends_with("a.json 经 include 又回到了自己"),
			"{looped}"
		);
		for step in 0..=DEPTH {
			write(
				&scratch.path().join(format!("deep/{step}.json")),
				&format!(r#"{{ "include": "{}.json" }}"#, step + 1),
			);
		}
		write(
			&scratch.path().join(format!("deep/{}.json", DEPTH + 1)),
			"{}",
		);
		let deep: String = load(&scratch.path().join("deep/0.json")).expect_err("too deep");
		assert_eq!(deep, format!("include 嵌套超过 {DEPTH} 层"));
		assert!(load(&scratch.path().join("deep/1.json")).is_ok());
	}

	/// Two editors that ship the same default themes under different ids, and a user folder
	/// with an extension an update left behind.
	#[test]
	fn discovery_lists_each_theme_once_under_the_first_editor_with_it() {
		let scratch: TempDir = TempDir::new().expect("temporary directory");
		let code: PathBuf = scratch.path().join("code/extensions");
		let cursor: PathBuf = scratch.path().join("cursor/extensions");
		let user: PathBuf = scratch.path().join("home/.cursor/extensions");
		defaults(&code.join("theme-defaults"), "Dark+");
		defaults(&cursor.join("theme-defaults"), "Default Dark+");
		write(
			&code.join("git/package.json"),
			r#"{ "name": "git", "publisher": "vscode" }"#,
		);
		for version in ["1.0.0", "1.1.0"] {
			let folder: PathBuf = user.join(format!("someone.night-{version}"));
			write(
				&folder.join("package.json"),
				&format!(
					r#"{{ "name": "night", "publisher": "someone", "contributes": {{ "themes": [
						{{ "label": "Night {version}", "uiTheme": "hc-black", "path": "night.json" }}
					] }} }}"#
				),
			);
			write(
				&folder.join("night.json"),
				r##"{ "colors": { "editor.background": "#000000" } }"##,
			);
		}
		write(
			&user.join(".obsolete"),
			r#"{ "someone.night-1.0.0": true }"#,
		);

		let found: Vec<Theme> = discover(&[
			Shelf {
				editor: "VS Code",
				dir: code,
			},
			Shelf {
				editor: "Cursor",
				dir: cursor,
			},
			Shelf {
				editor: "Cursor",
				dir: user,
			},
			Shelf {
				editor: "Nowhere",
				dir: scratch.path().join("missing"),
			},
		]);
		let listed: Vec<(&str, &str, &str, bool)> = found
			.iter()
			.map(|theme: &Theme| {
				(
					theme.id.as_str(),
					theme.name.as_str(),
					theme.source.as_str(),
					theme.dark,
				)
			})
			.collect();
		assert_eq!(
			listed,
			[
				("vscode.theme-defaults/Dark+", "Dark+", "VS Code", true),
				("vscode.theme-defaults/Light", "Light", "VS Code", false),
				("someone.night/Night 1.1.0", "Night 1.1.0", "Cursor", true),
			]
		);
		for theme in &found {
			assert_readable(theme);
		}
		assert_eq!(found[0].tokens["surface"], "#1e1e1e");
		assert_eq!(found[0].tokens["mark-point"], "#5ec4d6");
	}
}
