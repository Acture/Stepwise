//! The built-in proofs are data: three questions of the embedded set that give premises and a
//! conclusion and nothing else. They reach the program through the same load path, the same
//! name lookup and the same checker as a proof in an imported set, and the progress saved for
//! them before they moved still replays.
//!
//! A complete proof lives only in tests — the checker fixtures under `tests/fixtures` and
//! test code such as this file — never as a sample, in documentation or in anything the
//! program shows.

use std::{
	collections::BTreeSet,
	path::Path,
	process::{Command, Output},
};

use stepwise::{
	app::ProofPractice,
	core::Language,
	exercises::{self, ProofQuestion, Question, QuestionSet},
	progress::Progress,
};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
const EXAMPLE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/questions/example.toml");

/// Each built-in proof, the source strings it was built from while it lived in `main.rs`, and
/// the progress key those strings produced at 17c6f55. The key is the only thing tying saved
/// work to a question, so it is pinned literally rather than recomputed.
const BUILT_IN: [(&str, &[&str], &str, &str); 3] = [
	(
		"mp",
		&["P -> Q", "P"],
		"Q",
		"proof\n[Binary(Implies, Atom(\"P\"), Atom(\"Q\")), Atom(\"P\")]\nQ",
	),
	("raa", &["~~P"], "P", "proof\n[Not(Not(Atom(\"P\")))]\nP"),
	("identity", &[], "P -> P", "proof\n[]\n(P → P)"),
];

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

fn contains(haystack: &str, needle: &str) {
	assert!(
		haystack.contains(needle),
		"{needle:?} missing from {haystack}"
	);
}

fn fixture(name: &str) -> String {
	format!("{FIXTURES}/{name}")
}

fn built_in(name: &str) -> ProofQuestion {
	match exercises::builtin().unwrap().find(name) {
		Some(Question::Proof(question)) => question.clone(),
		other => panic!("{name} must be a proof question of the embedded set, got {other:?}"),
	}
}

#[test]
fn the_built_in_proofs_are_questions_of_the_embedded_set_with_their_sources_verbatim() {
	let set: QuestionSet = exercises::builtin().unwrap();
	for (name, premises, conclusion, _) in BUILT_IN {
		let question: ProofQuestion = built_in(name);
		assert_eq!(question.premises, premises, "{name}");
		assert_eq!(question.conclusion, conclusion, "{name}");
		assert_eq!(set.find(name).unwrap().language(), Language::Logic);
		assert!(!question.title.trim().is_empty());
	}
	// Premises and a conclusion, and no step of the derivation: the proof opens unfinished
	// with only its premises as lines.
	for (name, premises, _, _) in BUILT_IN {
		let proof: stepwise::logic::proof::Proof = built_in(name).proof().unwrap();
		assert!(!proof.is_finished(), "{name}");
		assert_eq!(proof.lines().len(), premises.len(), "{name}");
	}
}

#[test]
fn the_built_in_proofs_keep_the_progress_keys_their_saved_work_is_filed_under() {
	for (name, _, _, key) in BUILT_IN {
		assert_eq!(
			built_in(name).proof().unwrap().progress_key(),
			key,
			"{name}"
		);
	}
}

/// `progress-17c6f55.json` was written by the base commit's own `ProofPractice::record` and
/// `Progress::save`, with the proofs still built in `main.rs`: modus ponens and the identity
/// finished, reductio two lines in. The questions now come from the embedded set, and the
/// same file reopens all three exactly where they were left.
#[test]
fn progress_saved_before_the_proofs_became_data_still_replays() {
	let saved: Progress = Progress::load(Path::new(&fixture("progress-17c6f55.json"))).unwrap();
	assert_eq!(saved.proofs.len(), 3);
	for (name, finished, lines) in [("mp", true, 3), ("raa", false, 3), ("identity", true, 3)] {
		let practice: ProofPractice = ProofPractice::new(built_in(name), saved.clone()).unwrap();
		assert_eq!(practice.is_finished(), finished, "{name}");
		assert_eq!(practice.proof().lines().len(), lines, "{name}");
	}

	// The unfinished one continues from its third line.
	let mut raa: ProofPractice = ProofPractice::new(built_in("raa"), saved.clone()).unwrap();
	raa.paste("P ; raa ; 2,3");
	assert!(raa.submit());
	assert!(raa.is_finished());
}

