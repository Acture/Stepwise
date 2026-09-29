//! A VS Code colour theme turned into the roles this window draws with, readable by
//! construction. Each role walks the theme keys that mean something like it and takes the first
//! of the theme's colours that clears its floor against the surface, or else the theme's first
//! moved until it clears; only a theme with no colour of its own for a role takes VS Code's
//! defaults for those keys, the same way. No theme is trusted to be legible as it stands.

use std::collections::BTreeMap;

use Fallback::{Hex, Of, Unset};

use super::colour::{Rgb, Rgba};

/// WCAG contrast floors against the surface. Ink is what a student reads, so it clears AAA.
const INK: f64 = 7.0;
/// Faded ink is text too, but secondary: large-text AA, and always quieter than ink.
const FADED: f64 = 3.0;
/// Where faded ink is made by fading ink, it fades only this far.
const FADED_MADE: f64 = 4.5;
/// A mark is drawn around or under ink and must be seen as a colour of its own.
const MARK: f64 = 3.0;
/// A mark that is a shade of grey reads as more ink, not as a mark.
const CHROMA: f64 = 0.12;
/// A mark this near ink or another mark reads as that one.
const APART: f64 = 0.2;
/// A rule or a scrollbar only has to be seen.
const QUIET: f64 = 1.5;
/// Ink on the tray, where the tray's buttons turn from faded ink to ink.
const TRAY_INK: f64 = 4.5;

/// The four kinds of VS Code theme, which pick VS Code's defaults for keys a theme leaves out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
	Light,
	Dark,
	HighContrastDark,
	HighContrastLight,
}

impl Kind {
	/// A contribution's `uiTheme`.
	pub fn ui_theme(name: &str) -> Option<Self> {
		match name {
			"vs" => Some(Self::Light),
			"vs-dark" => Some(Self::Dark),
			"hc-black" => Some(Self::HighContrastDark),
			"hc-light" => Some(Self::HighContrastLight),
			_ => None,
		}
	}

	/// The `type` a theme file may declare.
	pub fn declared(name: &str) -> Option<Self> {
		match name {
			"light" => Some(Self::Light),
			"dark" => Some(Self::Dark),
			"hc" | "hcDark" => Some(Self::HighContrastDark),
			"hcLight" => Some(Self::HighContrastLight),
			_ => None,
		}
	}

	/// The kind a theme that declares none looks like, judged by its editor background.
	pub fn by_surface(colours: &BTreeMap<String, String>) -> Self {
		let surface: Option<Rgb> = colours
			.get("editor.background")
			.and_then(|hex: &String| Rgba::hex(hex))
			.map(|colour: Rgba| colour.over(Rgb::WHITE));
		match surface {
			Some(surface) if surface.contrast(Rgb::WHITE) > surface.contrast(Rgb::BLACK) => {
				Self::Dark
			}
			_ => Self::Light,
		}
	}

	pub fn dark(self) -> bool {
		matches!(self, Self::Dark | Self::HighContrastDark)
	}

