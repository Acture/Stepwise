//! The TOML question-set protocol and the `--set` entry that loads one. Every question
//! reaches the program through `QuestionSet::parse`, so these tests hold the embedded set,
//! the shipped example and hand-written files to exactly the same rules.

use std::{
	fs,
	path::PathBuf,
	process::{Command, Output},
};

use stepwise::{
	app::{self, ProofPractice},
	core::{EvaluationMode, ExprKind, Feedback, Language, Session, Value},
	exercises::{self, Exercise, Invalid, ProofQuestion, Question, QuestionSet, SetError},
	logic::proof::Proof,
	progress::Progress,
};

/// The shipped example, reached both as text (parsing) and as a path (the CLI).
const EXAMPLE_TEXT: &str = include_str!("../questions/example.toml");
const EXAMPLE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/questions/example.toml");

/// A well-formed Python question; a rejection fixture spoils exactly one part of it.
const PYTHON_FIELDS: &str = "name = \"q1\"\ntitle = \"一道题\"\nlanguage = \"python\"\nexpression = \"1 + 2\"\nnote = \"题面\"";
/// A well-formed proof question, for the same purpose.
const PROOF_FIELDS: &str = "name = \"p1\"\ntitle = \"一道证明题\"\npremises = [\"P -> Q\", \"P\"]\nconclusion = \"Q\"\nnote = \"题面\"";

/// Without a terminal each practice refuses before drawing, and each says so in its own
/// words — which is how a test tells which kind of question a launch opened. Neither sentence
/// contains the other, so seeing one rules the other out.
const PROOF_OPENED: &str = "自然演绎练习需要交互终端";
const EVALUATION_OPENED: &str = "交互练习需要终端";

fn header(id: &str, title: &str) -> String {
	format!("version = 1\nname = \"{id}\"\ntitle = \"{title}\"\n")
}

/// A set with the usual valid metadata and whatever body the test is about.
fn set(body: &str) -> String {
	format!("{}{body}", header("probe", "探针题集"))
}

fn evaluation(fields: &str) -> String {
	format!("\n[[questions]]\nkind = \"evaluation\"\n{fields}\n")
}

fn proof_question(fields: &str) -> String {
	format!("\n[[questions]]\nkind = \"proof\"\n{fields}\n")
}

/// The validation failure a set was refused with; TOML's own refusals are a different
/// branch, and a test that mixes them up would assert on the wrong words.
fn invalid(text: &str) -> Invalid {
	match QuestionSet::parse(text).unwrap_err() {
		SetError::Invalid(invalid) => invalid,
		SetError::Syntax(error) => {
			panic!("expected a validation failure, got TOML syntax: {error}")
		}
	}
}

/// The message TOML itself refused a set with: an unknown field, a missing one, an unknown
/// question type or broken syntax.
fn syntax(text: &str) -> String {
	match QuestionSet::parse(text).unwrap_err() {
		SetError::Syntax(error) => error.to_string(),
		SetError::Invalid(invalid) => panic!("expected TOML to refuse this, got {invalid}"),
	}
}

fn contains(haystack: &str, needle: &str) {
	assert!(
		haystack.contains(needle),
		"{needle:?} missing from {haystack}"
	);
}

/// Work one session through to the end exactly as `--trace` does, so a question that stops
/// at an exception stops here too.
fn finish(mut session: Session) -> Session {
	while let Some(step) = session.next_step() {
		let input: String = match step.outcome {
			Ok(value) => value.to_string(),
			Err(error) => error.name().expect("a raised error is typable").into(),
		};
		let group: bool = matches!(
			session.root().find(step.node_id).map(|node| &node.kind),
			Some(ExprKind::Group(_))
		);
		let feedback: Feedback = if group {
			session.remove_group(step.node_id)
		} else {
			session.submit(step.node_id, &input)
		};
		assert!(feedback.accepted(), "{}", feedback.message);
	}
	session
}

fn cli(args: &[&str]) -> Output {
	Command::new(env!("CARGO_BIN_EXE_stepwise"))
		.args(args)
		.output()
		.unwrap()
}

fn out(output: &Output) -> String {
	String::from_utf8(output.stdout.clone()).unwrap()
}

fn err(output: &Output) -> String {
	String::from_utf8(output.stderr.clone()).unwrap()
}

fn write(directory: &tempfile::TempDir, name: &str, text: &str) -> PathBuf {
	let path: PathBuf = directory.path().join(name);
	fs::write(&path, text).unwrap();
	path
}

// ---------------------------------------------------------------------------
// The embedded set is the default value of the one load path.
// ---------------------------------------------------------------------------

#[test]
fn the_embedded_set_parses_as_version_one_and_stamps_every_question_with_its_own_name() {
	let set: QuestionSet = exercises::builtin().unwrap();
	assert_eq!(set.name, "builtin");
	assert_eq!(set.version(), exercises::FORMAT_VERSION);
	assert_eq!(set.version(), 1);
	assert_eq!(set.questions().len(), 23);
	assert!(set.description.is_some());

	// Twenty evaluation questions, then the three built-in proofs: every question is one or
	// the other, and the file order holds in both views.
	let listed: Vec<&str> = set.questions().iter().map(Question::name).collect();
	let walked: Vec<&str> = set
		.exercises()
		.map(|exercise| exercise.name.as_str())
		.collect();
	let proofs: Vec<&str> = set
		.questions()
		.iter()
		.filter(|question| matches!(question, Question::Proof(_)))
		.map(Question::name)
		.collect();
	assert_eq!(walked.len(), 20);
	assert_eq!(proofs, ["mp", "raa", "identity"]);
	assert_eq!(listed, [walked.as_slice(), proofs.as_slice()].concat());
	// A proof's pointer names its set exactly as an evaluation question's does, so the
	// loader stamps both kinds.
	for question in set.questions() {
		assert_eq!(
			question.set(),
			"builtin",
			"{} lost its set name",
			question.name()
		);
	}

	// Each language's course holds every question of that language, both kinds, and the two
	// courses together are the whole set.
	let python: Vec<Question> = set.of_language(Language::Python);
	let logic: Vec<Question> = set.of_language(Language::Logic);
	assert!(!python.is_empty() && !logic.is_empty());
	assert_eq!(python.len() + logic.len(), 23);
	let split: Vec<&str> = python
		.iter()
		.chain(logic.iter())
		.map(Question::name)
		.collect();
	let mut sorted: Vec<&str> = split.clone();
	sorted.sort_unstable();
	let mut expected: Vec<&str> = listed.clone();
	expected.sort_unstable();
	assert_eq!(sorted, expected);
	assert!(
		python
			.iter()
			.all(|question| question.language() == Language::Python
				&& question.evaluation().is_some())
	);
	assert!(
		logic
			.iter()
			.all(|question| question.language() == Language::Logic)
	);
	// The logic course walks the file: its evaluation questions, then the proofs after them.
	let logic_evaluations: Vec<&str> = set
		.exercises()
		.filter(|exercise| exercise.language == Language::Logic)
		.map(|exercise| exercise.name.as_str())
		.collect();
	assert_eq!(
		logic.iter().map(Question::name).collect::<Vec<&str>>(),
		[logic_evaluations.as_slice(), proofs.as_slice()].concat()
	);

	assert!(set.find("precedence").is_some());
	assert!(set.find("no-such-question").is_none());
}

