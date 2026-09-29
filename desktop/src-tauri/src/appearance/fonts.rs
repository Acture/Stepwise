//! The faces a student may pick: the families installed on this machine, and the one their
//! editor is set to.

use std::{collections::BTreeMap, path::PathBuf};

use fontdb::{Database, FaceInfo, ID, Style};
use serde::Deserialize;
use ttf_parser::GlyphId;

use super::{Font, themes::jsonc};

/// Every symbol a formula may hold that a text face might not draw.
const LOGIC: [char; 6] = ['∧', '∨', '→', '↔', '¬', '⊥'];

/// The installed families by the name CSS knows them by, each once, sorted. A family the
/// system hides (its name starts with a dot) cannot be asked for by name and is left out.
/// Whether a family draws the logic symbols is read from its upright regular face: every
/// family is checked, which costs a tenth of finding them (390 families: 45 ms, then 10 ms).
/// A family is monospaced when every face says so, or when its regular face sets every
/// printable ASCII character in one width: Monaco, for one, never says so.
pub fn installed() -> Vec<Font> {
	let mut database: Database = Database::new();
	database.load_system_fonts();
	let mut families: BTreeMap<&str, Vec<&FaceInfo>> = BTreeMap::new();
	for face in database.faces() {
		if let Some((family, _)) = face.families.first()
			&& !family.starts_with('.')
		{
			families.entry(family).or_default().push(face);
		}
	}
	families
		.into_iter()
		.map(|(family, faces): (&str, Vec<&FaceInfo>)| {
			let shown: Option<Regular> = faces
				.iter()
				.min_by_key(|face: &&&FaceInfo| {
					(face.style != Style::Normal, face.weight.0.abs_diff(400))
				})
				.and_then(|face: &&FaceInfo| regular(&database, face.id));
			Font {
				family: family.into(),
				monospace: faces.iter().all(|face: &&FaceInfo| face.monospaced)
					|| shown.as_ref().is_some_and(|shown: &Regular| shown.even),
				logic: shown.map(|shown: Regular| shown.logic),
			}
		})
		.collect()
}

/// What a family's regular face shows of it.
struct Regular {
	/// Its character map holds every logic symbol.
	logic: bool,
	/// Every printable ASCII character is there, in one width.
	even: bool,
}

/// `None` when the face cannot be read.
fn regular(database: &Database, face: ID) -> Option<Regular> {
	database
		.with_face_data(face, |data: &[u8], index: u32| {
			ttf_parser::Face::parse(data, index)
				.ok()
				.map(|face: ttf_parser::Face<'_>| {
					let widths: Vec<Option<u16>> = (' '..='~')
						.map(|character: char| {
							face.glyph_index(character)
								.and_then(|glyph: GlyphId| face.glyph_hor_advance(glyph))
						})
						.collect();
					Regular {
						logic: LOGIC
							.iter()
							.all(|symbol: &char| face.glyph_index(*symbol).is_some()),
						even: widths[0].is_some()
							&& widths.iter().all(|width: &Option<u16>| *width == widths[0]),
					}
				})
		})
		.flatten()
}

/// Names CSS gives no one family: the generic families and the system-font aliases.
const GENERIC: [&str; 15] = [
	"serif",
	"sans-serif",
	"monospace",
	"cursive",
	"fantasy",
	"system-ui",
	"ui-serif",
	"ui-sans-serif",
	"ui-monospace",
	"ui-rounded",
	"math",
	"emoji",
	"fangsong",
	"-apple-system",
	"blinkmacsystemfont",
];

/// A CSS `font-family` list's names, unquoted. A comma inside quotes belongs to the name, and
/// an unquoted name's inner spaces collapse to one, as CSS reads them.
fn families(list: &str) -> Vec<String> {
	let mut names: Vec<String> = Vec::new();
	let mut name: String = String::new();
	let mut quote: Option<char> = None;
	for character in list.chars() {
		match (quote, character) {
			(Some(open), _) if character == open => quote = None,
			(Some(_), _) => name.push(character),
			(None, '\'' | '"') => quote = Some(character),
			(None, ',') => names.push(std::mem::take(&mut name)),
			(None, _) => name.push(character),
		}
	}
	names.push(name);
	names
		.iter()
		.map(|name: &String| name.split_whitespace().collect::<Vec<&str>>().join(" "))
		.filter(|name: &String| !name.is_empty())
		.collect()
}