#[test]
fn listing_logic_shows_the_built_in_proofs_by_name_and_sequent() {
	let logic: Output = cli(&["--logic", "--list"]);
	assert!(logic.status.success(), "{}", err(&logic));
	let listed: String = out(&logic);
	for line in [
		"mp\t肯定前件\tP -> Q，P ⊢ Q\t",
		"raa\t反证法\t~~P ⊢ P\t",
		"identity\t蕴涵引入\t⊢ P -> P\t",
	] {
		assert!(
			listed.lines().any(|listed| listed == line),
			"{line:?} missing from {listed}"
		);
	}

	let python: String = out(&cli(&["--python", "--list"]));
	for (name, _, _, _) in BUILT_IN {
		assert!(
			!python
				.lines()
				.any(|line| line.starts_with(&format!("{name}\t")))
		);
	}
}

/// `--exercise` and `--proof` resolve through the same lookup and open the natural-deduction
/// practice. Without a terminal it refuses before drawing — which is how the test can tell it
/// reached that practice and not an evaluation question.
#[test]
fn naming_a_built_in_proof_opens_the_proof_practice_either_way() {
	for args in [
		["--logic", "--exercise", "raa", "--no-save"],
		["--logic", "--proof", "raa", "--no-save"],
	] {
		let opened: Output = cli(&args);
		assert!(!opened.status.success());
		contains(&err(&opened), "自然演绎练习需要交互终端");
	}
}

/// `--proof NAME` and `--exercise NAME` are one lookup, and `--proof` only insists the
/// question is a proof, so a proof named either way takes the course strategy, refuses a
/// trace with the same words and refuses an assignment.
#[test]
fn a_proof_named_either_way_takes_the_course_strategy_and_refuses_a_trace_or_an_assignment() {
	for flag in ["--exercise", "--proof"] {
		let strategy: Output = cli(&["--logic", flag, "raa", "--evaluation", "eager", "--no-save"]);
		assert_eq!(
			strategy.status.code(),
			Some(1),
			"{flag}: {}",
			err(&strategy)
		);
		contains(&err(&strategy), "自然演绎练习需要交互终端");
		assert!(!err(&strategy).contains("是证明题"), "{}", err(&strategy));

		let traced: Output = cli(&["--logic", flag, "raa", "--trace"]);
		assert_eq!(traced.status.code(), Some(1), "{flag}: {}", err(&traced));
		assert_eq!(
			err(&traced),
			"Stepwise: 证明题没有逐步演示；写好的证明用 --logic --proof raa --check-proof FILE 检查。\n"
		);
		assert!(out(&traced).is_empty());

		let assigned: Output = cli(&["--logic", flag, "raa", "--assign", "P=True", "--no-save"]);
		assert!(!assigned.status.success(), "{flag}");
		contains(&err(&assigned), "--assign");
		assert!(out(&assigned).is_empty());
	}
	// Reached through --exercise, the refusal is the program's own sentence.
	contains(
		&err(&cli(&[
			"--logic",
			"--exercise",
			"raa",
			"--assign",
			"P=True",
			"--no-save",
		])),
		"题目 raa 是证明题，--assign 对它没有意义。",
	);
}

#[test]
fn a_proof_name_is_looked_up_in_the_loaded_set_and_refused_with_the_reason() {
	for (args, reason) in [
		(
			vec!["--logic", "--proof", "no-such-proof"],
			"题集 builtin 里没有题目 no-such-proof",
		),
		(
			vec!["--logic", "--proof", "logic-and"],
			"题目 logic-and 是求值题，不是证明题；用 --exercise logic-and 打开它。",
		),
		(
			vec!["--logic", "--proof", "precedence"],
			"题目 precedence 是 --python 的题目，当前是 --logic。",
		),
		// An imported set replaces the embedded one, built-in proofs included.
		(
			vec!["--logic", "--set", EXAMPLE_PATH, "--proof", "mp"],
			"题集 example-set 里没有题目 mp",
		),
	] {
		let refused: Output = cli(&args);
		assert_eq!(refused.status.code(), Some(1), "{args:?}");
		contains(&err(&refused), reason);
		assert!(out(&refused).is_empty());
	}
}

