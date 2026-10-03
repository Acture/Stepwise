//! What the page is told about how the window may look, and what it hands back. Appearance is
//! a preference of this window, not teaching state: none of it reaches the desk or the view.
//! Under `cargo test` these types write their TypeScript declarations beside the desk's, in
//! `src/desktop/ui/src/lib/protocol`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The look a student chose. Saved by the shell beside the progress file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../../ui/src/lib/protocol/")
)]
#[serde(rename_all = "camelCase")]
pub struct Look {
	/// A built-in skin's id, which the page owns, or the id of one of [`Appearance::themes`].
	pub skin: String,
	/// The face for prose: headers, feedback, buttons.
	pub prose: FontPick,
	/// The face for formulas and proof lines.
	pub formula: FontPick,
}

/// Where a face comes from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../../ui/src/lib/protocol/")
)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum FontPick {
	/// Whatever the skin sets.
	Skin,
	/// The editor font set in VS Code or a fork of it, see [`Appearance::editor_font`].
	Editor,
	/// An installed family, by the name CSS knows it by.
	Family { family: String },
}

/// A colour theme from the editor ecosystem, turned into the roles this window draws with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../../ui/src/lib/protocol/")
)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
	/// Stable across launches: where it came from and its label, so a saved look finds it again.
	pub id: String,
	pub name: String,
	/// Where it was found, for the settings list: "VS Code", "Cursor", "导入" and so on.
	pub source: String,
	pub dark: bool,
	/// Values for the page's role tokens, keyed by the custom property's name without `--`:
	/// surface, ink, ink-faded, ink-bright, mark-write, mark-wrong, mark-point, mark-good,
	/// quiet-mark, tray, scrollbar, focus-ring, ink-glow.
	pub tokens: BTreeMap<String, String>,
}

/// An installed family the student may pick.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../../ui/src/lib/protocol/")
)]
#[serde(rename_all = "camelCase")]
pub struct Font {
	pub family: String,
	pub monospace: bool,
	/// Whether it draws every logic symbol a formula may hold (∧ ∨ → ↔ ¬ ⊥); `None` where it
	/// was not checked.
	pub logic: Option<bool>,
}

/// Everything the settings need, and the look in force.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../../ui/src/lib/protocol/")
)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
	pub look: Look,
	pub themes: Vec<Theme>,
	pub fonts: Vec<Font>,
	/// The first family of the editor font VS Code or a fork is set to, when one is set.
	pub editor_font: Option<String>,
	/// Something the settings have to say, such as a theme file that could not be read.
	pub message: Option<String>,
}