/// The first family in a CSS list that names one face.
fn first_family(list: &str) -> Option<String> {
	families(list)
		.into_iter()
		.find(|name: &String| !GENERIC.contains(&name.to_lowercase().as_str()))
}

#[derive(Deserialize)]
struct EditorSettings {
	#[serde(rename = "editor.fontFamily")]
	font_family: Option<String>,
}

/// The first family of the editor font set in the first of these settings files that sets
/// one. A settings file that cannot be read sets none.
pub fn editor_font(files: &[PathBuf]) -> Option<String> {
	files.iter().find_map(|file: &PathBuf| {
		jsonc::<EditorSettings>(file)
			.ok()
			.and_then(|settings: EditorSettings| settings.font_family)
			.and_then(|list: String| first_family(&list))
	})
}

#[cfg(test)]
mod tests {
	use tempfile::TempDir;

	use super::*;
	use crate::appearance::themes::tests::write;

	#[test]
	fn a_css_font_list_is_read_as_css_reads_it() {
		assert_eq!(
			families("'JetBrains Mono', Menlo, monospace"),
			["JetBrains Mono", "Menlo", "monospace"]
		);
		assert_eq!(
			families(r#""Fira Code, Retina",  Source   Code Pro ,, ui-monospace"#),
			["Fira Code, Retina", "Source Code Pro", "ui-monospace"]
		);
		assert_eq!(
			first_family("'JetBrains Mono', Menlo, monospace").as_deref(),
			Some("JetBrains Mono")
		);
		assert_eq!(
			first_family("\"Fira Code, Retina\", monospace").as_deref(),
			Some("Fira Code, Retina")
		);
		assert_eq!(
			first_family("-apple-system, BlinkMacSystemFont, Menlo").as_deref(),
			Some("Menlo")
		);
		assert_eq!(first_family("monospace, 'ui-monospace'"), None);
		assert_eq!(first_family(""), None);
	}

	/// The first editor that sets a font is the one taken; a settings file that is JSON with
	/// comments is read, and one that cannot be read or sets only generic names is passed over.
	#[test]
	fn the_editor_font_comes_from_the_first_editor_that_sets_one() {
		let scratch: TempDir = TempDir::new().expect("temporary directory");
		let file = |name: &str, text: &str| -> PathBuf {
			let path: PathBuf = scratch.path().join(name).join("User/settings.json");
			write(&path, text);
			path
		};
		let files: Vec<PathBuf> = vec![
			scratch.path().join("Code/User/settings.json"),
			file("Broken", "{ not settings"),
			file("Generic", r#"{ "editor.fontFamily": "monospace" }"#),
			file(
				"Cursor",
				r#"{
					// The editor font, with a fallback.
					"[markdown]": { "editor.fontFamily": "Georgia" },
					"editor.fontFamily": "'JetBrains Mono', Menlo, monospace",
				}"#,
			),
			file("Windsurf", r#"{ "editor.fontFamily": "Menlo" }"#),
		];
		assert_eq!(editor_font(&files).as_deref(), Some("JetBrains Mono"));
		assert_eq!(editor_font(&files[..3]), None);
	}

	#[cfg(target_os = "macos")]
	#[test]
	fn the_installed_families_are_found_and_menlo_lacks_up_tack_and_monaco_is_monospaced() {
		let fonts: Vec<Font> = installed();
		let menlo: &Font = fonts
			.iter()
			.find(|font: &&Font| font.family == "Menlo")
			.expect("macOS ships Menlo");
		assert!(menlo.monospace);
		assert_eq!(menlo.logic, Some(false));
		let monaco: &Font = fonts
			.iter()
			.find(|font: &&Font| font.family == "Monaco")
			.expect("macOS ships Monaco");
		assert!(monaco.monospace, "one width, though it never says so");
		let helvetica: &Font = fonts
			.iter()
			.find(|font: &&Font| font.family == "Helvetica")
			.expect("macOS ships Helvetica");
		assert!(!helvetica.monospace);
		let checked: usize = fonts
			.iter()
			.filter(|font: &&Font| font.logic.is_some())
			.count();
		let drawing: usize = fonts
			.iter()
			.filter(|font: &&Font| font.logic == Some(true))
			.count();
		let monospace: usize = fonts.iter().filter(|font: &&Font| font.monospace).count();
		println!(
			"{} families, {monospace} monospaced, {checked} checked, {drawing} draw every logic symbol",
			fonts.len()
		);
	}
}