/// The checker's output is its interface: the same bytes as when the proofs were built into
/// `main.rs`, for a built-in proof, for an imported set's proof and for a sequent written out.
#[test]
fn check_proof_prints_every_line_and_its_explanation_for_any_proof_source() {
	let raa: Output = cli(&[
		"--logic",
		"--proof",
		"raa",
		"--check-proof",
		&fixture("raa.json"),
	]);
	assert!(raa.status.success(), "{}", err(&raa));
	assert_eq!(
		out(&raa),
		"~P ; assume\n\
		 正确。已打开一个假设；子证明中的结论不能直接带到假设之外。\n\
		 False ; not-elim ; 1,2\n\
		 正确。一个命题与它的否定同时成立，得到矛盾 ⊥。\n\
		 P ; raa ; 2,3\n\
		 正确。反证法：假设 ¬A 后推出矛盾，解除该假设，得到 A（经典逻辑）。 目标已在所有假设之外成立，证明完成。\n"
	);
	assert!(err(&raa).is_empty());

	// A sequent written out with --goal belongs to no set and needs no --proof beside it;
	// the same premises and conclusion check the same file the same way.
	let named: Output = cli(&[
		"--logic",
		"--proof",
		"mp",
		"--check-proof",
		&fixture("mp.json"),
	]);
	let written: Output = cli(&[
		"--logic",
		"--goal",
		"Q",
		"--premise",
		"P -> Q",
		"--premise",
		"P",
		"--check-proof",
		&fixture("mp.json"),
	]);
	assert!(named.status.success() && written.status.success());
	assert_eq!(named.stdout, written.stdout);
	contains(&out(&named), "目标已在所有假设之外成立，证明完成。");

	// An imported set's proof question is checked against the file just the same.
	let chain: Output = cli(&[
		"--logic",
		"--set",
		EXAMPLE_PATH,
		"--proof",
		"chain",
		"--check-proof",
		&fixture("chain.json"),
	]);
	assert!(chain.status.success(), "{}", err(&chain));
	contains(&out(&chain), "R ; mp ; 2,4");

	// Legal steps that stop short of the conclusion are not a proof.
	let short: Output = cli(&[
		"--logic",
		"--goal",
		"R",
		"--premise",
		"P -> Q",
		"--premise",
		"P",
		"--check-proof",
		&fixture("mp.json"),
	]);
	assert_eq!(short.status.code(), Some(1));
	contains(&out(&short), "Q ; mp ; 1,2");
	contains(&err(&short), "步骤合法，但尚未在所有假设之外得到目标。");
}

#[test]
fn the_argument_parser_keeps_each_proof_source_to_itself() {
	for args in [
		// One proof at a time: a named question or a sequent written out, not both.
		vec!["--logic", "--proof", "mp", "--goal", "Q"],
		vec!["--logic", "--exercise", "mp", "--goal", "Q"],
		// The checker needs a proof to check against.
		vec!["--logic", "--check-proof", "proof.json"],
		vec!["--logic", "--exercise", "mp", "--check-proof", "proof.json"],
		// A written-out sequent is its own source, so a set does not combine with it.
		vec!["--logic", "--set", EXAMPLE_PATH, "--goal", "P"],
		vec!["--logic", "--random", "--goal", "P"],
		// Natural deduction is propositional.
		vec!["--python", "--goal", "P"],
		// Premises belong to a written-out sequent.
		vec!["--logic", "--proof", "mp", "--premise", "P"],
		// A seed only means something to --random; clap would otherwise waive that
		// requirement beside any question source and drop the seed.
		vec!["--logic", "--goal", "P", "--seed", "3"],
		vec!["--logic", "--proof", "raa", "--seed", "3"],
		vec!["--logic", "--exercise", "raa", "--seed", "3"],
		vec!["--logic", "--list", "--seed", "3"],
		vec!["--logic", "P", "--seed", "3"],
		// An equivalence check needs the expression a named proof would replace.
		vec!["--logic", "--equivalent", "Q", "--proof", "raa"],
	] {
		let refused: Output = cli(&args);
		assert_eq!(
			refused.status.code(),
			Some(2),
			"{args:?}: {}",
			err(&refused)
		);
	}
}