// ---------------------------------------------------------------------------
// The protocol: what a file may say, and where it lands.
// ---------------------------------------------------------------------------

#[test]
fn one_set_carries_python_logic_and_proof_questions_with_their_fields_intact() {
	let text: String = format!(
		"{}{}{}{}",
		header("mixed", "混合题集"),
		evaluation(
			"name = \"py\"\ntitle = \"除法\"\nlanguage = \"python\"\nexpression = \"guard and (count / step)\"\nnote = \"题面一\"\nevaluation = \"eager\"\n[questions.bindings]\nguard = \"False\"\ncount = \"6\"\nstep = \"3\""
		),
		evaluation(
			"name = \"lg\"\ntitle = \"真值\"\nlanguage = \"logic\"\nexpression = \"P ∧ Q\"\nnote = \"题面二\"\n[questions.bindings]\nP = \"True\"\nQ = \"False\""
		),
		proof_question(PROOF_FIELDS),
	);
	let set: QuestionSet = QuestionSet::parse(&text).unwrap();
	assert_eq!(set.questions().len(), 3);
	assert_eq!(set.exercises().count(), 2);

	let python: &Exercise = set.find("py").unwrap().evaluation().unwrap();
	assert_eq!(python.set, "mixed");
	assert_eq!(python.language, Language::Python);
	assert_eq!(python.expression, "guard and (count / step)");
	assert_eq!(python.note.as_deref(), Some("题面一"));
	assert_eq!(python.evaluation, Some(EvaluationMode::Eager));
	assert_eq!(python.mode(), EvaluationMode::Eager);
	assert_eq!(python.assignments(), "count=6 guard=False step=3");

	let logic: &Exercise = set.find("lg").unwrap().evaluation().unwrap();
	assert_eq!(logic.language, Language::Logic);
	// No `evaluation` field, so the language's own default decides.
	assert_eq!(logic.evaluation, None);
	assert_eq!(logic.mode(), Language::Logic.default_mode());

	let question: &Question = set.find("p1").unwrap();
	assert_eq!(question.language(), Language::Logic);
	assert_eq!(question.title(), "一道证明题");
	assert!(question.evaluation().is_none());
	let Question::Proof(proof_question) = question else {
		panic!("kind = \"proof\" must deserialize as a proof question");
	};
	assert_eq!(proof_question.premises, ["P -> Q", "P"]);
	assert_eq!(proof_question.conclusion, "Q");
	assert_eq!(proof_question.note.as_deref(), Some("题面"));
	assert_eq!(proof_question.sequent(), "P -> Q，P ⊢ Q");
	let proof: Proof = proof_question.proof().unwrap();
	assert_eq!(proof.lines().len(), 2);
	assert_eq!(proof.goal.to_string(), "Q");
	assert!(!proof.is_finished());
}

#[test]
fn binding_literals_keep_their_python_type_and_their_sign() {
	let parsed = |literal: &str| -> Value {
		let text: String = set(&evaluation(&format!(
			"name = \"q\"\ntitle = \"取值\"\nlanguage = \"python\"\nexpression = \"x\"\nnote = \"题面\"\n[questions.bindings]\nx = \"{literal}\""
		)));
		let exercise: Exercise = QuestionSet::parse(&text)
			.unwrap()
			.find("q")
			.unwrap()
			.evaluation()
			.unwrap()
			.clone();
		let session: Session = exercise.session(exercise.mode()).unwrap();
		session
			.next_step()
			.expect("substituting the name is the first step")
			.outcome
			.expect("a literal never raises")
	};

	let zero: Value = parsed("0");
	let float_zero: Value = parsed("0.0");
	let negative_zero: Value = parsed("-0.0");
	let truth: Value = parsed("True");
	let falsehood: Value = parsed("False");
	let nothing: Value = parsed("None");

	assert!(matches!(zero, Value::Int(_)), "{zero:?}");
	assert_eq!(zero.to_string(), "0");
	assert!(matches!(float_zero, Value::Float(_)), "{float_zero:?}");
	assert_eq!(float_zero.to_string(), "0.0");
	assert!(matches!(nothing, Value::None), "{nothing:?}");
	assert_eq!(truth, Value::Bool(true));
	assert_eq!(falsehood, Value::Bool(false));

	// "0" is an int and "0.0" a float: same arithmetic value, different answers.
	assert_eq!(zero.type_name(), "int");
	assert_eq!(float_zero.type_name(), "float");
	assert!(!zero.same_answer(&float_zero));
	// "True" is a bool, never the int 1.
	assert_eq!(truth.type_name(), "bool");
	assert!(!truth.same_answer(&Value::Int(1.into())));

	// Negative zero survives the file, the TOML string and the parser.
	assert!(
		matches!(negative_zero, Value::Float(number) if number.is_sign_negative()),
		"{negative_zero:?}"
	);
	assert!(matches!(float_zero, Value::Float(number) if !number.is_sign_negative()));
	assert_eq!(negative_zero.to_string(), "-0.0");
	assert!(!negative_zero.same_answer(&float_zero));
}

// ---------------------------------------------------------------------------
// What a set may not say.
// ---------------------------------------------------------------------------

