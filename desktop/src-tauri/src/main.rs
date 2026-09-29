//! The desktop window's shell: it owns the window, the files and when the progress snapshot is
//! written, and hands everything else to [`Desk`]. The page draws the [`View`] each command
//! returns and forwards the student's input as a [`Command`]; it picks a question file itself
//! with the dialog plugin, so no command here waits on the student.
//!
//! Reading a question file belongs here and never to the library: the desk takes the text and
//! the name to say it by.
//!
//! The commands run on Tauri's async runtime, not the main thread: a progress write syncs the
//! file to disk, and the window must keep drawing and taking input meanwhile. They stay in the
//! student's order because the page sends one only after the last has answered (its queue in
//! `desktop/ui/src/lib/Window.svelte`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
	error::Error,
	ffi::OsString,
	fs, io,
	panic::{self, PanicHookInfo},
	path::PathBuf,
	process::{self, ExitCode},
	sync::{Mutex, MutexGuard},
};

use clap::Parser;
use stepwise::{
	desktop::{Command, Desk, View},
	exercises,
	progress::Progress,
};
use tauri::{Manager, State, Window, WindowEvent};

#[derive(Parser, Debug)]
#[command(version, about = "Stepwise 桌面窗口：选择下一步，理解求值与推理。")]
struct Args {
	/// 进度文件；默认与终端版 stepwise 共用同一个
	#[arg(long, value_name = "PATH", conflicts_with = "no_save")]
	progress_file: Option<PathBuf>,
	/// 不读取也不写入进度
	#[arg(long)]
	no_save: bool,
}

/// The window's state behind the page's commands.
struct Shell {
	desk: Desk,
	/// Where the progress is written: none with `--no-save`, or when the file could not be
	/// read, so a file this build does not understand is never overwritten.
	path: Option<PathBuf>,
}

impl Shell {
	fn open(args: Args) -> Result<Self, Box<dyn Error>> {
		let (progress, path, message): (Progress, Option<PathBuf>, Option<String>) = opening(args);
		Ok(Self {
			desk: Desk::new(progress, exercises::builtin()?, message),
			path,
		})
	}

	/// Writes the progress snapshot, when this launch saves one.
	fn save(&mut self) -> io::Result<()> {
		match &self.path {
			Some(path) => self.desk.progress().save(path),
			None => Ok(()),
		}
	}

	/// Writes the progress after a change; the window says so when the write fails.
	fn saved(&mut self) {
		if let Err(error) = self.save() {
			self.desk
				.alert(format!("进度未能保存：{}。", reason(&error)));
		}
	}

	fn act(&mut self, command: Command) -> View {
		if self.desk.handle(command) {
			self.saved();
		}
		self.desk.view()
	}

	/// Opens the question set at `path`, a file the page's dialog picked.
	fn load_set(&mut self, path: &str) -> View {
		match fs::read_to_string(path) {
			Ok(text) => {
				if self.desk.load(&text, path) {
					self.saved();
				}
			}
			Err(error) => self.desk.alert(format!("无法读取题集 {path}：{error}")),
		}
		self.desk.view()
	}
}

/// The progress the terminal would load for the same flags, where to write it, and what the
/// window says first. A progress file that cannot be found or read still opens the window,
/// empty and saving nothing, and says why.
fn opening(args: Args) -> (Progress, Option<PathBuf>, Option<String>) {
	if args.no_save {
		return (Progress::default(), None, None);
	}
	let path: PathBuf = match args.progress_file.map_or_else(Progress::default_path, Ok) {
		Ok(path) => path,
		Err(error) => {
			let message: String = format!("{}。本次不保存。", reason(&error));
			return (Progress::default(), None, Some(message));
		}
	};
	match Progress::load(&path) {
		Ok(progress) => (progress, Some(path), None),
		Err(error) => (
			Progress::default(),
			None,
			Some(format!(
				"进度文件 {} 无法读取：{}。本次不保存，原文件未改动。",
				path.display(),
				reason(&error)
			)),
		),
	}
}

/// An error as a clause of a sentence that ends it: some already end in a full stop.
fn reason(error: &io::Error) -> String {
	error.to_string().trim_end_matches('。').into()
}

/// A command that panicked while holding the shell has already taken the window down: see
/// [`abort_on_panic`].
fn hold<'a>(shell: &'a State<'_, Mutex<Shell>>) -> MutexGuard<'a, Shell> {
	shell
		.lock()
		.expect("no command panics while holding the shell")
}

#[tauri::command(async)]
fn view(shell: State<'_, Mutex<Shell>>) -> View {
	hold(&shell).desk.view()
}

#[tauri::command(async)]
fn act(command: Command, shell: State<'_, Mutex<Shell>>) -> View {
	hold(&shell).act(command)
}

#[tauri::command(async)]
fn load_set(path: String, shell: State<'_, Mutex<Shell>>) -> View {
	hold(&shell).load_set(&path)
}

/// A panic on the async runtime would end only its own task: nothing answers the page, whose
/// queue then waits for ever, and every later command finds the shell poisoned. Any panic ends
/// the process instead, as one on the main thread does.
fn abort_on_panic() {
	let report: Box<dyn Fn(&PanicHookInfo<'_>) + Send + Sync> = panic::take_hook();
	panic::set_hook(Box::new(move |info: &PanicHookInfo<'_>| {
		report(info);
		process::abort();
	}));
}

