use std::{
	collections::BTreeSet,
	process::{Command, Output},
};

use num_traits::ToPrimitive;
use stepwise::{
	core::{ExprKind, Language, NextStep, Session, Value},
	exercises::Exercise,
	generate,
	progress::Progress,
};

/// The step a hint names: the language's own short-circuit order.
fn hinted(session: &Session) -> NextStep {
	session.next_step().unwrap()
}

/// An allowed step that applies no operation before every operand is a value, so nothing a
/// short circuit could skip is left unread.
fn computing_everything(session: &Session) -> NextStep {
	session
		.allowed_steps()
		.into_iter()
		.find(|step| {
			!matches!(
				&session.root().find(step.node_id).unwrap().kind,
				ExprKind::Operation(_, operands) if operands.iter().any(|operand| operand.value().is_none())
			)
		})
		.expect("computing every operand is always allowed")
}

fn finish(mut session: Session, choose: fn(&Session) -> NextStep) -> Value {
	while !session.is_finished() {
		let step: NextStep = choose(&session);
		let value: Value = step.outcome.unwrap();
		assert!(value.to_string().len() <= 10);
		match &value {
			Value::Int(number) => assert!((-999..=999).contains(&number.to_i64().unwrap())),
			Value::Float(number) => assert!(number.abs() <= 999.0),
			Value::Bool(_) => {}
			Value::None => panic!("generator does not emit None"),
		}
		assert!(session.submit(step.node_id, &value.to_string()).accepted());
		assert!(session.history().len() <= 40);
	}
	assert!((2..=40).contains(&session.history().len()));
	assert!(session.terminal_error().is_none());
	session.root().value().unwrap().clone()
}

#[test]
fn generated_questions_are_varied_bounded_and_solvable_by_either_route() {
	for language in [Language::Python, Language::Logic] {
		let mut sources: BTreeSet<String> = BTreeSet::new();
		for seed in (0..128).chain([u64::MAX]) {
			let exercise: Exercise = generate::generate(language, seed).unwrap();
			assert!(!exercise.bindings.is_empty());
			assert!(exercise.expression.len() <= 180);
			// Both routes run in the same kind of session; nothing switches between them.
			let short: Value = finish(exercise.session().unwrap(), hinted);
			let complete: Value = finish(exercise.session().unwrap(), computing_everything);
			assert!(
				short.same_answer(&complete) && short.type_name() == complete.type_name(),
				"{}",
				exercise.expression
			);
			sources.insert(exercise.expression);
		}
		assert!(
			sources.len() > 120,
			"generator must vary structure, not only assignments"
		);
	}
}

#[test]
fn versioned_seed_restores_the_exact_question_and_partial_progress() {
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: std::path::PathBuf = directory.path().join("progress.json");
	for language in [Language::Python, Language::Logic] {
		let exercise: Exercise = generate::generate(language, 42).unwrap();
		let repeat: Exercise = generate::generate(language, 42).unwrap();
		assert_eq!(exercise.expression, repeat.expression);
		assert_eq!(exercise.bindings, repeat.bindings);
		// These versioned fixtures protect persisted seeds from accidental generator drift.
		let (source, assignments): (&str, &str) = match language {
			Language::Python => (
				"(z - 6 - y) % 2 >= 4 - x or not (-(z - x)) // 4 == 9 / 2",
				"x=7 y=-4 z=3",
			),
			Language::Logic => (
				"(S ∨ ((R → Q) ∨ P)) ∧ (Q ∧ S) ∨ Q ∧ (S → S)",
				"P=True Q=False R=True S=False",
			),
		};
		assert_eq!(exercise.expression, source);
		assert_eq!(exercise.assignments(), assignments);
		let mut session: Session = exercise.session().unwrap();
		for _ in 0..3 {
			let step: NextStep = computing_everything(&session);
			assert!(
				session
					.submit(step.node_id, &step.outcome.unwrap().to_string())
					.accepted()
			);
		}
		let mut progress: Progress = Progress::default();
		progress.record(&exercise.set, &exercise.name, &session);
		progress.save(&path).unwrap();
		let progress: Progress = Progress::load(&path).unwrap();
		let restored: Exercise = generate::restore(&progress.current).unwrap().unwrap();
		assert_eq!(restored.expression, exercise.expression);
		assert_eq!(restored.bindings, exercise.bindings);
		let initial: Session = restored.session().unwrap();
		let restored: Session = initial.clone().replay(progress.attempts(&initial)).unwrap();
		assert_eq!(restored.render(), session.render());
		assert_eq!(restored.attempts(), session.attempts());
	}
	assert!(generate::restore("precedence").unwrap().is_none());
	for bad in [
		"random-v0-python-42",
		"random-v1-unknown-1",
		"random-v1-python-bad",
	] {
		assert!(generate::restore(bad).is_err());
	}
}