#[test]
fn only_this_format_version_loads_and_it_must_be_written_down() {
	let wrong: Invalid = invalid(&format!(
		"{}{}",
		header("probe", "探针题集").replace("version = 1", "version = 2"),
		evaluation(PYTHON_FIELDS)
	));
	assert_eq!(wrong.field, "version");
	assert_eq!(wrong.question, None);
	contains(&wrong.to_string(), "题集的 version 字段");
	contains(&wrong.to_string(), "version = 1");

	let missing: String = syntax(&format!(
		"name = \"probe\"\ntitle = \"探针题集\"\n{}",
		evaluation(PYTHON_FIELDS)
	));
	contains(&missing, "version");
}

#[test]
fn an_unknown_field_is_refused_rather_than_ignored() {
	contains(
		&syntax(&format!(
			"{}author = \"someone\"\n{}",
			header("probe", "探针题集"),
			evaluation(PYTHON_FIELDS)
		)),
		"author",
	);
	contains(
		&syntax(&set(&evaluation(&format!(
			"{PYTHON_FIELDS}\nhint = \"先算乘法\""
		)))),
		"hint",
	);
}

#[test]
fn a_proof_field_on_an_evaluation_question_is_refused() {
	for field in ["conclusion = \"Q\"", "premises = [\"P\"]"] {
		let message: String = syntax(&set(&evaluation(&format!("{PYTHON_FIELDS}\n{field}"))));
		let name: &str = field.split(' ').next().unwrap();
		contains(&message, name);
	}
}

/// 证明题不接受短路字段：short circuit is an evaluation strategy and says nothing about a
/// derivation, so the field is refused instead of quietly ignored.
#[test]
fn a_proof_question_refuses_the_short_circuit_field() {
	let message: String = syntax(&set(&proof_question(&format!(
		"{PROOF_FIELDS}\nevaluation = \"eager\""
	))));
	contains(&message, "evaluation");
	// The same file without that one line is a set the program accepts.
	QuestionSet::parse(&set(&proof_question(PROOF_FIELDS))).unwrap();
}

#[test]
fn an_evaluation_only_field_on_a_proof_question_is_refused() {
	for field in [
		"expression = \"P ∧ Q\"",
		"language = \"logic\"",
		"[questions.bindings]\nP = \"True\"",
	] {
		let message: String = syntax(&set(&proof_question(&format!("{PROOF_FIELDS}\n{field}"))));
		let name: &str = if field.starts_with('[') {
			"bindings"
		} else {
			field.split(' ').next().unwrap()
		};
		contains(&message, name);
	}
}

#[test]
fn the_set_name_is_stamped_by_the_loader_and_cannot_be_written_in_the_file() {
	contains(
		&syntax(&set(&evaluation(&format!(
			"{PYTHON_FIELDS}\nset = \"someone-elses-set\""
		)))),
		"set",
	);
	// It is stamped instead, from the set's own id.
	let loaded: QuestionSet = QuestionSet::parse(&set(&evaluation(PYTHON_FIELDS))).unwrap();
	assert_eq!(loaded.exercises().next().unwrap().set, "probe");
}

#[test]
fn a_question_states_its_kind_and_only_a_known_one() {
	let missing: String = syntax(&set(&format!("\n[[questions]]\n{PYTHON_FIELDS}\n")));
	contains(&missing, "kind");

	let unknown: String = syntax(&set(&format!(
		"\n[[questions]]\nkind = \"quiz\"\n{PYTHON_FIELDS}\n"
	)));
	contains(&unknown, "quiz");
}

#[test]
fn question_names_are_present_unique_and_titles_are_not_blank() {
	let duplicate: Invalid = invalid(&set(&format!(
		"{}{}",
		evaluation(PYTHON_FIELDS),
		evaluation(&PYTHON_FIELDS.replace("expression = \"1 + 2\"", "expression = \"2 + 3\""))
	)));
	assert_eq!(duplicate.question.as_deref(), Some("q1"));
	assert_eq!(duplicate.field, "name");
	contains(&duplicate.to_string(), "题目 q1 的 name 字段");
	contains(&duplicate.to_string(), "重复");

	let blank_name: Invalid = invalid(&set(&evaluation(
		&PYTHON_FIELDS.replace("name = \"q1\"", "name = \"  \""),
	)));
	assert_eq!(blank_name.question.as_deref(), Some("#1"));
	assert_eq!(blank_name.field, "name");
	contains(&blank_name.to_string(), "题目 #1 的 name 字段");
	contains(&blank_name.to_string(), "题目名称不能为空");

	let blank_title: Invalid = invalid(&set(&evaluation(
		&PYTHON_FIELDS.replace("title = \"一道题\"", "title = \"\""),
	)));
	assert_eq!(blank_title.question.as_deref(), Some("q1"));
	assert_eq!(blank_title.field, "title");
	contains(&blank_title.to_string(), "题目标题不能为空");
}

#[test]
fn a_set_needs_a_name_a_title_and_at_least_one_question() {
	let blank_name: Invalid = invalid(&format!(
		"{}{}",
		header("", "探针题集"),
		evaluation(PYTHON_FIELDS)
	));
	assert_eq!(blank_name.question, None);
	assert_eq!(blank_name.field, "name");
	contains(&blank_name.to_string(), "题集名称不能为空");

	let blank_title: Invalid = invalid(&format!(
		"{}{}",
		header("probe", ""),
		evaluation(PYTHON_FIELDS)
	));
	assert_eq!(blank_title.field, "title");
	contains(&blank_title.to_string(), "题集标题不能为空");

	let empty: Invalid = invalid(&header("probe", "探针题集"));
	assert_eq!(empty.field, "questions");
	contains(&empty.to_string(), "题集里没有题目");
}

#[test]
fn a_binding_must_be_a_source_literal_of_its_own_language() {
	let python: Invalid = invalid(&set(&evaluation(
		"name = \"q1\"\ntitle = \"一道题\"\nlanguage = \"python\"\nexpression = \"x + 1\"\nnote = \"题面\"\n[questions.bindings]\nx = \"maybe\"",
	)));
	assert_eq!(python.question.as_deref(), Some("q1"));
	assert_eq!(python.field, "bindings");
	contains(&python.to_string(), "x = \"maybe\" 不是可用的取值");
	contains(&python.to_string(), "Python 源码字面量");

	let logic: Invalid = invalid(&set(&evaluation(
		"name = \"q2\"\ntitle = \"一道题\"\nlanguage = \"logic\"\nexpression = \"P ∧ Q\"\nnote = \"题面\"\n[questions.bindings]\nP = \"True\"\nQ = \"perhaps\"",
	)));
	assert_eq!(logic.question.as_deref(), Some("q2"));
	assert_eq!(logic.field, "bindings");
	contains(&logic.to_string(), "Q = \"perhaps\" 不是可用的取值");
	contains(&logic.to_string(), "真值 True / False");
}

