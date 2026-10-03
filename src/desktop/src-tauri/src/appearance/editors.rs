//! VS Code and the forks that share its extension format and its settings file, and where
//! each keeps them on macOS, Windows and Linux. Only folders that exist are ever read.

use std::path::{Path, PathBuf};

use directories::BaseDirs;

/// A folder of extensions, and the editor to name it by in the settings list.
pub struct Shelf {
	pub editor: &'static str,
	pub dir: PathBuf,
}

struct Editor {
	name: &'static str,
	/// Its folder in the system's per-user configuration folder, holding `User/settings.json`,
	/// named alike on every system.
	config: &'static str,
	/// Its folder in the home folder, holding the extensions the user installed.
	home: &'static str,
}

/// In the order themes are listed: a theme several editors ship is listed under the first.
const EDITORS: [Editor; 5] = [
	Editor {
		name: "VS Code",
		config: "Code",
		home: ".vscode",
	},
	Editor {
		name: "VS Code Insiders",
		config: "Code - Insiders",
		home: ".vscode-insiders",
	},
	Editor {
		name: "VSCodium",
		config: "VSCodium",
		home: ".vscode-oss",
	},
	Editor {
		name: "Cursor",
		config: "Cursor",
		home: ".cursor",
	},
	Editor {
		name: "Windsurf",
		config: "Windsurf",
		home: ".windsurf",
	},
];

/// Where each editor's app is installed, each holding `resources/app/extensions` below it
/// (macOS: `Contents/Resources/app/extensions`), keyed by the editor's name.
#[cfg(target_os = "macos")]
fn installs(home: &Path) -> Vec<(&'static str, PathBuf)> {
	let bundles: [(&str, &str); 5] = [
		("VS Code", "Visual Studio Code.app"),
		("VS Code Insiders", "Visual Studio Code - Insiders.app"),
		("VSCodium", "VSCodium.app"),
		("Cursor", "Cursor.app"),
		("Windsurf", "Windsurf.app"),
	];
	let mut installs: Vec<(&str, PathBuf)> = Vec::new();
	for (editor, bundle) in bundles {
		for applications in [Path::new("/Applications"), &home.join("Applications")] {
			installs.push((
				editor,
				applications
					.join(bundle)
					.join("Contents/Resources/app/extensions"),
			));
		}
	}
	installs
}

#[cfg(target_os = "windows")]
fn installs(_home: &Path) -> Vec<(&'static str, PathBuf)> {
	// A per-user install goes under Programs in the local app data folder; a machine-wide
	// one under Program Files.
	let folders: [(&str, &str); 5] = [
		("VS Code", "Microsoft VS Code"),
		("VS Code Insiders", "Microsoft VS Code Insiders"),
		("VSCodium", "VSCodium"),
		("Cursor", "cursor"),
		("Windsurf", "Windsurf"),
	];
	let roots: Vec<PathBuf> = [
		std::env::var_os("LOCALAPPDATA")
			.map(|local: std::ffi::OsString| PathBuf::from(local).join("Programs")),
		std::env::var_os("ProgramFiles").map(PathBuf::from),
	]
	.into_iter()
	.flatten()
	.collect();
	let mut installs: Vec<(&str, PathBuf)> = Vec::new();
	for (editor, folder) in folders {
		for root in &roots {
			installs.push((editor, root.join(folder).join("resources/app/extensions")));
		}
	}
	installs
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn installs(home: &Path) -> Vec<(&'static str, PathBuf)> {
	// Distribution packages, the tarballs' usual home, Snap and Flatpak.
	let flatpak = |id: &str, inside: &str| -> [PathBuf; 2] {
		let app: String = format!("flatpak/app/{id}/current/active/files/{inside}");
		[
			Path::new("/var/lib").join(&app),
			home.join(".local/share").join(&app),
		]
	};
	let places: [(&str, Vec<PathBuf>); 5] = [
		(
			"VS Code",
			[
				PathBuf::from("/usr/share/code"),
				PathBuf::from("/opt/visual-studio-code"),
				PathBuf::from("/snap/code/current/usr/share/code"),
			]
			.into_iter()
			.chain(flatpak("com.visualstudio.code", "extra/vscode"))
			.collect(),
		),
		(
			"VS Code Insiders",
			vec![
				PathBuf::from("/usr/share/code-insiders"),
				PathBuf::from("/opt/visual-studio-code-insiders"),
				PathBuf::from("/snap/code-insiders/current/usr/share/code-insiders"),
			],
		),
		(
			"VSCodium",
			[
				PathBuf::from("/usr/share/codium"),
				PathBuf::from("/opt/vscodium-bin"),
				PathBuf::from("/snap/codium/current/usr/share/codium"),
			]
			.into_iter()
			.chain(flatpak("com.vscodium.codium", "share/codium"))
			.collect(),
		),
		(
			"Cursor",
			vec![
				PathBuf::from("/usr/share/cursor"),
				PathBuf::from("/opt/cursor"),
			],
		),
		(
			"Windsurf",
			vec![
				PathBuf::from("/usr/share/windsurf"),
				PathBuf::from("/opt/windsurf"),
			],
		),
	];
	places
		.into_iter()
		.flat_map(|(editor, roots): (&'static str, Vec<PathBuf>)| {
			roots
				.into_iter()
				.map(move |root: PathBuf| (editor, root.join("resources/app/extensions")))
		})
		.collect()
}

/// Every extension folder of every editor: each editor's built-in themes, then the ones its
/// user installed. None when the system names no home folder.
pub fn shelves() -> Vec<Shelf> {
	let Some(dirs) = BaseDirs::new() else {
		return Vec::new();
	};
	let home: &Path = dirs.home_dir();
	let installs: Vec<(&str, PathBuf)> = installs(home);
	let mut shelves: Vec<Shelf> = Vec::new();
	for editor in &EDITORS {
		for (_, dir) in installs.iter().filter(|(name, _)| *name == editor.name) {
			shelves.push(Shelf {
				editor: editor.name,
				dir: dir.clone(),
			});
		}
		shelves.push(Shelf {
			editor: editor.name,
			dir: home.join(editor.home).join("extensions"),
		});
	}
	shelves
}

/// Each editor's user settings file, in the editors' order.
pub fn settings_files() -> Vec<PathBuf> {
	BaseDirs::new().map_or_else(Vec::new, |dirs: BaseDirs| {
		EDITORS
			.iter()
			.map(|editor: &Editor| {
				dirs.config_dir()
					.join(editor.config)
					.join("User/settings.json")
			})
			.collect()
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	/// An install place keyed by a name no editor has would never be read.
	#[test]
	fn every_install_place_belongs_to_an_editor() {
		for (editor, dir) in installs(Path::new("/home")) {
			assert!(
				EDITORS.iter().any(|known: &Editor| known.name == editor),
				"{editor}: {}",
				dir.display()
			);
			assert!(dir.ends_with("app/extensions"), "{}", dir.display());
		}
	}
}
