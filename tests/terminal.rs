//! The inline terminal adapter, driven through a real pseudo-terminal.
//!
//! Everything below the app layer — raw mode, the inline viewport, key mapping, the
//! archived transcript and the exit sequence — only exists once a terminal does, so the
//! unit tests in `src/tui` cannot reach it. `tests/terminal_smoke.py` supplies the pty and
//! nothing else; the expectations stay here.

use std::{
	collections::BTreeMap,
	io::Write,
	process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct Run {
	args: Vec<String>,
	keys: Vec<String>,
}

#[derive(Serialize)]
struct Request {
	binary: String,
	columns: u16,
	rows: u16,
	runs: Vec<Run>,
}

#[derive(Deserialize)]
struct Capture {
	raw: String,
	visible: String,
}

#[derive(Deserialize)]
struct Response {
	runs: Vec<Capture>,
}

fn run(args: &[&str], keys: &[&str]) -> Run {
	Run {
		args: args.iter().map(|arg| (*arg).into()).collect(),
		keys: keys.iter().map(|key| (*key).into()).collect(),
	}
}

/// The inline viewport places characters with cursor moves rather than literal spaces, so
/// compare without whitespace instead of pinning cell padding.
fn squashed(text: &str) -> String {
	text.chars()
		.filter(|character| !character.is_whitespace())
		.collect()
}

fn drive(runs: Vec<Run>) -> Vec<Capture> {
	let request: Request = Request {
		binary: env!("CARGO_BIN_EXE_stepwise").into(),
		columns: 80,
		rows: 24,
		runs,
	};
	let mut child: std::process::Child = Command::new("python3")
		.arg(concat!(
			env!("CARGO_MANIFEST_DIR"),
			"/tests/terminal_smoke.py"
		))
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.spawn()
		.expect("python3 must be installed for the explicit terminal test");
	child
		.stdin
		.take()
		.expect("piped stdin")
		.write_all(&serde_json::to_vec(&request).expect("request serializes"))
		.expect("request reaches the driver");
	let output: std::process::Output = child.wait_with_output().expect("driver finishes");
	assert!(
		output.status.success(),
		"pty driver failed: {:?}",
		output.status
	);
	serde_json::from_slice::<Response>(&output.stdout)
		.expect("driver returns captures")
		.runs
}

#[test]
#[ignore = "requires a pty and python3; run cargo test --test terminal -- --ignored --nocapture"]
fn the_inline_adapter_teaches_and_saves_through_a_real_terminal() {
	let directory: tempfile::TempDir = tempfile::tempdir().expect("temporary directory");
	let progress: std::path::PathBuf = directory.path().join("progress.json");
	let file: String = progress.to_string_lossy().into();
	let captures: Vec<Capture> = drive(vec![
		// Hint, navigate, answer wrong, answer right, click the finished bracket, finish.
		run(
			&[
				"--python",
				"--exercise",
				"precedence",
				"--progress-file",
				&file,
			],
			&[
				"h",
				"\u{1b}[B",
				"\u{1b}[B",
				"\r",
				"99",
				"\r",
				"\u{7f}\u{7f}",
				"12",
				"\r",
				"\u{1b}[B",
				"\r",
				"14",
				"\r",
				"q",
			],
		),
		// Natural deduction: a rejected rule, modus ponens, then undo.
		run(
			&["--logic", "--proof", "mp", "--progress-file", &file],
			&[
				"Q ; and-intro ; 1,2",
				"\r",
				"\u{15}",
				"Q ; mp ; 1,2",
				"\r",
				"\u{1a}",
				"\u{3}",
			],
		),
	]);
	let [expression, proof] = <[Capture; 2]>::try_from(captures).ok().expect("two runs");

	// The inline viewport must never take over or wipe the screen, and must hand the
	// terminal back the way it found it.
	assert!(
		!expression.raw.contains("\u{1b}[?1049h"),
		"entered the alternate screen"
	);
	assert!(
		!expression.raw.contains("\u{1b}[2J"),
		"cleared the whole screen"
	);
	assert!(
		expression.raw.contains("\u{1b}[?1000l") || expression.raw.contains("\u{1b}[?1003l"),
		"left mouse capture enabled"
	);

	let shown: String = squashed(&expression.visible);
	for expected in [
		"2 + (3 * 4)",              // the source stays visible above the draft
		"下一步选择",               // h hints without answering
		"____",                     // the blank opens in place
		"选对了位置，但结果不正确", // 99 is rejected
		"2 + (12)",                 // 12 contracts the multiplication
		"已去掉这一层括号",         // the finished bracket needs no answer
		"完成",
	] {
		assert!(
			shown.contains(&squashed(expected)),
			"expression run missing {expected}"
		);
	}
	assert!(shown.contains("14"), "final value missing");

	let proved: String = squashed(&proof.visible);
	for expected in [
		"自然演绎 · 目标：Q",
		"1 (P → Q) [premise ]",
		"2 P [premise ]",
		"不符合", // and-intro does not apply here
		"3 Q [mp 1,2]",
		"目标已在所有假设之外成立，证明完成。",
		"撤销第 3 行及其假设作用域变更。",
	] {
		assert!(
			proved.contains(&squashed(expected)),
			"proof run missing {expected}"
		);
	}

	// Both runs wrote through the same progress file, atomically, in the saved format.
	let saved: BTreeMap<String, serde_json::Value> =
		serde_json::from_str(&std::fs::read_to_string(&progress).expect("progress written"))
			.expect("progress parses");
	assert!(
		saved["sessions"]
			.as_object()
			.expect("sessions map")
			.keys()
			.any(|key| key.starts_with("flexible-substitution-v3")),
		"expression progress missing"
	);
	assert!(
		!saved["proofs"].as_object().expect("proofs map").is_empty(),
		"proof progress missing"
	);
}