#[test]
fn an_expression_must_parse_and_every_name_in_it_must_be_bound() {
	let broken: Invalid = invalid(&set(&evaluation(
		&PYTHON_FIELDS.replace("expression = \"1 + 2\"", "expression = \"1 + \""),
	)));
	assert_eq!(broken.question.as_deref(), Some("q1"));
	assert_eq!(broken.field, "expression");
	contains(&broken.to_string(), "语法错误");

	let unbound: Invalid = invalid(&set(&evaluation(
		&PYTHON_FIELDS.replace("expression = \"1 + 2\"", "expression = \"x + 1\""),
	)));
	assert_eq!(unbound.field, "expression");
	contains(&unbound.to_string(), "未给变量 x 赋值");
}

#[test]
fn every_premise_and_the_conclusion_must_be_a_formula() {
	let premise: Invalid = invalid(&set(&proof_question(&PROOF_FIELDS.replace(
		"premises = [\"P -> Q\", \"P\"]",
		"premises = [\"P -> Q\", \"P ->\"]",
	))));
	assert_eq!(premise.question.as_deref(), Some("p1"));
	assert_eq!(premise.field, "premises");
	contains(&premise.to_string(), "第 2 条「P ->」");

	let conclusion: Invalid = invalid(&set(&proof_question(
		&PROOF_FIELDS.replace("conclusion = \"Q\"", "conclusion = \"Q ∧\""),
	)));
	assert_eq!(conclusion.question.as_deref(), Some("p1"));
	assert_eq!(conclusion.field, "conclusion");
}

#[test]
fn broken_toml_is_reported_with_the_line_and_column_to_fix() {
	let message: String = syntax("version = 1\nid = =\"probe\"\ntitle = \"t\"\n");
	contains(&message, "line");
	contains(&message, "column");
	contains(&message, "2");
}

#[test]
fn a_set_is_refused_when_it_holds_too_many_questions_or_too_many_bytes() {
	let question = |index: usize| -> String {
		evaluation(&format!(
			"name = \"q{index}\"\ntitle = \"第 {index} 题\"\nlanguage = \"python\"\nexpression = \"1 + {index}\"\nnote = \"题面\""
		))
	};
	let at_limit: String = set(&(0..1024).map(question).collect::<String>());
	assert_eq!(
		QuestionSet::parse(&at_limit).unwrap().questions().len(),
		1024
	);

	let over: Invalid = invalid(&set(&(0..1025).map(question).collect::<String>()));
	assert_eq!(over.field, "questions");
	contains(&over.to_string(), "1025");
	contains(&over.to_string(), "超过上限 1024");

	// Valid TOML, refused on weight alone: the size rule is its own check.
	let heavy: String = format!(
		"version = 1\nname = \"heavy\"\ntitle = \"t\"\ndescription = \"{}\"\n{}",
		"x".repeat(1 << 20),
		evaluation(PYTHON_FIELDS)
	);
	assert!(toml::from_str::<toml::Table>(&heavy).is_ok());
	let weighed: Invalid = invalid(&heavy);
	// No single field is at fault here, so the message names the file instead of inventing one.
	assert_eq!(weighed.question, None);
	assert_eq!(weighed.field, "");
	contains(&weighed.to_string(), "题集文件：");
	contains(&weighed.to_string(), "字节");
	contains(&weighed.to_string(), "超过上限");
}

// ---------------------------------------------------------------------------
// A question that ends in an exception is teaching material, not a broken file.
// ---------------------------------------------------------------------------

#[test]
fn a_question_that_ends_in_an_exception_loads_and_is_practised_to_that_exception() {
	let text: String = set(&evaluation(
		"name = \"boom\"\ntitle = \"除以零\"\nlanguage = \"python\"\nexpression = \"6 / 0\"\nnote = \"题面\"",
	));
	let set: QuestionSet = QuestionSet::parse(&text).unwrap();
	let exercise: &Exercise = set.find("boom").unwrap().evaluation().unwrap();
	let session: Session = finish(exercise.session(exercise.mode()).unwrap());
	assert_eq!(
		session.terminal_error().and_then(|error| error.name()),
		Some("ZeroDivisionError")
	);
	assert!(session.is_finished());
	assert_eq!(session.history().len(), 1);

	// The shipped one says the same thing, and its `evaluation = "eager"` is why the
	// division on the skipped side runs at all.
	let shipped: QuestionSet = QuestionSet::parse(EXAMPLE_TEXT).unwrap();
	let eager: &Exercise = shipped
		.find("eager-division")
		.unwrap()
		.evaluation()
		.unwrap();
	assert_eq!(eager.mode(), EvaluationMode::Eager);
	let raised: Session = finish(eager.session(eager.mode()).unwrap());
	assert_eq!(
		raised.terminal_error().and_then(|error| error.name()),
		Some("ZeroDivisionError")
	);
	// Under short circuit the same source never reaches the division.
	let skipped: Session = finish(eager.session(EvaluationMode::ShortCircuit).unwrap());
	assert!(skipped.terminal_error().is_none());
}

// ---------------------------------------------------------------------------
// The shipped example, driven through the real rules.
// ---------------------------------------------------------------------------