	/// VS Code's default for one of the keys read here, from its colour registry and the
	/// git and terminal contributions, as its workbench bundle declares them.
	fn fallback(self, key: &str) -> Fallback {
		let [light, dark, contrast_dark, contrast_light]: [Fallback; 4] = match key {
			"editor.background" => [
				Hex("#ffffff"),
				Hex("#1e1e1e"),
				Hex("#000000"),
				Hex("#ffffff"),
			],
			"editor.foreground" => [
				Hex("#333333"),
				Hex("#bbbbbb"),
				Hex("#ffffff"),
				Of("foreground", 1.0),
			],
			"foreground" => [
				Hex("#616161"),
				Hex("#cccccc"),
				Hex("#ffffff"),
				Hex("#292929"),
			],
			"descriptionForeground" => [
				Hex("#717171"),
				Of("foreground", 0.7),
				Of("foreground", 0.7),
				Of("foreground", 0.7),
			],
			"editorLineNumber.foreground" => [
				Hex("#237893"),
				Hex("#858585"),
				Hex("#ffffff"),
				Hex("#292929"),
			],
			"editorCursor.foreground" => [
				Hex("#000000"),
				Hex("#aeafad"),
				Hex("#ffffff"),
				Hex("#0f4a85"),
			],
			"terminal.ansiYellow" => [
				Hex("#949800"),
				Hex("#e5e510"),
				Hex("#cdcd00"),
				Hex("#949800"),
			],
			"editorWarning.foreground" => [
				Hex("#bf8803"),
				Hex("#cca700"),
				Hex("#ffd370"),
				Hex("#895503"),
			],
			"editorError.foreground" => [
				Hex("#e51400"),
				Hex("#f14c4c"),
				Hex("#f48771"),
				Hex("#b5200d"),
			],
			"errorForeground" => [
				Hex("#a1260d"),
				Hex("#f48771"),
				Hex("#f48771"),
				Hex("#b5200d"),
			],
			"terminal.ansiRed" => [
				Hex("#cd3131"),
				Hex("#cd3131"),
				Hex("#cd0000"),
				Hex("#cd3131"),
			],
			"focusBorder" => [
				Hex("#0090f1"),
				Hex("#007fd4"),
				Hex("#f38518"),
				Hex("#006bbd"),
			],
			"textLink.foreground" => [
				Hex("#006ab1"),
				Hex("#3794ff"),
				Hex("#21a6ff"),
				Hex("#0f4a85"),
			],
			"terminal.ansiCyan" => [
				Hex("#0598bc"),
				Hex("#11a8cd"),
				Hex("#00cdcd"),
				Hex("#0598bc"),
			],
			"terminal.ansiBlue" => [
				Hex("#0451a5"),
				Hex("#2472c8"),
				Hex("#0000ee"),
				Hex("#0451a5"),
			],
			"terminal.ansiGreen" => [
				Hex("#107c10"),
				Hex("#0dbc79"),
				Hex("#00cd00"),
				Hex("#136c13"),
			],
			"gitDecoration.addedResourceForeground" => [
				Hex("#587c0c"),
				Hex("#81b88b"),
				Hex("#a1e3ad"),
				Hex("#374e06"),
			],
			"contrastBorder" => [Unset, Unset, Hex("#6fc3df"), Hex("#0f4a85")],
			"panel.border" => [
				Hex("#80808059"),
				Hex("#80808059"),
				Of("contrastBorder", 1.0),
				Of("contrastBorder", 1.0),
			],
			"editorGroup.border" => [
				Hex("#e7e7e7"),
				Hex("#444444"),
				Of("contrastBorder", 1.0),
				Of("contrastBorder", 1.0),
			],
			"widget.border" => [
				Unset,
				Unset,
				Of("contrastBorder", 1.0),
				Of("contrastBorder", 1.0),
			],
			"sideBar.background" => [
				Hex("#f3f3f3"),
				Hex("#252526"),
				Hex("#000000"),
				Hex("#ffffff"),
			],
			"panel.background" => [Of("editor.background", 1.0); 4],
			"statusBar.background" => [Hex("#007acc"), Hex("#007acc"), Unset, Unset],
			"scrollbarSlider.background" => [
				Hex("#64646466"),
				Hex("#79797966"),
				Of("contrastBorder", 0.6),
				Of("contrastBorder", 0.4),
			],
			_ => [Unset; 4],
		};
		match self {
			Self::Light => light,
			Self::Dark => dark,
			Self::HighContrastDark => contrast_dark,
			Self::HighContrastLight => contrast_light,
		}
	}
}

/// VS Code's default for a key a theme leaves out: a colour, another key's colour at an
/// opacity, or nothing.
#[derive(Clone, Copy)]
enum Fallback {
	Hex(&'static str),
	Of(&'static str, f64),
	Unset,
}

/// A theme's colours, as the theme set them and as VS Code fills in what it did not.
struct Palette<'a> {
	colours: &'a BTreeMap<String, String>,
	kind: Kind,
}

impl Palette<'_> {
	/// The theme's own colour for a key: the one it set, or the one it set for the key VS
	/// Code's default names, such as its editor background for the panel's.
	fn own(&self, key: &str) -> Option<Rgba> {
		let set: Option<Rgba> = self
			.colours
			.get(key)
			.and_then(|hex: &String| Rgba::hex(hex));
		set.or_else(|| match self.kind.fallback(key) {
			Of(other, opacity) => self.own(other).map(|colour: Rgba| colour.faded(opacity)),
			Hex(_) | Unset => None,
		})
	}

