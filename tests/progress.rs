use std::{collections::BTreeMap, fs, path::Path};
use stepwise::{
	app::{self, Course, Lesson, Practice, Task},
	core::{Language, RecordedAttempt, Session, Value},
	exercises::{self, Exercise, ProofQuestion, Question, QuestionSet},
	logic::{self, proof::Proof},
	progress::Progress,
	python::{self, parse_value},
};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// A version-1 file left by a build that still switched strategies; see
/// `a_version_one_file_loads_without_its_strategy_and_keeps_every_record_verbatim`.
const STRATEGIES: &str = "progress-v1-strategies.json";

/// The message every refused version carries, whichever number the file declares.
const UNSUPPORTED: &str =
	"不支持的进度版本；请指定新的 --progress-file 或使用 --no-save。原文件未改动。";

fn fixture(name: &str) -> std::path::PathBuf {
	Path::new(FIXTURES).join(name)
}

/// One question keeps one record whichever way the student goes: short-circuiting and
/// computing the operand the short circuit skips are two paths through the same session, so
/// they share a key and the later record replaces the earlier. Only the question and its
/// valuation tell records apart.
#[test]
fn atomic_round_trip_keeps_one_record_per_question_and_valuation() {
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: std::path::PathBuf = directory.path().join("nested/progress.json");
	let mut progress: Progress = Progress::load(&path).unwrap();
	let mut short: Session = python::session("False and (1 / 0)", &BTreeMap::new()).unwrap();
	let mut continued: Session = short.clone();
	assert!(short.submit(0, "False").accepted());
	// The division the short circuit skips is still a step the student may take.
	let division: usize = continued
		.allowed_steps()
		.into_iter()
		.find(|step| step.outcome.is_err())
		.expect("the skipped division stays allowed")
		.node_id;
	assert!(continued.submit(division, "ZeroDivisionError").accepted());
	assert!(continued.terminal_error().is_some());
	assert_eq!(short.progress_key(), continued.progress_key());

	progress.record("", "short", &short);
	progress.save(&path).unwrap();
	let saved: Progress = Progress::load(&path).unwrap();
	assert_eq!(saved.attempts(&short), short.attempts());
	let replayed: Session = python::session(short.source(), &BTreeMap::new())
		.unwrap()
		.replay(saved.attempts(&short))
		.unwrap();
	assert_eq!(replayed.render(), "False");

	// The other path overwrites the one record rather than adding a second.
	progress.record("", "short", &continued);
	assert_eq!(progress.sessions.len(), 1);
	progress.save(&path).unwrap();
	let saved: Progress = Progress::load(&path).unwrap();
	assert_eq!(saved.attempts(&short), continued.attempts());
	let replayed: Session = python::session(short.source(), &BTreeMap::new())
		.unwrap()
		.replay(saved.attempts(&short))
		.unwrap();
	assert!(replayed.is_finished());
	assert_eq!(replayed.terminal_error(), continued.terminal_error());

	let mut first: Session = logic::session("P", &BTreeMap::from([("P".into(), true)])).unwrap();
	assert!(first.submit(0, "True").accepted());
	progress.record("", "logic", &first);
	let different_binding: Session =
		logic::session("P", &BTreeMap::from([("P".into(), false)])).unwrap();
	assert!(progress.attempts(&different_binding).is_empty());
	progress.save(&path).unwrap();
	assert_eq!(Progress::load(&path).unwrap().sessions.len(), 2);
}