#[test]
fn the_shipped_example_parses_and_its_proof_is_checked_by_the_same_rules() {
	let set: QuestionSet = QuestionSet::parse(EXAMPLE_TEXT).unwrap();
	assert_eq!(set.name, "example-set");
	assert_eq!(set.version(), 1);
	assert_eq!(set.questions().len(), 4);
	assert_eq!(set.exercises().count(), 3);
	// Each language's course is its questions in file order; the proof is a stop on logic's.
	let course = |language: Language| -> Vec<String> {
		set.of_language(language)
			.iter()
			.map(|question| question.name().to_owned())
			.collect()
	};
	assert_eq!(
		course(Language::Python),
		["literal-types", "eager-division"]
	);
	assert_eq!(course(Language::Logic), ["modus-ponens-truth", "chain"]);

	let Some(Question::Proof(question)) = set.find("chain") else {
		panic!("the example ships a proof question named chain");
	};
	assert_eq!(question.set, "example-set");
	assert_eq!(question.sequent(), "P -> Q，Q -> R，P ⊢ R");
	let mut practice: ProofPractice =
		ProofPractice::new(question.clone(), Progress::default()).unwrap();
	assert_eq!(practice.proof().lines().len(), 3);
	assert!(!practice.is_finished());

	// A rule that does not fit the cited lines is refused, and adds no line.
	practice.paste("Q ; and-left ; 1");
	assert!(!practice.submit());
	assert_eq!(practice.proof().lines().len(), 3);
	assert_eq!(practice.input(), "Q ; and-left ; 1");
	practice.clear_input();

	// So is a reference to a line that does not exist.
	practice.paste("Q ; mp ; 1,9");
	assert!(!practice.submit());
	assert_eq!(practice.proof().lines().len(), 3);
	practice.clear_input();

	// Two applications of modus ponens finish it.
	practice.paste("Q ; mp ; 1,3");
	assert!(practice.submit());
	assert_eq!(practice.proof().lines().len(), 4);
	assert!(!practice.is_finished());
	practice.paste("R ; mp ; 2,4");
	assert!(practice.submit());
	assert!(practice.is_finished());
	assert!(practice.proof().open_assumptions().is_empty());
}

// ---------------------------------------------------------------------------
// The CLI entry.
// ---------------------------------------------------------------------------

#[test]
fn listing_an_external_set_shows_only_the_questions_of_the_chosen_language() {
	let python: Output = cli(&["--python", "--set", EXAMPLE_PATH, "--list"]);
	assert!(python.status.success());
	let listed: String = out(&python);
	assert_eq!(listed.lines().count(), 2);
	contains(&listed, "literal-types");
	contains(&listed, "eager-division");
	contains(&listed, "flag + (zero == negative)");
	assert!(!listed.contains("chain"), "{listed}");
	assert!(err(&python).is_empty());

	let logic: Output = cli(&["--logic", "--set", EXAMPLE_PATH, "--list"]);
	assert!(logic.status.success());
	let listed: String = out(&logic);
	assert_eq!(listed.lines().count(), 2);
	contains(&listed, "modus-ponens-truth");
	// A proof question lists its sequent where an evaluation question lists its source.
	contains(&listed, "chain");
	contains(&listed, "P -> Q，Q -> R，P ⊢ R");
	assert!(!listed.contains("literal-types"), "{listed}");
}

#[test]
fn a_set_with_nothing_for_the_chosen_language_says_so_instead_of_showing_nothing() {
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: PathBuf = write(
		&directory,
		"python-only.toml",
		&format!(
			"{}{}",
			header("only-python", "只有 Python"),
			evaluation(PYTHON_FIELDS)
		),
	);
	let file: &str = path.to_str().unwrap();

	let listed: Output = cli(&["--logic", "--set", file, "--list"]);
	assert!(listed.status.success(), "--list still exits 0");
	assert!(out(&listed).is_empty());
	contains(&err(&listed), "题集 only-python 里没有 --logic 的题目");

	let launched: Output = cli(&["--logic", "--set", file, "--no-save"]);
	assert!(!launched.status.success());
	contains(&err(&launched), "题集 only-python 里没有 --logic 的题目");
}

#[test]
fn naming_a_question_resolves_against_the_loaded_set_and_says_why_when_it_cannot() {
	let found: Output = cli(&[
		"--python",
		"--set",
		EXAMPLE_PATH,
		"--trace",
		"--exercise",
		"literal-types",
	]);
	assert!(found.status.success(), "{}", err(&found));
	contains(&out(&found), "flag + (zero == negative)");

	let missing: Output = cli(&[
		"--python",
		"--set",
		EXAMPLE_PATH,
		"--trace",
		"--exercise",
		"precedence",
	]);
	assert!(!missing.status.success(), "the embedded set is replaced");
	contains(&err(&missing), "题集 example-set 里没有题目 precedence");
	contains(&err(&missing), "--python --list");

	let other_language: Output = cli(&[
		"--python",
		"--set",
		EXAMPLE_PATH,
		"--trace",
		"--exercise",
		"modus-ponens-truth",
	]);
	assert!(!other_language.status.success());
	contains(
		&err(&other_language),
		"题目 modus-ponens-truth 是 --logic 的题目，当前是 --python。",
	);

	let proof_under_python: Output = cli(&[
		"--python",
		"--set",
		EXAMPLE_PATH,
		"--trace",
		"--exercise",
		"chain",
	]);
	assert!(!proof_under_python.status.success());
	contains(
		&err(&proof_under_python),
		"题目 chain 是证明题，请改用 --logic。",
	);

	let traced_proof: Output = cli(&[
		"--logic",
		"--set",
		EXAMPLE_PATH,
		"--trace",
		"--exercise",
		"chain",
	]);
	assert!(!traced_proof.status.success());
	contains(&err(&traced_proof), "证明题没有逐步演示");
}

#[test]
fn trace_on_a_question_from_a_file_prints_the_steps_the_rules_derived() {
	let traced: Output = cli(&[
		"--python",
		"--set",
		EXAMPLE_PATH,
		"--trace",
		"--exercise",
		"eager-division",
	]);
	assert!(traced.status.success(), "{}", err(&traced));
	let printed: String = out(&traced);
	// The assignments, then one numbered line per derived step, then the outcome.
	contains(&printed, "count=6 guard=False step=0");
	contains(&printed, "guard and (count / step)");
	contains(&printed, "1. 将「guard」替换为 False");
	contains(&printed, "将「6 / 0」替换为 ZeroDivisionError");
	contains(&printed, "完成：ZeroDivisionError");
	assert!(printed.contains("4. "), "{printed}");
}

