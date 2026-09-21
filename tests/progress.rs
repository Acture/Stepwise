use std::{collections::BTreeMap, fs};
use stepwise::{
	core::{EvaluationMode, RecordedAttempt, Session, Value},
	logic,
	progress::Progress,
	python::{self, parse_value},
};

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
	progress.record("short", &short);
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
	progress.record("logic", &first);
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
	progress.record("negative", &session);
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
	progress.record("groups", &session);
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
		progress.record("typed", &session);
		let restored: Session = python::session("(x)", &bindings, EvaluationMode::ShortCircuit)
			.unwrap()
			.replay(progress.attempts(&session))
			.unwrap();
		assert_eq!(restored.render(), session.render());
		assert!(!restored.is_finished(), "group removal is still required");
	}
	assert_eq!(progress.sessions.len(), 7);
}