	/// What VS Code draws for a key the theme says nothing about.
	fn default(&self, key: &str) -> Option<Rgba> {
		match self.kind.fallback(key) {
			Hex(hex) => Rgba::hex(hex),
			Of(other, opacity) => self
				.own(other)
				.or_else(|| self.default(other))
				.map(|colour: Rgba| colour.faded(opacity)),
			Unset => None,
		}
	}

	/// The keys' colours laid over `ground`, the theme's own apart from VS Code's defaults.
	fn candidates(&self, keys: &[&str], ground: Rgb) -> Candidates {
		let over = |colours: &dyn Fn(&str) -> Option<Rgba>| -> Vec<Rgb> {
			keys.iter()
				.filter_map(|key: &&str| colours(key))
				.map(|colour: Rgba| colour.over(ground))
				.collect()
		};
		Candidates {
			own: over(&|key: &str| self.own(key)),
			defaults: over(&|key: &str| self.default(key)),
		}
	}
}

/// A role's colours in the order they are tried.
struct Candidates {
	own: Vec<Rgb>,
	defaults: Vec<Rgb>,
}

impl Candidates {
	/// Only the colours `keep` keeps, unless it keeps none at all.
	fn keeping(self, keep: impl Fn(Rgb) -> bool) -> Self {
		let kept = |colours: &[Rgb]| -> Vec<Rgb> {
			colours
				.iter()
				.copied()
				.filter(|colour: &Rgb| keep(*colour))
				.collect()
		};
		let kept: Self = Self {
			own: kept(&self.own),
			defaults: kept(&self.defaults),
		};
		if kept.own.is_empty() && kept.defaults.is_empty() {
			self
		} else {
			kept
		}
	}

	/// The theme's first own colour that clears, else its first moved toward `anchor` until it
	/// does: a theme keeps its own hue wherever it has one. Only a theme with no colour of its
	/// own for the role takes VS Code's defaults, the same way. `anchor` itself clears.
	fn choose(&self, anchor: Rgb, clears: impl Fn(Rgb) -> bool) -> Rgb {
		for colours in [&self.own, &self.defaults] {
			if let Some(colour) = colours.iter().find(|colour: &&Rgb| clears(**colour)) {
				return *colour;
			}
			if let Some(first) = colours.first() {
				return move_until(*first, anchor, &clears);
			}
		}
		anchor
	}
}

/// The least share of the way from `from` to `to` whose mix satisfies `clears`, which `to`
/// itself must satisfy. Mixing is continuous, so halving finds the edge; the mix returned is
/// always on the side that clears.
fn move_until(from: Rgb, to: Rgb, clears: impl Fn(Rgb) -> bool) -> Rgb {
	if clears(from) {
		return from;
	}
	let (mut short, mut far): (f64, f64) = (0.0, 1.0);
	for _ in 0..24 {
		let middle: f64 = (short + far) / 2.0;
		if clears(from.mix(to, middle)) {
			far = middle;
		} else {
			short = middle;
		}
	}
	from.mix(to, far)
}