/// Every one of these names a question the set does not hold. `--proof NAME` is not among
/// them: like `--exercise`, it names a question of whichever set is loaded.
#[test]
fn a_set_cannot_be_combined_with_a_question_from_anywhere_else() {
	for conflicting in [
		vec!["--python", "--set", EXAMPLE_PATH, "1 + 1"],
		vec!["--python", "--set", EXAMPLE_PATH, "--random"],
		vec!["--python", "--set", EXAMPLE_PATH, "--seed", "1"],
		vec!["--logic", "--set", EXAMPLE_PATH, "--goal", "P"],
		vec!["--logic", "--set", EXAMPLE_PATH, "--premise", "P"],
		vec!["--logic", "--set", EXAMPLE_PATH, "--equivalent", "P"],
	] {
		let refused: Output = cli(&conflicting);
		assert_eq!(
			refused.status.code(),
			Some(2),
			"{conflicting:?} must be refused by the argument parser"
		);
	}
}

#[test]
fn a_set_that_does_not_load_never_touches_the_progress_file() {
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	// Bytes that a load would refuse and a save would replace, so any read-then-write of
	// this file shows up immediately.
	let progress: PathBuf = write(&directory, "progress.json", "not json at all\n");
	let before: Vec<u8> = fs::read(&progress).unwrap();

	let broken: PathBuf = write(&directory, "broken.toml", "version = 1\nid = =\"x\"\n");
	let unteachable: PathBuf = write(
		&directory,
		"unteachable.toml",
		&set(&evaluation(
			&PYTHON_FIELDS.replace("expression = \"1 + 2\"", "expression = \"x + 1\""),
		)),
	);

	for path in [&broken, &unteachable] {
		let refused: Output = cli(&[
			"--python",
			"--set",
			path.to_str().unwrap(),
			"--progress-file",
			progress.to_str().unwrap(),
		]);
		assert!(!refused.status.success(), "{}", out(&refused));
		contains(&err(&refused), "题集 ");
		assert_eq!(
			fs::read(&progress).unwrap(),
			before,
			"loading the set must happen before the progress file is read"
		);
	}

	let unreadable: Output = cli(&[
		"--python",
		"--set",
		directory.path().join("absent.toml").to_str().unwrap(),
		"--progress-file",
		progress.to_str().unwrap(),
	]);
	assert!(!unreadable.status.success());
	contains(&err(&unreadable), "无法读取题集 ");
	assert_eq!(fs::read(&progress).unwrap(), before);
}

// ---------------------------------------------------------------------------
// What only a file has to answer, and what the file's own name decides.
// ---------------------------------------------------------------------------

/// The version field exists so a set from another protocol says so. Reading it only after
/// the whole document would bury that reason under the new fields the set carries.
#[test]
fn a_set_from_a_later_protocol_names_the_version_even_though_its_fields_are_unknown_here() {
	let later: String = format!(
		"version = 2\nname = \"probe\"\ntitle = \"探针题集\"\ndifficulty = \"hard\"\n{}",
		evaluation(PYTHON_FIELDS)
	);
	// The unknown field alone would be a TOML refusal; the version is read before it.
	let refused: Invalid = invalid(&later);
	assert_eq!(refused.question, None);
	assert_eq!(refused.field, "version");
	contains(&refused.to_string(), "version = 1");
	contains(&refused.to_string(), "文件写的是 2");
	assert!(!refused.to_string().contains("difficulty"));
}

/// Progress is kept per set name, so a file that called itself the embedded set would
/// reopen the embedded questions' work. `parse` is the shared path and still reads it —
/// that is how the embedded set itself loads — and `import` adds the rule a file answers.
#[test]
fn an_imported_file_may_not_call_itself_the_embedded_set() {
	let claiming: String = format!(
		"{}{}",
		header(exercises::BUILTIN_SET, "冒名题集"),
		evaluation(PYTHON_FIELDS)
	);
	assert_eq!(
		QuestionSet::parse(&claiming).unwrap().name,
		exercises::BUILTIN_SET
	);
	let SetError::Invalid(refused) = QuestionSet::import(&claiming).unwrap_err() else {
		panic!("the reserved name is a validation failure, not TOML's")
	};
	assert_eq!(refused.field, "id");
	contains(&refused.to_string(), exercises::BUILTIN_SET);

	// Any other name imports, and the embedded set still loads through `parse`.
	assert!(QuestionSet::import(&set(&evaluation(PYTHON_FIELDS))).is_ok());
	assert_eq!(exercises::builtin().unwrap().name, exercises::BUILTIN_SET);

	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: PathBuf = write(&directory, "claiming.toml", &claiming);
	let refused: Output = cli(&["--python", "--set", path.to_str().unwrap(), "--list"]);
	assert!(!refused.status.success());
	contains(&err(&refused), "留给内置题集");
}

/// The set's own name is its identity, so the same bytes are the same set wherever they sit.
#[test]
fn the_same_set_bytes_are_the_same_set_at_any_path() {
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let here: PathBuf = write(&directory, "here.toml", EXAMPLE_TEXT);
	let moved: PathBuf = write(&directory, "somewhere-else.toml", EXAMPLE_TEXT);
	assert_ne!(here, moved);

	let from_here: Output = cli(&["--python", "--set", here.to_str().unwrap(), "--list"]);
	let from_moved: Output = cli(&["--python", "--set", moved.to_str().unwrap(), "--list"]);
	assert!(from_here.status.success() && from_moved.status.success());
	assert_eq!(out(&from_here), out(&from_moved));

	// The identity the progress pointer uses comes from the file's contents, not its path.
	let parsed: QuestionSet = QuestionSet::import(EXAMPLE_TEXT).unwrap();
	for exercise in parsed.exercises() {
		assert_eq!(exercise.set, parsed.name);
	}
}

/// Without `--exercise`, a set opens at its own first question rather than generating one.
#[test]
fn a_set_opens_its_own_first_question_when_no_question_is_named() {
	let traced: Output = cli(&["--python", "--set", EXAMPLE_PATH, "--trace"]);
	assert!(traced.status.success(), "{}", err(&traced));
	let python: Vec<Question> = QuestionSet::import(EXAMPLE_TEXT)
		.unwrap()
		.of_language(Language::Python);
	let first: &Exercise = python[0]
		.evaluation()
		.expect("the example's first Python question is an evaluation one");
	// --trace prints the assignments, the language and then the source; a generated question
	// would put its own expression on that line.
	let printed: String = out(&traced);
	assert_eq!(printed.lines().nth(2), Some(first.expression.as_str()));
	contains(printed.lines().next().unwrap(), "flag=True");
}

