//! The binary's noninteractive commands and failure codes, with no terminal attached.

use std::process::{Command, Output, Stdio};

fn run(args: &[&str], code: i32) -> String {
	let output: Output = Command::new(env!("CARGO_BIN_EXE_stepwise"))
		.args(args)
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.output()
		.expect("binary runs without a terminal");
	let stdout: String = String::from_utf8(output.stdout).expect("UTF-8 stdout");
	let stderr: String = String::from_utf8(output.stderr).expect("UTF-8 stderr");
	assert_eq!(
		output.status.code(),
		Some(code),
		"{args:?}: {stdout}\n{stderr}"
	);
	assert!(
		!stdout.contains('\u{1b}'),
		"noninteractive output contains terminal controls"
	);
	format!("{stdout}{stderr}")
}

#[test]
fn noninteractive_commands_need_no_terminal_or_progress() {
	assert_eq!(
		run(&["--version"], 0).trim(),
		concat!("stepwise ", env!("CARGO_PKG_VERSION"))
	);
	let help: String = run(&["--help"], 0);
	for option in [
		"--python",
		"--logic",
		"--trace",
		"--check-proof",
		"--no-save",
	] {
		assert!(help.contains(option), "help missing {option}");
	}
	assert!(run(&["--python", "--list"], 0).contains("precedence"));
	assert!(run(&["--logic", "--list"], 0).contains("raa"));
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let progress: std::path::PathBuf = directory.path().join("中文 进度.json");
	std::fs::write(&progress, b"not even a progress file").unwrap();
	let path: &str = progress.to_str().unwrap();
	assert!(
		run(
			&[
				"--python",
				"2 + (3 * 4)",
				"--trace",
				"--progress-file",
				path
			],
			0
		)
		.contains("完成：14")
	);
	assert!(
		run(
			&["--logic", "真 & 假", "--trace", "--progress-file", path],
			0
		)
		.contains("完成：False")
	);
	assert!(
		run(
			&[
				"--logic",
				"P -> Q",
				"--equivalent",
				"~P | Q",
				"--progress-file",
				path
			],
			0
		)
		.contains("等价：")
	);
	assert!(
		run(
			&[
				"--python",
				"--random",
				"--seed",
				"42",
				"--trace",
				"--progress-file",
				path
			],
			0
		)
		.contains("完成：")
	);
	let set: std::path::PathBuf =
		std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("questions/example.toml");
	assert!(
		run(&["--python", "--set", set.to_str().unwrap(), "--list"], 0)
			.contains("guarded-division")
	);
	let proof: std::path::PathBuf =
		std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/raa.json");
	assert!(
		run(
			&[
				"--logic",
				"--proof",
				"raa",
				"--check-proof",
				proof.to_str().unwrap(),
				"--progress-file",
				path
			],
			0
		)
		.contains("证明完成")
	);
	let invalid: std::path::PathBuf = directory.path().join("错误 证明.json");
	std::fs::write(&invalid, "[\"Q ; and-intro ; 1,2\"]").unwrap();
	assert!(
		run(
			&[
				"--logic",
				"--proof",
				"mp",
				"--check-proof",
				invalid.to_str().unwrap()
			],
			1
		)
		.contains("Stepwise:")
	);
	std::fs::write(&invalid, "[]").unwrap();
	assert!(
		run(
			&[
				"--logic",
				"--proof",
				"mp",
				"--check-proof",
				invalid.to_str().unwrap()
			],
			1
		)
		.contains("尚未")
	);
	assert_eq!(
		std::fs::read(&progress).unwrap(),
		b"not even a progress file"
	);
}

#[test]
fn invalid_arguments_and_interactive_launches_have_distinct_failure_codes() {
	assert!(run(&[], 2).contains("--python"));
	assert!(run(&["--python", "--logic"], 2).contains("error:"));
	assert!(run(&["--python", "1 +", "--trace"], 1).contains("Stepwise:"));
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let progress: std::path::PathBuf = directory.path().join("中文 进度.json");
	let path: &str = progress.to_str().unwrap();
	assert!(run(&["--python", "1 + 2", "--progress-file", path], 1).contains("交互练习需要终端"));
	assert!(
		run(&["--logic", "--proof", "mp", "--progress-file", path], 1)
			.contains("自然演绎练习需要交互终端")
	);
	assert!(!progress.exists(), "a refused launch wrote progress");
}