/// How to write a proof line is documented, not demonstrated by a worked solution: the README
/// and the in-app rule text both give the line format and name the same rules. Each rule is
/// applied here once, and the rules named at the head of an in-app rule line and in the first
/// column of the README table must be exactly these — so a rule the checker stops accepting,
/// one documented that it never accepted, or a documented rule whose line or row goes
/// missing fails this test. The checker's own match is not visible from here: a rule added
/// there has to be added to `applied`, which then holds both documents to it.
#[test]
fn every_rule_the_checker_accepts_is_documented_in_the_readme_and_the_in_app_rules() {
	const README: &str = include_str!("../README.md");
	let manual: &str = README
		.split("## 自然演绎")
		.nth(1)
		.and_then(|rest| rest.split("\n## ").next())
		.expect("README has a natural-deduction section");
	let rules: &str = stepwise::logic::proof::RULES;
	for text in [manual, rules] {
		contains(text, "公式 ; 规则 ; 引用行");
	}

	let applied: [(&str, &[&str], &[&str]); 16] = [
		("assume", &[], &["P ; assume"]),
		("mp", &["P -> Q", "P"], &["Q ; mp ; 1,2"]),
		("copy", &["P"], &["P ; copy ; 1"]),
		("and-intro", &["P", "Q"], &["P & Q ; and-intro ; 1,2"]),
		("and-left", &["P & Q"], &["P ; and-left ; 1"]),
		("and-right", &["P & Q"], &["Q ; and-right ; 1"]),
		("or-left", &["P"], &["P | Q ; or-left ; 1"]),
		("or-right", &["Q"], &["P | Q ; or-right ; 1"]),
		("not-elim", &["P", "~P"], &["False ; not-elim ; 1,2"]),
		("bottom-elim", &["False"], &["Q ; bottom-elim ; 1"]),
		(
			"imp-intro",
			&["Q"],
			&["P ; assume", "Q ; copy ; 1", "P -> Q ; imp-intro ; 2,3"],
		),
		(
			"not-intro",
			&["P -> Q", "~Q"],
			&[
				"P ; assume",
				"Q ; mp ; 1,3",
				"False ; not-elim ; 2,4",
				"~P ; not-intro ; 3,5",
			],
		),
		(
			"raa",
			&["~P -> Q", "~Q"],
			&[
				"~P ; assume",
				"Q ; mp ; 1,3",
				"False ; not-elim ; 2,4",
				"P ; raa ; 3,5",
			],
		),
		(
			"iff-intro",
			&["P -> Q", "Q -> P"],
			&["P <-> Q ; iff-intro ; 1,2"],
		),
		("iff-left", &["P <-> Q"], &["P -> Q ; iff-left ; 1"]),
		("iff-right", &["P <-> Q"], &["Q -> P ; iff-right ; 1"]),
	];
	for (rule, premises, lines) in applied {
		let mut proof: stepwise::logic::proof::Proof = ProofQuestion {
			set: String::new(),
			name: rule.into(),
			title: rule.into(),
			premises: premises.iter().map(|premise| (*premise).into()).collect(),
			conclusion: "R".into(),
			note: None,
		}
		.proof()
		.unwrap();
		for line in lines {
			proof
				.submit(line)
				.unwrap_or_else(|error| panic!("{rule}: {line}: {error}"));
		}
		assert_eq!(proof.lines().last().unwrap().rule, rule);
	}

	let accepted: BTreeSet<&str> = applied.iter().map(|(rule, _, _)| *rule).collect();
	// An in-app line starts with its rule, or with two rules joined by " / ".
	let in_app: BTreeSet<&str> = rules
		.lines()
		.flat_map(|line| line.split(" / "))
		.filter_map(|part| part.split_whitespace().next())
		.filter(|head| {
			head.chars()
				.all(|character| character.is_ascii_lowercase() || character == '-')
		})
		.collect();
	assert_eq!(in_app, accepted, "the in-app rule lines");
	// A README table row names its rules in backticks in the first column.
	let in_manual: BTreeSet<&str> = manual
		.lines()
		.filter_map(|row| row.strip_prefix('|'))
		.filter_map(|row| row.split('|').next())
		.flat_map(|cell| cell.split('`').skip(1).step_by(2))
		.collect();
	assert_eq!(in_manual, accepted, "the README rule table");
}

/// The checker explains the line format with an example of its own, in the refusal of a
/// malformed line and in the in-app rules. Neither example may be a step of a built-in proof,
/// or asking how to write a line would hand over an answer.
#[test]
fn the_format_examples_the_checker_shows_answer_no_built_in_proof() {
	let in_rules: &str = stepwise::logic::proof::RULES
		.lines()
		.find_map(|line| line.split_once("写 ").map(|(_, example)| example.trim()))
		.expect("the in-app rules show an example line");
	for (name, _, _, _) in BUILT_IN {
		let mut proof: stepwise::logic::proof::Proof = built_in(name).proof().unwrap();
		let refusal: String = proof.submit("Q").unwrap_err().to_string();
		let in_refusal: &str = refusal
			.split_once("例如")
			.map(|(_, example)| example.trim())
			.expect("the format refusal shows an example line");
		for example in [in_refusal, in_rules] {
			assert!(
				proof.submit(example).is_err(),
				"{name} accepts the format example {example:?}"
			);
			assert!(proof.commands().is_empty());
		}
	}
}