/// A proof question binds no variable, so an assignment is refused rather than accepted and
/// dropped. `--evaluation` is the course's strategy rather than the question's: a course that
/// opens on a proof accepts it for the evaluation questions after it, and opens the proof.
#[test]
fn a_proof_question_refuses_an_assignment_and_leaves_the_strategy_to_the_course() {
	let named: [&str; 6] = [
		"--logic",
		"--set",
		EXAMPLE_PATH,
		"--exercise",
		"chain",
		"--no-save",
	];
	let assigned: Output = cli(&[named.as_slice(), &["--assign", "P=True"]].concat());
	assert_eq!(assigned.status.code(), Some(1), "{}", err(&assigned));
	contains(&err(&assigned), "是证明题");
	contains(&err(&assigned), "--assign");

	let strategy: Output = cli(&[named.as_slice(), &["--evaluation", "eager"]].concat());
	assert_eq!(strategy.status.code(), Some(1), "{}", err(&strategy));
	contains(&err(&strategy), PROOF_OPENED);
	assert!(!err(&strategy).contains("是证明题"), "{}", err(&strategy));
}

/// A language whose only questions are proofs is a course of proofs, not an empty language:
/// the set opens its first proof as it would open any first question. What does not apply to a
/// proof is refused with the reason, and the other language really has nothing.
#[test]
fn a_language_whose_only_questions_are_proofs_is_practised_as_a_course_of_proofs() {
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: PathBuf = write(
		&directory,
		"proofs-only.toml",
		&set(&proof_question(PROOF_FIELDS)),
	);
	let file: &str = path.to_str().unwrap();
	let listed: Output = cli(&["--logic", "--set", file, "--list"]);
	assert!(listed.status.success());
	contains(&out(&listed), "p1");

	// Without a terminal the practice refuses before drawing, which is how the test can tell
	// it reached the proof and not a report that the language has nothing.
	let practice: Output = cli(&["--logic", "--set", file, "--no-save"]);
	assert_eq!(practice.status.code(), Some(1), "{}", err(&practice));
	contains(&err(&practice), PROOF_OPENED);
	assert!(!err(&practice).contains("没有 --logic 的题目"));

	// The refusal names the checker command that finds this proof: --proof looks the name up
	// in whichever set is loaded, so the command carries the set.
	let traced: Output = cli(&["--logic", "--set", file, "--no-save", "--trace"]);
	assert_eq!(traced.status.code(), Some(1), "{}", err(&traced));
	contains(&err(&traced), "证明题没有逐步演示");
	contains(
		&err(&traced),
		&format!("--logic --set {file} --proof p1 --check-proof FILE"),
	);
	assert!(out(&traced).is_empty());

	// The other language really has nothing, and says that instead.
	let python: Output = cli(&["--python", "--set", file, "--list"]);
	assert!(python.status.success());
	assert!(out(&python).is_empty());
	contains(&err(&python), "没有 --python 的题目");
}

// ---------------------------------------------------------------------------
// One course walks evaluation and proof questions alike.
// ---------------------------------------------------------------------------

/// A logic evaluation question for the mixed-course tests below. `P` is bound to False so an
/// `--assign P=True` that reaches it shows in the assignments line.
const LOGIC_FIELDS: &str = "name = \"lg\"\ntitle = \"真值\"\nlanguage = \"logic\"\nexpression = \"P & Q\"\n[questions.bindings]\nP = \"False\"\nQ = \"True\"";
/// A proof one line leaves unfinished and a second finishes, so saved work can stop halfway.
const CHAIN_FIELDS: &str = "name = \"chain\"\ntitle = \"两次肯定前件\"\npremises = [\"P -> Q\", \"Q -> R\", \"P\"]\nconclusion = \"R\"";

/// `--set` walks the file in order whatever kind each question is, so the first question
/// opens even when it is a proof, and an evaluation question first still opens first.
#[test]
fn an_ordered_set_opens_on_its_first_question_whichever_kind_it_is() {
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	for (name, body, opened, not_opened) in [
		(
			"proof-first.toml",
			format!(
				"{}{}",
				proof_question(CHAIN_FIELDS),
				evaluation(LOGIC_FIELDS)
			),
			PROOF_OPENED,
			EVALUATION_OPENED,
		),
		(
			"evaluation-first.toml",
			format!(
				"{}{}",
				evaluation(LOGIC_FIELDS),
				proof_question(CHAIN_FIELDS)
			),
			EVALUATION_OPENED,
			PROOF_OPENED,
		),
	] {
		let path: PathBuf = write(&directory, name, &set(&body));
		let launched: Output = cli(&["--logic", "--set", path.to_str().unwrap(), "--no-save"]);
		assert_eq!(
			launched.status.code(),
			Some(1),
			"{name}: {}",
			err(&launched)
		);
		contains(&err(&launched), opened);
		assert!(
			!err(&launched).contains(not_opened),
			"{name}: {}",
			err(&launched)
		);
	}
}

/// A proof saves the pointer like any question, so an ordered set picks up at an unfinished
/// proof the student left — not at the set's first question — and once that proof is
/// finished it moves on to the question after it.
#[test]
fn an_ordered_set_resumes_at_an_unfinished_proof_and_moves_on_once_it_is_finished() {
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let text: String = set(&format!(
		"{}{}{}",
		evaluation(LOGIC_FIELDS),
		proof_question(CHAIN_FIELDS),
		evaluation(&LOGIC_FIELDS.replace("name = \"lg\"", "name = \"after\""))
	));
	let file: PathBuf = write(&directory, "mixed.toml", &text);
	let progress_path: PathBuf = directory.path().join("progress.json");
	let launch = || -> Output {
		cli(&[
			"--logic",
			"--set",
			file.to_str().unwrap(),
			"--progress-file",
			progress_path.to_str().unwrap(),
		])
	};

	let loaded: QuestionSet = QuestionSet::import(&text).unwrap();
	let questions: Vec<Question> = loaded.of_language(Language::Logic);
	let Some(Question::Proof(chain)) = loaded.find("chain") else {
		panic!("the mixed set holds the proof named chain");
	};
	let mut proof: Proof = chain.proof().unwrap();
	proof.submit("Q ; mp ; 1,3").unwrap();
	assert!(!proof.is_finished());

	// One line in, with the untouched evaluation question still ahead of it in the file.
	let mut progress: Progress = Progress::default();
	progress.record_proof(&chain.set, &chain.name, &proof);
	assert!(progress.points_at("probe", "chain"));
	assert_eq!(progress.commands(&chain.proof().unwrap()).len(), 1);
	progress.save(&progress_path).unwrap();
	assert_eq!(app::resume_in_set(&progress, &questions, None).unwrap(), 1);
	let resumed: Output = launch();
	assert_eq!(resumed.status.code(), Some(1), "{}", err(&resumed));
	contains(&err(&resumed), PROOF_OPENED);

	// Finished, the proof gives way to the question after it.
	proof.submit("R ; mp ; 2,4").unwrap();
	assert!(proof.is_finished());
	progress.record_proof(&chain.set, &chain.name, &proof);
	progress.save(&progress_path).unwrap();
	assert_eq!(app::resume_in_set(&progress, &questions, None).unwrap(), 2);
	let moved_on: Output = launch();
	assert_eq!(moved_on.status.code(), Some(1), "{}", err(&moved_on));
	contains(&err(&moved_on), EVALUATION_OPENED);
}