/// The tokens for a theme's colours: values for the page's role custom properties, keyed by
/// their names without `--`.
pub fn roles(colours: &BTreeMap<String, String>, kind: Kind) -> BTreeMap<String, String> {
	let palette: Palette<'_> = Palette { colours, kind };
	// Light ink on a dark ground or dark ink on a light one, as the theme's kind says: the
	// end ink is moved toward when it must be, and the end the ground is.
	let (inkward, groundward): (Rgb, Rgb) = if kind.dark() {
		(Rgb::WHITE, Rgb::BLACK)
	} else {
		(Rgb::BLACK, Rgb::WHITE)
	};

	// A ground too near the middle carries no ink at the floor whichever way the ink goes,
	// and a ground on the wrong side of its kind carries none either: it is moved toward its
	// kind's own end until it does.
	let surface: Rgb = palette
		.candidates(&["editor.background"], groundward)
		.choose(groundward, |surface: Rgb| inkward.contrast(surface) >= INK);
	let against = |colour: Rgb| -> f64 { colour.contrast(surface) };

	let ink: Rgb = palette
		.candidates(&["editor.foreground", "foreground"], surface)
		.choose(inkward, |colour: Rgb| against(colour) >= INK);
	// Faded ink is a quieter colour of the theme's own, or else its ink faded toward the
	// ground as far as it still reads: VS Code's grey sits oddly on a coloured ground.
	let quieter = |colour: Rgb| -> bool { (FADED..against(ink)).contains(&against(colour)) };
	let faded: Rgb = palette
		.candidates(
			&["editorLineNumber.foreground", "descriptionForeground"],
			surface,
		)
		.own
		.into_iter()
		.find(|colour: &Rgb| quieter(*colour))
		.unwrap_or_else(|| move_until(surface, ink, |colour: Rgb| against(colour) >= FADED_MADE));
	let bright: Rgb = ink.mix(inkward, 0.5);

	// Each mark is a colour, and a colour apart from ink and from the marks before it; many
	// themes draw the cursor in their ink.
	let mut taken: Vec<Rgb> = vec![ink];
	let mut mark = |keys: &[&str]| -> Rgb {
		let chosen: Rgb = palette
			.candidates(keys, surface)
			.keeping(|colour: Rgb| {
				colour.chroma() >= CHROMA
					&& taken
						.iter()
						.all(|other: &Rgb| colour.apart(*other) >= APART)
			})
			.choose(ink, |colour: Rgb| against(colour) >= MARK);
		taken.push(chosen);
		chosen
	};
	let point: Rgb = mark(&[
		"focusBorder",
		"textLink.foreground",
		"terminal.ansiCyan",
		"terminal.ansiBlue",
	]);
	let write: Rgb = mark(&[
		"editorCursor.foreground",
		"terminal.ansiYellow",
		"editorWarning.foreground",
	]);
	let wrong: Rgb = mark(&[
		"editorError.foreground",
		"errorForeground",
		"terminal.ansiRed",
	]);
	let good: Rgb = mark(&[
		"terminal.ansiGreen",
		"gitDecoration.addedResourceForeground",
	]);
	let quiet = |keys: &[&str]| -> Rgb {
		palette
			.candidates(keys, surface)
			.choose(ink, |colour: Rgb| against(colour) >= QUIET)
	};

	// The tray holds buttons in faded ink that turn to ink under the pointer. A saturated
	// status bar rarely carries either, so it comes last; the ground itself always does.
	let tray: Rgb = palette
		.candidates(
			&[
				"sideBar.background",
				"panel.background",
				"statusBar.background",
			],
			surface,
		)
		.choose(surface, |tray: Rgb| {
			faded.contrast(tray) >= FADED && ink.contrast(tray) >= TRAY_INK
		});

	BTreeMap::from([
		("surface", surface.css()),
		("ink", ink.css()),
		("ink-faded", faded.css()),
		("ink-bright", bright.css()),
		("mark-write", write.css()),
		("mark-wrong", wrong.css()),
		("mark-point", point.css()),
		("mark-good", good.css()),
		(
			"quiet-mark",
			quiet(&["panel.border", "editorGroup.border", "widget.border"]).css(),
		),
		("tray", tray.css()),
		("scrollbar", quiet(&["scrollbarSlider.background"]).css()),
		("focus-ring", format!("2px solid {}", point.css())),
		(
			"ink-glow",
			if kind.dark() {
				format!("0 0 8px {}", point.css_alpha(0.35))
			} else {
				"none".into()
			},
		),
	])
	.into_iter()
	.map(|(name, value): (&str, String)| (name.to_owned(), value))
	.collect()
}

#[cfg(test)]
pub(super) mod tests {
	use super::*;
	use crate::appearance::{Theme, editors, themes};