/// Every change was written when it happened; closing writes once more, as the terminal
/// does on quitting. No page is left to show a failure, so it goes to stderr.
fn closing(window: &Window, event: &WindowEvent) {
	if window.label() != "main" || !matches!(event, WindowEvent::CloseRequested { .. }) {
		return;
	}
	let shell: State<'_, Mutex<Shell>> = window.state::<Mutex<Shell>>();
	if let Err(error) = hold(&shell).save() {
		eprintln!("Stepwise: 进度未能保存：{error}");
	}
}

fn run(shell: Shell) -> tauri::Result<()> {
	tauri::Builder::default()
		.plugin(tauri_plugin_dialog::init())
		.manage(Mutex::new(shell))
		.invoke_handler(tauri::generate_handler![view, act, load_set])
		.on_window_event(closing)
		.run(tauri::generate_context!())
}

fn main() -> ExitCode {
	abort_on_panic();
	// An app opened from the Finder may be handed a `-psn_…` process serial number, which is
	// no flag of ours; refusing it would close the window before it opened.
	let args: Args = Args::parse_from(
		std::env::args_os()
			.filter(|argument: &OsString| !argument.to_string_lossy().starts_with("-psn_")),
	);
	match Shell::open(args).and_then(|shell: Shell| Ok(run(shell)?)) {
		Ok(()) => ExitCode::SUCCESS,
		Err(error) => {
			eprintln!("Stepwise: {error}");
			ExitCode::FAILURE
		}
	}
}

#[cfg(test)]
mod tests {
	use std::path::Path;

	use clap::CommandFactory;
	use serde_json::{Map, Value};
	use stepwise::desktop::{Choice, EntryView};
	use tempfile::TempDir;

	use super::*;

	fn launched(arguments: &[&str]) -> Shell {
		let args: Args = Args::parse_from([&["stepwise-desktop"], arguments].concat());
		Shell::open(args).expect("the embedded set loads")
	}

	fn message(view: &View) -> Option<&str> {
		match view {
			View::Entry(EntryView { message, .. }) => message.as_deref(),
			View::Evaluation(view) => view.message.as_deref(),
			View::Proof(view) => view.message.as_deref(),
		}
	}

	const PYTHON: Command = Command::Choose {
		language: Choice::Python,
	};

	/// The one window a config file declares.
	fn window(config: &str) -> Map<String, Value> {
		let config: Value = serde_json::from_str(config).expect("a JSON config");
		let windows: &Vec<Value> = config["app"]["windows"].as_array().expect("a window list");
		assert_eq!(windows.len(), 1, "one window");
		windows[0].as_object().expect("a window").clone()
	}

	/// A merge patch replaces an array whole, so the macOS file repeats the window; it may only
	/// add the title bar the page draws under.
	#[test]
	fn the_macos_window_is_the_window_under_the_page_title_bar() {
		let mut macos: Map<String, Value> = window(include_str!("../tauri.macos.conf.json"));
		assert_eq!(macos.remove("titleBarStyle"), Some("Overlay".into()));
		assert_eq!(macos.remove("hiddenTitle"), Some(true.into()));
		assert_eq!(macos, window(include_str!("../tauri.conf.json")));
	}

	#[test]
	fn flags_are_consistent_and_no_save_refuses_a_progress_file() {
		Args::command().debug_assert();
		assert!(
			Args::try_parse_from(["stepwise-desktop", "--no-save", "--progress-file", "p.json"])
				.is_err()
		);
	}

	#[test]
	fn a_change_is_written_where_the_terminal_would_read_it() {
		let directory: TempDir = TempDir::new().expect("temporary directory");
		let path: &Path = &directory.path().join("progress.json");
		let mut shell: Shell = launched(&["--progress-file", path.to_str().expect("utf-8")]);
		assert!(!path.exists());
		assert!(matches!(shell.act(PYTHON), View::Evaluation(_)));
		let written: Progress = Progress::load(path).expect("the written progress loads");
		assert!(!written.current.is_empty());
	}

	/// A file that is not progress, and one from a build with another progress version.
	#[test]
	fn an_unreadable_progress_file_opens_the_window_and_is_left_as_it_was() {
		for text in ["{ not progress", r#"{ "version": 99 }"#] {
			let directory: TempDir = TempDir::new().expect("temporary directory");
			let path: &Path = &directory.path().join("progress.json");
			fs::write(path, text).expect("scratch file");
			let mut shell: Shell = launched(&["--progress-file", path.to_str().expect("utf-8")]);
			assert!(shell.path.is_none());
			let said: String = message(&shell.desk.view()).expect("says why").into();
			assert!(
				said.contains("无法读取") && said.contains("本次不保存"),
				"{said}"
			);
			assert!(matches!(shell.act(PYTHON), View::Evaluation(_)));
			shell.save().expect("nothing to write");
			assert_eq!(fs::read_to_string(path).expect("still there"), text);
		}
	}

	#[test]
	fn a_question_file_that_cannot_be_read_is_said_in_the_window() {
		let directory: TempDir = TempDir::new().expect("temporary directory");
		let missing: String = directory.path().join("missing.toml").display().to_string();
		let mut shell: Shell = launched(&["--no-save"]);
		let view: View = shell.load_set(&missing);
		let said: &str = message(&view).expect("says why");
		assert!(
			said.starts_with(&format!("无法读取题集 {missing}：")),
			"{said}"
		);
	}
}
