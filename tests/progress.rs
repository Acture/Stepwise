use std::{collections::BTreeMap, fs, path::Path};
use stepwise::{
	app,
	core::{EvaluationMode, Language, RecordedAttempt, Session, Value},
	exercises::{self, Exercise, ProofQuestion, Question, QuestionSet},
	logic::{self, proof::Proof},
	progress::Progress,
	python::{self, parse_value},
};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

#[test]
fn atomic_round_trip_and_mode_assignment_isolation() {
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: std::path::PathBuf = directory.path().join("nested/progress.json");
	let mut progress: Progress = Progress::load(&path).unwrap();
	let mut short: Session = python::session(
		"False and (1 / 0)",
		&BTreeMap::new(),
		EvaluationMode::ShortCircuit,
	)
	.unwrap();
	assert!(short.submit(0, "False").accepted());
	progress.record("", "short", &short);
	progress.save(&path).unwrap();
	let saved: Progress = Progress::load(&path).unwrap();
	assert_eq!(saved.attempts(&short), short.attempts());
	let eager: Session =
		python::session(short.source(), &BTreeMap::new(), EvaluationMode::Eager).unwrap();
	assert!(saved.attempts(&eager).is_empty());
	let mut first: Session = logic::session(
		"P",
		&BTreeMap::from([("P".into(), true)]),
		EvaluationMode::Eager,
	)
	.unwrap();
	assert!(first.submit(0, "True").accepted());
	progress.record("", "logic", &first);
	let different_binding: Session = logic::session(
		"P",
		&BTreeMap::from([("P".into(), false)]),
		EvaluationMode::Eager,
	)
	.unwrap();
	assert!(progress.attempts(&different_binding).is_empty());
	progress.save(&path).unwrap();
	assert_eq!(Progress::load(&path).unwrap().sessions.len(), 2);
}

/// The key is the saved-progress identity: every character of it must stay put, including
/// the Debug shape of each language's own binding map.
#[test]
fn progress_keys_keep_their_exact_text_per_language() {
	let python: Session = python::session(
		"x + 1",
		&BTreeMap::from([("x".into(), parse_value("3").unwrap())]),
		EvaluationMode::ShortCircuit,
	)
	.unwrap();
	assert_eq!(
		python.progress_key(),
		"flexible-substitution-v3\nshort-circuit\npython\n{\"x\": Int(3)}\nx + 1"
	);
	let logic: Session = logic::session(
		"P",
		&BTreeMap::from([("P".into(), true)]),
		EvaluationMode::Eager,
	)
	.unwrap();
	assert_eq!(
		logic.progress_key(),
		"flexible-substitution-v3\neager\nlogic\n{\"P\": true}\nP"
	);
}

#[test]
fn malformed_progress_is_never_silently_reset() {
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: std::path::PathBuf = directory.path().join("progress.json");
	fs::write(&path, "broken progress").unwrap();
	assert!(Progress::load(&path).is_err());
	assert_eq!(fs::read_to_string(&path).unwrap(), "broken progress");
}