/// Assignments with no question named belong to an evaluation question: a proof binds no
/// variable. A set whose course opens on a proof still binds them to its evaluation question
/// instead of refusing them, as it did before proofs were stops on the course.
#[test]
fn assignments_with_no_question_named_bind_the_sets_evaluation_question_not_its_proof() {
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: PathBuf = write(
		&directory,
		"proof-first.toml",
		&set(&format!(
			"{}{}",
			proof_question(CHAIN_FIELDS),
			evaluation(LOGIC_FIELDS)
		)),
	);
	let traced: Output = cli(&[
		"--logic",
		"--set",
		path.to_str().unwrap(),
		"--assign",
		"P=True",
		"--trace",
	]);
	assert!(traced.status.success(), "{}", err(&traced));
	// --trace prints the assignments first: the file's P=False, overridden.
	assert_eq!(out(&traced).lines().next(), Some("P=True Q=True"));
	contains(&out(&traced), "P & Q");
}

/// The embedded set keeps assignments with no question named on its current evaluation
/// question, or its first one; a pointer left on an unfinished built-in proof does not pull
/// them onto the proof.
#[test]
fn assignments_with_no_question_named_skip_the_proof_the_embedded_pointer_names() {
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let raa: ProofQuestion = match exercises::builtin().unwrap().find("raa") {
		Some(Question::Proof(question)) => question.clone(),
		other => panic!("raa must be a proof question of the embedded set, got {other:?}"),
	};
	let mut proof: Proof = raa.proof().unwrap();
	proof.submit("~P ; assume").unwrap();
	assert!(!proof.is_finished());
	let mut progress: Progress = Progress::default();
	progress.record_proof(&raa.set, &raa.name, &proof);
	let progress_path: PathBuf = directory.path().join("progress.json");
	progress.save(&progress_path).unwrap();
	let launched: Output = cli(&[
		"--logic",
		"--assign",
		"P=True",
		"--progress-file",
		progress_path.to_str().unwrap(),
	]);
	assert_eq!(launched.status.code(), Some(1), "{}", err(&launched));
	contains(&err(&launched), EVALUATION_OPENED);
	assert!(!err(&launched).contains("是证明题"), "{}", err(&launched));
}

/// README documents the format with a whole TOML file. A documented example that no longer
/// loads is worse than none, so the one in the manual goes through the real loader.
#[test]
fn the_example_set_printed_in_the_readme_is_one_the_program_accepts() {
	const README: &str = include_str!("../README.md");
	// A checkout may hand this file either line ending, and so may a teacher: the block is
	// found and read the same way regardless, and the CRLF form is asserted below.
	let block: String = README
		.split("```toml")
		.nth(1)
		.and_then(|rest| rest.split("```").next())
		.expect("README documents the format with a toml block")
		.replace("\r\n", "\n");
	let documented: QuestionSet = QuestionSet::import(&block).expect("the documented set loads");
	assert_eq!(documented.name, "example-set");

	// Both kinds are shown, and each field the block names is one the loader read.
	assert_eq!(documented.questions().len(), 2);
	let python: &Exercise = documented
		.find("literal-types")
		.and_then(Question::evaluation)
		.expect("the documented evaluation question");
	assert_eq!(python.language, Language::Python);
	assert_eq!(python.mode(), EvaluationMode::Eager);
	assert_eq!(python.bindings["negative"], "-0.0");
	assert!(python.note.is_some());
	let Some(Question::Proof(proof)) = documented.find("chain") else {
		panic!("the documented proof question")
	};
	assert_eq!(proof.conclusion, "R");
	assert_eq!(proof.premises.len(), 3);
	proof.proof().expect("its formulas parse");
}

/// A teacher writing a set on Windows produces CRLF, and an editor may convert a file either
/// way. Line endings are the file's business, not the question's: the same set must read the
/// same, character for character, however its lines end.
#[test]
fn a_set_reads_the_same_whichever_line_ending_its_file_uses() {
	let lf: &str = EXAMPLE_TEXT;
	let crlf: String = lf.replace("\r\n", "\n").replace('\n', "\r\n");
	assert_ne!(lf.replace("\r\n", "\n"), crlf);

	let straight: QuestionSet = QuestionSet::import(lf).expect("the shipped example loads");
	let windows: QuestionSet = QuestionSet::import(&crlf).expect("its CRLF twin loads");
	assert_eq!(straight.name, windows.name);
	assert_eq!(straight.questions().len(), windows.questions().len());

	// Down to the source a student sees and the literals their answers are compared against.
	for (plain, carried) in straight.exercises().zip(windows.exercises()) {
		assert_eq!(plain.name, carried.name);
		assert_eq!(plain.expression, carried.expression);
		assert_eq!(plain.bindings, carried.bindings);
		assert_eq!(plain.note, carried.note);
	}

	// The CLI reads a file, not a checkout, so it sees whatever the teacher saved.
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: PathBuf = write(&directory, "crlf.toml", &crlf);
	let listed: Output = cli(&["--python", "--set", path.to_str().unwrap(), "--list"]);
	assert!(listed.status.success(), "{}", err(&listed));
	assert_eq!(
		out(&listed),
		out(&cli(&["--python", "--set", EXAMPLE_PATH, "--list"]))
	);
}