	/// A token's colour, read back from what the page is told.
	fn token(theme: &Theme, name: &str) -> Rgb {
		let value: &str = theme.tokens[name].trim_start_matches("2px solid ");
		Rgba::hex(value)
			.unwrap_or_else(|| panic!("{}: {name} is {value}", theme.name))
			.over(Rgb::BLACK)
	}

	/// Every floor, recomputed from the token strings the page receives.
	pub fn assert_readable(theme: &Theme) {
		let surface: Rgb = token(theme, "surface");
		let against = |name: &str| -> f64 { token(theme, name).contrast(surface) };
		let say = |name: &str| -> String {
			format!(
				"{} ({}): {name} at {:.2}",
				theme.name,
				theme.id,
				against(name)
			)
		};
		assert!(against("ink") >= INK, "{}", say("ink"));
		assert!(
			against("ink-faded") >= FADED && against("ink-faded") < against("ink"),
			"{}",
			say("ink-faded")
		);
		assert!(
			against("ink-bright") >= against("ink"),
			"{}",
			say("ink-bright")
		);
		for name in ["mark-write", "mark-wrong", "mark-point", "mark-good"] {
			assert!(against(name) >= MARK, "{}", say(name));
		}
		for name in ["quiet-mark", "scrollbar"] {
			assert!(against(name) >= QUIET, "{}", say(name));
		}
		let tray: Rgb = token(theme, "tray");
		assert!(
			token(theme, "ink-faded").contrast(tray) >= FADED,
			"{}: tray",
			theme.name
		);
		assert!(
			token(theme, "ink").contrast(tray) >= TRAY_INK,
			"{}: tray",
			theme.name
		);
		assert_eq!(token(theme, "focus-ring"), token(theme, "mark-point"));
		assert_eq!(
			theme.dark,
			surface.contrast(Rgb::WHITE) > surface.contrast(Rgb::BLACK)
		);
		assert_eq!(theme.tokens.len(), 13, "{}", theme.name);
	}

	pub fn mapped(name: &str, kind: Kind, colours: &[(&str, &str)]) -> Theme {
		let colours: BTreeMap<String, String> = colours
			.iter()
			.map(|(key, value): &(&str, &str)| ((*key).to_owned(), (*value).to_owned()))
			.collect();
		Theme {
			id: format!("test/{name}"),
			name: name.into(),
			source: "test".into(),
			dark: kind.dark(),
			tokens: roles(&colours, kind),
		}
	}

	/// Each kind with nothing set, each with what a careless theme sets, and grounds the ink
	/// cannot be read on as they stand.
	#[test]
	fn every_kind_and_every_last_resort_clears_its_floors() {
		let themes: Vec<Theme> = vec![
			mapped("light defaults", Kind::Light, &[]),
			mapped("dark defaults", Kind::Dark, &[]),
			mapped("dark high contrast defaults", Kind::HighContrastDark, &[]),
			mapped("light high contrast defaults", Kind::HighContrastLight, &[]),
			// Every colour grey and near the ground: nothing clears, everything is moved.
			mapped(
				"washed out",
				Kind::Dark,
				&[
					("editor.background", "#303030"),
					("editor.foreground", "#555555"),
					("foreground", "#505050"),
					("editorLineNumber.foreground", "#383838"),
					("descriptionForeground", "#383838"),
					("editorCursor.foreground", "#353535"),
					("terminal.ansiYellow", "#3a3a30"),
					("editorWarning.foreground", "#353530"),
					("editorError.foreground", "#3a3030"),
					("errorForeground", "#3a3030"),
					("terminal.ansiRed", "#3a3030"),
					("focusBorder", "#30303a"),
					("textLink.foreground", "#30303a"),
					("terminal.ansiCyan", "#303a3a"),
					("terminal.ansiBlue", "#30303a"),
					("terminal.ansiGreen", "#303a30"),
					("gitDecoration.addedResourceForeground", "#303a30"),
					("panel.border", "#303030"),
					("editorGroup.border", "#303030"),
					("widget.border", "#303030"),
					("sideBar.background", "#d0d0d0"),
					("panel.background", "#d0d0d0"),
					("statusBar.background", "#d0d0d0"),
					("scrollbarSlider.background", "#30303000"),
				],
			),
			// A mid grey carries no ink at 7:1 whichever way the ink goes.
			mapped("mid grey", Kind::Light, &[("editor.background", "#777777")]),
			mapped(
				"mid grey dark",
				Kind::Dark,
				&[("editor.background", "#777777")],
			),
			// Declared dark with a white ground: the ground goes dark, as declared.
			mapped(
				"white but dark",
				Kind::Dark,
				&[("editor.background", "#ffffff")],
			),
			// Translucent ground and ink: each is laid over what is under it.
			mapped(
				"translucent",
				Kind::Light,
				&[
					("editor.background", "#fdf6e380"),
					("editor.foreground", "#657b8340"),
				],
			),
			// Ink brighter than the line numbers' contrast allows: they are not quieter.
			mapped(
				"loud numbers",
				Kind::Dark,
				&[
					("editor.foreground", "#c0c0c0"),
					("editorLineNumber.foreground", "#ffffff"),
					("descriptionForeground", "#ffffff"),
				],
			),
		];
		for theme in &themes {
			assert_readable(theme);
		}
		let white_but_dark: &Theme = &themes[7];
		assert!(token(white_but_dark, "surface").contrast(Rgb::WHITE) >= INK);
	}