/// The key is the saved-progress identity: every character of it must stay put, including
/// the Debug shape of each language's own binding map. It names the teaching rules and the
/// question, and no evaluation strategy.
#[test]
fn progress_keys_keep_their_exact_text_per_language() {
	let python: Session = python::session(
		"x + 1",
		&BTreeMap::from([("x".into(), parse_value("3").unwrap())]),
	)
	.unwrap();
	assert_eq!(
		python.progress_key(),
		"flexible-substitution-v4\npython\n{\"x\": Int(3)}\nx + 1"
	);
	let logic: Session = logic::session("P", &BTreeMap::from([("P".into(), true)])).unwrap();
	assert_eq!(
		logic.progress_key(),
		"flexible-substitution-v4\nlogic\n{\"P\": true}\nP"
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
/// file written before them is version 1, carries the strategy the pointer was left in, has no
/// set name — which reads as "the question in hand belongs to no set", the same thing a
/// generated question says — and keeps its evaluation work under a key with a strategy in it,
/// written under earlier teaching rules. So the pointer names nothing this build can reopen and
/// practice starts on a new question, while the work is read and kept verbatim without being
/// replayed into today's rules, and survives the next save.
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
	let mut session: Session = precedence.session().unwrap();
	let step: stepwise::core::NextStep = session.next_step().unwrap();
	assert!(
		session
			.submit(step.node_id, &step.outcome.unwrap().to_string())
			.accepted()
	);

	// Exactly what a build from before question sets left behind: version 1, a strategy, no
	// set name, and the work under that build's key.
	let old_key: String = session.progress_key().replacen(
		"flexible-substitution-v4\n",
		"flexible-substitution-v3\nshort-circuit\n",
		1,
	);
	let older: String = serde_json::to_string_pretty(&serde_json::json!({
		"version": 1,
		"current": "precedence",
		"mode": "short-circuit",
		"sessions": { old_key.clone(): session.attempts() },
		"proofs": {},
	}))
	.unwrap();
	fs::write(&path, &older).unwrap();

	// It loads untouched, and the record — the student's actual work — is kept verbatim but
	// does not replay into today's rules.
	let loaded: Progress = Progress::load(&path).unwrap();
	assert_eq!(fs::read_to_string(&path).unwrap(), older);
	assert_eq!(loaded.current, "precedence");
	assert_eq!(loaded.current_set, "");
	assert_eq!(loaded.sessions[&old_key], session.attempts());
	assert!(loaded.attempts(&session).is_empty());

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

	// Moving on rewrites the pointer, saves today's version with no strategy, and leaves every
	// earlier record where it was.
	let mut moved_on: Progress = loaded.clone();
	let drawn_session: Session = drawn.session().unwrap();
	moved_on.record("", &drawn.name, &drawn_session);
	moved_on.save(&path).unwrap();
	let saved: serde_json::Value =
		serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
	assert_eq!(saved["version"], 3);
	assert!(saved.get("mode").is_none());
	let after: Progress = Progress::load(&path).unwrap();
	assert_eq!(after.sessions[&old_key], session.attempts());
	assert!(after.points_at("", &drawn.name));

	// A version this build has never seen is a different matter: it is refused with the
	// reason, not with whichever field the reader tripped over, and the file is left alone.
	let later: &str = r#"{"version":4,"whatever":true}"#;
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
/// empty pointer. It is a version-1 file, strategy and all, and still loads: proof records
/// never carried a strategy, so they replay exactly as before. A bare launch reads the empty
/// pointer as "no question in hand" and draws a random one: old proof work never hijacks a
/// launch that did not ask for it. Now that a proof records the pointer too, a pointer naming
/// the unfinished reductio reopens it where it was left, and a finished one gives way to a new
/// question.
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

	// Recording the unfinished reductio points at it and keeps its lines.
	let mut pointed: Progress = old.clone();
	let unfinished: Proof = initial.clone().replay(old.commands(&initial)).unwrap();
	assert!(!unfinished.is_finished());
	pointed.record_proof("builtin", "raa", &unfinished);
	assert!(pointed.points_at("builtin", "raa"));
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
	let mut session: Session = python::session("-2 ** 2", &BTreeMap::new()).unwrap();
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
			.replacen("flexible-substitution-v4", "explicit-groups-v1", 1);
	let mut progress: Progress = Progress::default();
	progress
		.sessions
		.insert(old_key.clone(), old_attempts.clone());
	assert!(progress.attempts(&session).is_empty());
	progress.record("", "negative", &session);
	assert_eq!(progress.sessions[&old_key], old_attempts);
	let restored: Session = python::session(session.source(), &BTreeMap::new())
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
	let mut session: Session = python::session("((3))", &BTreeMap::new()).unwrap();
	assert!(!session.remove_group(session.root().id).accepted());
	let inner: usize = session.next_step().unwrap().node_id;
	assert!(session.remove_group(inner).accepted());
	assert_eq!(session.render(), "(3)");
	assert_eq!(session.attempts()[0].input, None);
	let mut progress: Progress = Progress::default();
	progress.record("", "groups", &session);
	progress.save(&path).unwrap();
	let saved: Progress = Progress::load(&path).unwrap();
	let restored: Session = python::session(session.source(), &BTreeMap::new())
		.unwrap()
		.replay(saved.attempts(&session))
		.unwrap();
	assert_eq!(restored.render(), "(3)");
	assert_eq!(restored.root(), session.root());
	let mut arithmetic: Session = python::session("2 + 3", &BTreeMap::new()).unwrap();
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
		let mut session: Session = python::session("(x)", &bindings).unwrap();
		assert!(progress.attempts(&session).is_empty());
		let id: usize = session.next_step().unwrap().node_id;
		assert!(session.submit(id, literal).accepted());
		progress.record("", "typed", &session);
		let restored: Session = python::session("(x)", &bindings)
			.unwrap()
			.replay(progress.attempts(&session))
			.unwrap();
		assert_eq!(restored.render(), session.render());
		assert!(!restored.is_finished(), "group removal is still required");
	}
	assert_eq!(progress.sessions.len(), 7);
}