/// Question sets added a name beside the pointer; they did not change what a record is. A
/// file written before them has no set name, which reads as "the question in hand belongs to
/// no set" — the same thing a generated question says. So the pointer names nothing this
/// build can reopen and practice starts on a new question, while the work itself is read,
/// kept and still replayed by the question it belongs to.
#[test]
fn a_pointer_written_before_question_sets_draws_a_new_question_and_keeps_the_work() {
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: std::path::PathBuf = directory.path().join("progress.json");
	let embedded: Vec<Question> = exercises::builtin().unwrap().of_language(Language::Python);
	let precedence: &Exercise = embedded
		.iter()
		.filter_map(Question::evaluation)
		.find(|exercise| exercise.name == "precedence")
		.expect("an embedded question");
	let mut session: Session = precedence.session(EvaluationMode::ShortCircuit).unwrap();
	let step: stepwise::core::NextStep = session.next_step().unwrap();
	assert!(
		session
			.submit(step.node_id, &step.outcome.unwrap().to_string())
			.accepted()
	);

	// Write the file this build writes, then take the set name back out of it — that is
	// exactly the shape a build from before question sets left behind.
	let mut written: Progress = Progress::default();
	written.record("builtin", "precedence", &session);
	written.save(&path).unwrap();
	let older: String = fs::read_to_string(&path)
		.unwrap()
		.lines()
		.filter(|line| !line.contains("\"current_set\""))
		.collect::<Vec<&str>>()
		.join("\n");
	assert!(!older.contains("current_set"));
	fs::write(&path, &older).unwrap();

	// It loads, and the record — the student's actual work — is still found by its question.
	let loaded: Progress = Progress::load(&path).unwrap();
	assert_eq!(loaded.current, "precedence");
	assert_eq!(loaded.current_set, "");
	assert_eq!(loaded.attempts(&session), session.attempts());

	// The pointer names no set, so it names no question this build can reopen.
	assert!(!loaded.points_at("builtin", "precedence"));
	let Question::Evaluation(drawn) =
		app::resume_or_generate(Language::Python, &loaded, &embedded).unwrap()
	else {
		panic!("a drawn question is an evaluation one");
	};
	assert!(
		drawn.name.starts_with("random-v1-python-"),
		"{}",
		drawn.name
	);

	// Moving on rewrites the pointer and leaves every earlier record where it was.
	let mut moved_on: Progress = loaded.clone();
	let drawn_session: Session = drawn.session(EvaluationMode::ShortCircuit).unwrap();
	moved_on.record("", &drawn.name, &drawn_session);
	moved_on.save(&path).unwrap();
	let after: Progress = Progress::load(&path).unwrap();
	assert_eq!(after.attempts(&session), session.attempts());
	assert!(after.points_at("", &drawn.name));

	// A version this build has never seen is a different matter: it is refused with the
	// reason, not with whichever field the reader tripped over, and the file is left alone.
	let later: &str = r#"{"version":3,"whatever":true}"#;
	fs::write(&path, later).unwrap();
	assert!(
		Progress::load(&path)
			.unwrap_err()
			.to_string()
			.contains("不支持的进度版本")
	);
	assert_eq!(fs::read_to_string(&path).unwrap(), later);
}

/// Before proofs were stops on a course, saving one left the pointer alone, so
/// `progress-17c6f55.json` — written by the base commit — holds three proofs' lines under an
/// empty pointer. A bare launch reads that as "no question in hand" and draws a random one:
/// old proof work never hijacks a launch that did not ask for it. Now that a proof records the
/// pointer too, a pointer naming the unfinished reductio reopens it where it was left, a
/// finished one gives way to a new question, and neither touches the evaluation strategy.
#[test]
fn a_bare_launch_reopens_a_proof_only_when_the_pointer_names_it_unfinished() {
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: std::path::PathBuf = directory.path().join("progress.json");
	let old: Progress =
		Progress::load(Path::new(&format!("{FIXTURES}/progress-17c6f55.json"))).unwrap();
	assert_eq!(old.current_set, "");
	assert_eq!(old.current, "");
	let logic: Vec<Question> = exercises::builtin().unwrap().of_language(Language::Logic);
	let raa: ProofQuestion = match logic.iter().find(|question| question.name() == "raa") {
		Some(Question::Proof(question)) => question.clone(),
		other => panic!("raa must be a logic proof question of the embedded set, got {other:?}"),
	};
	let initial: Proof = raa.proof().unwrap();
	assert_eq!(old.commands(&initial).len(), 2);

	// The pointer names nothing, so the saved proofs stay saved and a new question is drawn.
	let drawn: Question = app::resume_or_generate(Language::Logic, &old, &logic).unwrap();
	let Question::Evaluation(drawn) = drawn else {
		panic!("an empty pointer draws an evaluation question, got {drawn:?}");
	};
	assert!(drawn.name.starts_with("random-v1-logic-"), "{}", drawn.name);

	// Recording the unfinished reductio points at it and keeps its lines and the strategy.
	let mut pointed: Progress = old.clone();
	pointed.mode = EvaluationMode::Eager;
	let unfinished: Proof = initial.clone().replay(old.commands(&initial)).unwrap();
	assert!(!unfinished.is_finished());
	pointed.record_proof("builtin", "raa", &unfinished);
	assert!(pointed.points_at("builtin", "raa"));
	assert_eq!(pointed.mode, EvaluationMode::Eager);
	assert_eq!(pointed.commands(&initial), old.commands(&initial));
	pointed.save(&path).unwrap();
	let loaded: Progress = Progress::load(&path).unwrap();
	match app::resume_or_generate(Language::Logic, &loaded, &logic).unwrap() {
		Question::Proof(reopened) => {
			assert_eq!(reopened.set, "builtin");
			assert_eq!(reopened.name, "raa");
		}
		other => panic!("the unfinished proof the pointer names reopens, got {other:?}"),
	}
	// Even offered every question of the set, a Python launch leaves the logic proof where
	// it is: the pointer names nothing of that language.
	let everything: QuestionSet = exercises::builtin().unwrap();
	let Question::Evaluation(python) =
		app::resume_or_generate(Language::Python, &loaded, everything.questions()).unwrap()
	else {
		panic!("--python never reopens a logic proof");
	};
	assert!(
		python.name.starts_with("random-v1-python-"),
		"{}",
		python.name
	);

	// Finished, it gives way to a new question and its lines stay on record.
	let mut finished: Proof = unfinished.clone();
	finished.submit("P ; raa ; 2,3").unwrap();
	assert!(finished.is_finished());
	pointed.record_proof("builtin", "raa", &finished);
	assert_eq!(pointed.commands(&initial).len(), 3);
	let Question::Evaluation(moved_on) =
		app::resume_or_generate(Language::Logic, &pointed, &logic).unwrap()
	else {
		panic!("a finished proof is not reopened");
	};
	assert!(
		moved_on.name.starts_with("random-v1-logic-"),
		"{}",
		moved_on.name
	);
}