	/// A theme's own choice wins where it clears: the mapper only moves what it must.
	#[test]
	fn a_legible_theme_keeps_its_own_colours() {
		let theme: Theme = mapped(
			"kept",
			Kind::Dark,
			&[
				("editor.background", "#1e1e1e"),
				("editor.foreground", "#d4d4d4"),
				("editorCursor.foreground", "#aeafad"),
				("terminal.ansiYellow", "#e5c07b"),
				("focusBorder", "#5ec4d6"),
				("sideBar.background", "#252526"),
			],
		);
		assert_eq!(theme.tokens["surface"], "#1e1e1e");
		assert_eq!(theme.tokens["ink"], "#d4d4d4");
		// The grey cursor is no mark; the theme's yellow is.
		assert_eq!(theme.tokens["mark-write"], "#e5c07b");
		assert_eq!(theme.tokens["mark-point"], "#5ec4d6");
		assert_eq!(theme.tokens["focus-ring"], "2px solid #5ec4d6");
		assert_eq!(theme.tokens["ink-glow"], "0 0 8px rgba(94, 196, 214, 0.35)");
		assert_eq!(theme.tokens["tray"], "#252526");
	}

	/// A default that names another key follows the theme's colour for it: the tray is the
	/// theme's own ground before VS Code's grey side bar, and a high-contrast theme's borders
	/// are its contrast border.
	#[test]
	fn a_default_that_names_another_key_takes_the_themes_colour_for_it() {
		let theme: Theme = mapped(
			"referring",
			Kind::Dark,
			&[
				("editor.background", "#002b36"),
				("foreground", "#93a1a1"),
				("editor.foreground", "#fdf6e3"),
			],
		);
		assert_eq!(theme.tokens["tray"], "#002b36");
		// The description default is the theme's foreground at 70%.
		assert_eq!(
			theme.tokens["ink-faded"],
			Rgba::hex("#93a1a1b3")
				.expect("a colour")
				.over(token(&theme, "surface"))
				.css()
		);
		let contrast: Theme = mapped(
			"contrast",
			Kind::HighContrastDark,
			&[("contrastBorder", "#ff00ff")],
		);
		assert_eq!(contrast.tokens["quiet-mark"], "#ff00ff");
	}

	/// Every theme the editors on this machine have installed, light and dark alike. On a
	/// machine with no editor this checks nothing, and the fixtures above still run.
	#[test]
	fn every_theme_installed_here_clears_its_floors() {
		let found: Vec<Theme> = themes::discover(&editors::shelves());
		let dark: usize = found.iter().filter(|theme: &&Theme| theme.dark).count();
		println!(
			"{} themes installed here, {dark} dark and {} light",
			found.len(),
			found.len() - dark
		);
		for theme in &found {
			println!("  {} — {} ({})", theme.name, theme.source, theme.id);
			assert_readable(theme);
		}
	}
}