/// `progress-v1-strategies.json` is shaped like the file the last version-1 build left
/// behind: the student short-circuited the embedded `short-circuit` question, switched to
/// eager evaluation and computed the division the short circuit skips, quit with the pointer
/// on that question in eager mode, and had begun a reductio. Version 3 drops the strategy and
/// nothing else: both evaluation records and the proof's line come back verbatim under their
/// old keys, the pointer is kept, and saving writes version 3 without a strategy while
/// carrying every old record along unchanged.
#[test]
fn a_version_one_file_loads_without_its_strategy_and_keeps_every_record_verbatim() {
	let text: String = fs::read_to_string(fixture(STRATEGIES)).unwrap();
	let written: serde_json::Value = serde_json::from_str(&text).unwrap();
	assert_eq!(written["version"], 1);
	assert_eq!(written["mode"], "eager");
	let loaded: Progress = Progress::load(&fixture(STRATEGIES)).unwrap();
	assert_eq!(fs::read_to_string(fixture(STRATEGIES)).unwrap(), text);

	assert!(loaded.points_at("builtin", "short-circuit"));
	let short_key: &str =
		"flexible-substitution-v3\nshort-circuit\npython\n{}\nFalse and (3 / 0 > 1)";
	let eager_key: &str = "flexible-substitution-v3\neager\npython\n{}\nFalse and (3 / 0 > 1)";
	assert_eq!(
		loaded.sessions,
		BTreeMap::from([
			(
				short_key.to_string(),
				vec![RecordedAttempt {
					node_id: 0,
					input: Some("False".into()),
				}],
			),
			(
				eager_key.to_string(),
				vec![RecordedAttempt {
					node_id: 3,
					input: Some("ZeroDivisionError".into()),
				}],
			),
		])
	);
	assert_eq!(
		loaded.proofs,
		BTreeMap::from([(
			"proof\n[Not(Not(Atom(\"P\")))]\nP".to_string(),
			vec!["~P ; assume".to_string()],
		)])
	);

	// Both records belong to this very question — its key under today's rules is theirs
	// without the strategy — and neither replays into it, because they were taken under
	// other rules.
	let embedded: QuestionSet = exercises::builtin().unwrap();
	let Some(Question::Evaluation(exercise)) = embedded.find("short-circuit") else {
		panic!("short-circuit is an embedded evaluation question");
	};
	let question: Session = exercise.session().unwrap();
	for (mode, key) in [("short-circuit", short_key), ("eager", eager_key)] {
		assert_eq!(
			question.progress_key().replacen(
				"flexible-substitution-v4\n",
				&format!("flexible-substitution-v3\n{mode}\n"),
				1
			),
			key
		);
	}
	assert!(loaded.attempts(&question).is_empty());

	// A proof record never carried a strategy, so it replays as before.
	let Some(Question::Proof(raa)) = embedded.find("raa") else {
		panic!("raa is an embedded proof question");
	};
	let initial: Proof = raa.proof().unwrap();
	let resumed: Proof = initial.clone().replay(loaded.commands(&initial)).unwrap();
	assert_eq!(resumed.commands(), ["~P ; assume"]);
	assert!(!resumed.is_finished());

	// Saved, it is a version-3 file with no strategy, and every old record rides along.
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: std::path::PathBuf = directory.path().join("progress.json");
	loaded.save(&path).unwrap();
	let saved_text: String = fs::read_to_string(&path).unwrap();
	assert!(!saved_text.contains("\"mode\""), "{saved_text}");
	let saved: serde_json::Value = serde_json::from_str(&saved_text).unwrap();
	let mut fields: Vec<&str> = saved
		.as_object()
		.unwrap()
		.keys()
		.map(String::as_str)
		.collect();
	fields.sort_unstable();
	assert_eq!(
		fields,
		["current", "current_set", "proofs", "sessions", "version"]
	);
	assert_eq!(saved["version"], 3);
	for field in ["current_set", "current", "sessions", "proofs"] {
		assert_eq!(saved[field], written[field], "{field}");
	}
	let reloaded: Progress = Progress::load(&path).unwrap();
	assert!(reloaded.points_at("builtin", "short-circuit"));
	assert_eq!(reloaded.sessions, loaded.sessions);
	assert_eq!(reloaded.proofs, loaded.proofs);
}