#[test]
fn final_negative_rule_preserves_older_progress_without_replaying_the_extra_answer() {
	let mut session: Session =
		python::session("-2 ** 2", &BTreeMap::new(), EvaluationMode::ShortCircuit).unwrap();
	let id: usize = session.next_step().unwrap().node_id;
	assert!(session.submit(id, "4").accepted());
	let mut old_attempts: Vec<RecordedAttempt> = session.attempts().to_vec();
	old_attempts.push(RecordedAttempt {
		node_id: session.root().id,
		input: Some("-4".into()),
	});
	let old_key: String =
		session
			.progress_key()
			.replacen("flexible-substitution-v3", "explicit-groups-v1", 1);
	let mut progress: Progress = Progress::default();
	progress
		.sessions
		.insert(old_key.clone(), old_attempts.clone());
	assert!(progress.attempts(&session).is_empty());
	progress.record("", "negative", &session);
	assert_eq!(progress.sessions[&old_key], old_attempts);
	let restored: Session = python::session(session.source(), &BTreeMap::new(), session.mode())
		.unwrap()
		.replay(progress.attempts(&session))
		.unwrap();
	assert!(restored.is_finished());
	assert_eq!(restored.render(), "-4");
	assert_eq!(restored.attempts().len(), 1);
}

#[test]
fn group_clicks_round_trip_without_fabricated_input_and_cannot_solve_operations() {
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: std::path::PathBuf = directory.path().join("progress.json");
	let mut session: Session =
		python::session("((3))", &BTreeMap::new(), EvaluationMode::ShortCircuit).unwrap();
	assert!(!session.remove_group(session.root().id).accepted());
	let inner: usize = session.next_step().unwrap().node_id;
	assert!(session.remove_group(inner).accepted());
	assert_eq!(session.render(), "(3)");
	assert_eq!(session.attempts()[0].input, None);
	let mut progress: Progress = Progress::default();
	progress.record("", "groups", &session);
	progress.save(&path).unwrap();
	let saved: Progress = Progress::load(&path).unwrap();
	let restored: Session = python::session(session.source(), &BTreeMap::new(), session.mode())
		.unwrap()
		.replay(saved.attempts(&session))
		.unwrap();
	assert_eq!(restored.render(), "(3)");
	assert_eq!(restored.root(), session.root());
	let mut arithmetic: Session =
		python::session("2 + 3", &BTreeMap::new(), EvaluationMode::ShortCircuit).unwrap();
	assert!(!arithmetic.remove_group(arithmetic.root().id).accepted());
	assert!(arithmetic.history().is_empty());
	assert!(!arithmetic.is_finished());
}

#[test]
fn python_assignments_types_and_signed_zero_have_separate_progress() {
	let mut progress: Progress = Progress::default();
	for literal in ["False", "0", "0.0", "-0.0", "None", "2", "2.0"] {
		let bindings: BTreeMap<String, Value> =
			BTreeMap::from([("x".into(), parse_value(literal).unwrap())]);
		let mut session: Session =
			python::session("(x)", &bindings, EvaluationMode::ShortCircuit).unwrap();
		assert!(progress.attempts(&session).is_empty());
		let id: usize = session.next_step().unwrap().node_id;
		assert!(session.submit(id, literal).accepted());
		progress.record("", "typed", &session);
		let restored: Session = python::session("(x)", &bindings, EvaluationMode::ShortCircuit)
			.unwrap()
			.replay(progress.attempts(&session))
			.unwrap();
		assert_eq!(restored.render(), session.render());
		assert!(!restored.is_finished(), "group removal is still required");
	}
	assert_eq!(progress.sessions.len(), 7);
}