fn cli(args: &[&str]) -> Output {
	Command::new(env!("CARGO_BIN_EXE_stepwise"))
		.args(args)
		.output()
		.unwrap()
}

#[test]
fn cli_requires_exactly_one_language_before_starting() {
	for args in [
		vec![],
		vec!["--trace"],
		vec!["--list"],
		vec!["2 + 3"],
		vec!["--proof", "raa"],
		vec!["--python", "--logic", "--trace"],
		vec!["--python", "--proof", "raa"],
	] {
		let output: Output = cli(&args);
		assert_eq!(output.status.code(), Some(2), "{args:?}");
		let error: String = String::from_utf8(output.stderr).unwrap();
		assert!(
			error.contains("--python") && error.contains("--logic"),
			"{error}"
		);
		assert!(output.stdout.is_empty());
	}
	for args in [
		vec!["--help"],
		vec!["--version"],
		vec!["--python", "--list"],
		vec!["--logic", "--list"],
		vec![
			"--logic",
			"--proof",
			"raa",
			"--check-proof",
			concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/raa.json"),
		],
	] {
		let output: Output = cli(&args);
		assert!(
			output.status.success(),
			"{args:?}: {}",
			String::from_utf8_lossy(&output.stderr)
		);
	}
	for (language, formula, label) in [
		("--python", "2 + 3", "Python 运算练习"),
		("--logic", "True ∧ False", "命题逻辑"),
	] {
		let output: Output = cli(&[language, "--trace", formula]);
		assert!(output.status.success());
		assert_eq!(
			String::from_utf8(output.stdout).unwrap().lines().next(),
			Some(label)
		);
	}
}

#[test]
fn seeded_random_cli_is_repeatable_and_rejects_conflicting_modes() {
	for args in [vec!["--python", "--trace"], vec!["--logic", "--trace"]] {
		let output: Output = cli(&args);
		assert!(
			output.status.success(),
			"{}",
			String::from_utf8_lossy(&output.stderr)
		);
		let text: String = String::from_utf8(output.stdout).unwrap();
		assert!(
			text.lines().next().unwrap().contains('='),
			"default random question must include bindings"
		);
		let source: &str = text.lines().nth(2).unwrap();
		assert!(
			!stepwise::exercises::builtin()
				.unwrap()
				.exercises()
				.any(|exercise| exercise.expression == source)
		);
	}
	let sample: Output = cli(&["--python", "--exercise", "precedence", "--trace"]);
	assert!(sample.status.success());
	assert!(String::from_utf8_lossy(&sample.stdout).contains("2 + (3 * 4)"));
	for args in [
		vec!["--python", "--random", "--seed", "42", "--trace"],
		vec!["--logic", "--random", "--seed", "42", "--trace"],
	] {
		let first: Output = cli(&args);
		assert!(
			first.status.success(),
			"{}",
			String::from_utf8_lossy(&first.stderr)
		);
		assert_eq!(first.stdout, cli(&args).stdout);
		assert!(String::from_utf8_lossy(&first.stdout).contains("完成："));
	}
	for args in [
		vec!["--python", "--seed", "1", "--trace"],
		vec!["--python", "--random", "2 + 3", "--trace"],
		vec!["--python", "--random", "--assign", "x=2", "--trace"],
		vec!["--logic", "--random", "--proof", "mp"],
	] {
		assert!(!cli(&args).status.success());
	}
}

/// Seeds still draw the questions they drew before optional short circuit. `random-v1-seeds.tsv` was written by
/// a binary built at c3e2aea — the base commit, where suitability checked the two evaluation
/// modes — from its own `--trace` over seeds 0–127 and `u64::MAX` per language. Suitability now
/// checks the two reference routes instead; if that ever accepted or refused a draw the modes
/// did not, a persisted `random-v1-` ID would reopen as another question, and this fails.
#[test]
fn seeds_draw_the_questions_the_base_commit_drew() {
	let fixture: &str = include_str!("fixtures/random-v1-seeds.tsv");
	let mut checked: usize = 0;
	for line in fixture.lines() {
		let [language, seed, assignments, expression] = line.split('\t').collect::<Vec<&str>>()[..]
		else {
			panic!("malformed line: {line}");
		};
		let language: Language = Language::from_key(language).unwrap();
		let exercise: Exercise = generate::generate(language, seed.parse().unwrap()).unwrap();
		assert_eq!(exercise.assignments(), assignments, "{line}");
		assert_eq!(exercise.expression, expression, "{line}");
		assert_eq!(
			exercise.name,
			format!("random-v1-{}-{seed}", language.key()),
			"{line}"
		);
		checked += 1;
	}
	assert_eq!(checked, 258);
}