/// Nothing is recorded under today's key for the question the version-1 pointer names, so it
/// opens from its start — though both old records finished it, which is why a version-1 build
/// walking the set would have passed it by. Work done now lands under the new key beside the
/// old records, which stay exactly as they were.
#[test]
fn the_question_a_version_one_pointer_names_opens_from_its_start() {
	let loaded: Progress = Progress::load(&fixture(STRATEGIES)).unwrap();
	let questions: Vec<Question> = exercises::builtin().unwrap().of_language(Language::Python);
	let index: usize = app::resume_in_set(&loaded, &questions).unwrap();
	assert_eq!(questions[index].name(), "short-circuit");
	let exercise: Exercise = questions[index].evaluation().unwrap().clone();

	// Named on its own, as --exercise opens it, it starts from its source too.
	let named: Practice = Practice::new(exercise.clone(), loaded.clone()).unwrap();
	assert!(named.session().attempts().is_empty());
	assert_eq!(named.session().render(), exercise.expression);

	let mut lesson: Lesson = Lesson::new(
		Course::ordered(questions.clone(), index).unwrap(),
		loaded.clone(),
	)
	.unwrap();
	let Task::Evaluation(practice) = lesson.task_mut() else {
		panic!("short-circuit is an evaluation question");
	};
	assert!(practice.session().attempts().is_empty());
	assert!(practice.session().history().is_empty());
	assert!(!practice.session().is_finished());
	assert_eq!(practice.session().render(), exercise.expression);

	// Short-circuit it through the operations a front end calls.
	let root: usize = practice.session().root().id;
	assert!(!practice.select(root));
	assert_eq!(practice.draft(), Some(root));
	practice.paste("False");
	assert!(practice.submit());
	assert!(practice.session().is_finished());
	lesson.record();

	let recorded: &Progress = lesson.progress();
	assert!(recorded.points_at("builtin", "short-circuit"));
	assert_eq!(recorded.sessions.len(), loaded.sessions.len() + 1);
	for (key, attempts) in &loaded.sessions {
		assert_eq!(&recorded.sessions[key], attempts, "{key}");
	}
	let fresh: Session = exercise.session().unwrap();
	assert!(
		fresh
			.progress_key()
			.starts_with("flexible-substitution-v4\n")
	);
	assert_eq!(
		recorded.attempts(&fresh),
		[RecordedAttempt {
			node_id: root,
			input: Some("False".into()),
		}]
	);
	assert_eq!(recorded.proofs, loaded.proofs);

	// Written out and read back, the set now counts it finished and moves on.
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: std::path::PathBuf = directory.path().join("progress.json");
	recorded.save(&path).unwrap();
	let reloaded: Progress = Progress::load(&path).unwrap();
	for (key, attempts) in &loaded.sessions {
		assert_eq!(&reloaded.sessions[key], attempts, "{key}");
	}
	assert_eq!(
		app::resume_in_set(&reloaded, &questions).unwrap(),
		index + 1
	);
}

/// This build writes version 3 and reads the two versions that saved a strategy: 1, and 2,
/// which the first question-set commits wrote in exactly the same shape before the number was
/// set back to 1. Any other number is refused with the reason, not with whichever field the
/// reader tripped over, and the file is left exactly as it was. So is a strategy-carrying file
/// relabelled 3: version 3 has no strategy, and a field it does not have is refused rather
/// than dropped.
#[test]
fn other_progress_versions_are_refused_and_left_untouched() {
	let original: String = fs::read_to_string(fixture(STRATEGIES)).unwrap();
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: std::path::PathBuf = directory.path().join("progress.json");
	for version in [0, 4] {
		let text: String =
			original.replacen("\"version\": 1", &format!("\"version\": {version}"), 1);
		assert_ne!(text, original);
		fs::write(&path, &text).unwrap();
		assert_eq!(
			Progress::load(&path).unwrap_err().to_string(),
			UNSUPPORTED,
			"version {version}"
		);
		assert_eq!(fs::read_to_string(&path).unwrap(), text);
	}

	let two: String = original.replacen("\"version\": 1", "\"version\": 2", 1);
	fs::write(&path, &two).unwrap();
	let loaded: Progress = Progress::load(&path).unwrap();
	let one: Progress = Progress::load(&fixture(STRATEGIES)).unwrap();
	assert_eq!(loaded.sessions, one.sessions);
	assert_eq!(loaded.proofs, one.proofs);
	assert!(loaded.points_at(&one.current_set, &one.current));
	assert_eq!(fs::read_to_string(&path).unwrap(), two);

	let relabelled: String = original.replacen("\"version\": 1", "\"version\": 3", 1);
	fs::write(&path, &relabelled).unwrap();
	let error: String = Progress::load(&path).unwrap_err().to_string();
	assert!(error.contains("unknown field `mode`"), "{error}");
	assert_eq!(fs::read_to_string(&path).unwrap(), relabelled);
}

/// A version-1 file is read by the version-1 shape, not guessed at: one missing the strategy
/// every version-1 build wrote, or carrying a field none of them wrote, is refused and left
/// alone.
#[test]
fn a_version_one_file_without_its_strategy_or_with_an_unknown_field_is_refused() {
	let original: String = fs::read_to_string(fixture(STRATEGIES)).unwrap();
	let strategy: &str = "  \"mode\": \"eager\",\n";
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: std::path::PathBuf = directory.path().join("progress.json");
	for (text, reason) in [
		(original.replacen(strategy, "", 1), "missing field `mode`"),
		(
			original.replacen(
				strategy,
				&format!("{strategy}  \"strategy\": \"eager\",\n"),
				1,
			),
			"unknown field `strategy`",
		),
	] {
		assert_ne!(text, original);
		fs::write(&path, &text).unwrap();
		let error: String = Progress::load(&path).unwrap_err().to_string();
		assert!(error.contains(reason), "{error}");
		assert_eq!(fs::read_to_string(&path).unwrap(), text);
	}
}

/// Every version-1 build wrote its strategy as `short-circuit` or `eager` and refused any
/// other value. Dropping the field on load is no licence to accept a value no build wrote.
#[test]
fn a_version_one_strategy_no_build_wrote_is_refused() {
	let original: String = fs::read_to_string(fixture(STRATEGIES)).unwrap();
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: std::path::PathBuf = directory.path().join("progress.json");
	for value in ["\"lazy\"", "42", "null"] {
		let text: String =
			original.replacen("\"mode\": \"eager\"", &format!("\"mode\": {value}"), 1);
		assert_ne!(text, original);
		fs::write(&path, &text).unwrap();
		assert!(Progress::load(&path).is_err(), "mode {value} loaded");
		assert_eq!(fs::read_to_string(&path).unwrap(), text);
	}
}
