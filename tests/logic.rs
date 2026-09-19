use std::collections::BTreeMap;

use stepwise::{
	core::{EvaluationMode, FeedbackKind, Session, Value},
	logic::{Formula, LogicOp, parse_formula, proof::Proof},
};

fn formula(source: &str) -> Formula {
	parse_formula(source).unwrap()
}
fn proof(premises: &[&str], goal: &str) -> Proof {
	Proof::new(
		premises.iter().map(|source| formula(source)).collect(),
		formula(goal),
	)
}

#[test]
fn ascii_unicode_precedence_and_equivalence() {
	for alias in ["¬", "~", "!", "not "] {
		assert_eq!(formula(&format!("{alias}P")), formula("~P"));
	}
	for (aliases, canonical) in [
		(vec!["∧", "&", "&&", "and"], "P & Q"),
		(vec!["∨", "|", "||", "or"], "P | Q"),
		(vec!["→", "⇒", "⊃", "->", "=>"], "P -> Q"),
		(vec!["↔", "⇔", "≡", "<->", "<=>"], "P <-> Q"),
	] {
		for alias in aliases {
			assert_eq!(formula(&format!("P {alias} Q")), formula(canonical));
		}
	}
	assert_eq!(formula("¬P ∧ Q → R ↔ S"), formula("((~P & Q) -> R) <-> S"));
	assert_eq!(formula("P -> Q -> R"), formula("P -> (Q -> R)"));
	assert!(formula("P -> Q").equivalent(&formula("~P | Q")).unwrap());
	assert!(formula("~(P & Q)").equivalent(&formula("~P | ~Q")).unwrap());
	assert!(!formula("P -> Q").equivalent(&formula("Q -> P")).unwrap());
	for invalid in ["P Q", "P ->", "∀x P", "P)", "(P", "P + Q"] {
		assert!(parse_formula(invalid).is_err());
	}
}

#[test]
fn truth_tables_and_both_evaluation_strategies() {
	for op in [LogicOp::And, LogicOp::Or, LogicOp::Implies, LogicOp::Iff] {
		for left in [false, true] {
			for right in [false, true] {
				let source: String = format!("P {} Q", op.symbol());
				let expected: bool = match op {
					LogicOp::And => left && right,
					LogicOp::Or => left || right,
					LogicOp::Implies => !left || right,
					LogicOp::Iff => left == right,
				};
				for mode in [EvaluationMode::ShortCircuit, EvaluationMode::Eager] {
					let mut session: Session = Session::logic(
						&source,
						&BTreeMap::from([("P".into(), left), ("Q".into(), right)]),
						mode,
					)
					.unwrap();
					while let Some(step) = session.next_step() {
						assert!(
							session
								.submit(step.node_id, &step.outcome.unwrap().to_string())
								.accepted()
						);
					}
					assert_eq!(
						session.root().value(),
						Some(&Value::Bool(expected)),
						"{source}, {left}, {right}, {mode:?}"
					);
				}
			}
		}
	}
}

#[test]
fn logic_requires_binding_and_rejects_numeric_answers() {
	assert!(Session::logic("P", &BTreeMap::new(), EvaluationMode::Eager).is_err());
	let mut session: Session = Session::logic(
		"P",
		&BTreeMap::from([("P".into(), true)]),
		EvaluationMode::Eager,
	)
	.unwrap();
	assert_eq!(session.submit(0, "1").kind, FeedbackKind::InvalidInput);
	assert!(session.submit(0, "真").accepted());
}

#[test]
fn modus_ponens_does_not_accept_affirming_consequent() {
	let mut valid: Proof = proof(&["P -> Q", "P"], "Q");
	assert!(valid.submit("Q ; mp ; 1,2").is_ok());
	assert!(valid.is_finished());
	let mut invalid: Proof = proof(&["P -> Q", "Q"], "P");
	assert!(invalid.submit("P ; mp ; 1,2").is_err());
	assert_eq!(invalid.lines().len(), 2);
	assert!(invalid.submit("P ; mp ; 1,3").is_err());
}

#[test]
fn reductio_requires_a_scoped_contradiction_and_discharges_it() {
	let mut practice: Proof = proof(&["~~P"], "P");
	assert!(practice.submit("~P ; assume").is_ok());
	assert!(practice.submit("P ; raa ; 2,2").is_err());
	assert!(practice.submit("False ; not-elim ; 1,2").is_ok());
	assert!(practice.submit("P ; raa ; 2,3").is_ok());
	assert!(practice.is_finished());
	assert!(practice.open_assumptions().is_empty());
	assert!(practice.undo());
	assert_eq!(practice.open_assumptions(), &[2]);
	assert!(!practice.is_finished());
}

#[test]
fn rejects_scope_leaks_and_cross_level_discharge() {
	let mut practice: Proof = proof(&[], "Q");
	practice.submit("P ; assume").unwrap();
	practice.submit("P -> P ; imp-intro ; 1,1").unwrap();
	assert!(
		practice
			.submit("P ; copy ; 1")
			.unwrap_err()
			.to_string()
			.contains("已关闭")
	);
	practice.submit("Q ; assume").unwrap();
	assert!(
		!practice.is_finished(),
		"goal inside an assumption is not a proof"
	);
	practice.submit("R ; assume").unwrap();
	assert!(practice.submit("Q -> R ; imp-intro ; 3,4").is_err());
	practice.submit("R -> R ; imp-intro ; 4,4").unwrap();
	assert!(practice.submit("R ; copy ; 4").is_err());
	assert!(practice.submit("Q ; copy ; 3").is_ok());
}

#[test]
fn introduction_elimination_and_replay() {
	let mut practice: Proof = proof(&["P", "Q"], "Q & P");
	practice.submit("P & Q ; and-intro ; 1,2").unwrap();
	practice.submit("Q ; and-right ; 3").unwrap();
	practice.submit("Q & P ; and-intro ; 4,1").unwrap();
	assert!(practice.is_finished());
	let resumed: Proof = proof(&["P", "Q"], "Q & P")
		.replay(practice.commands())
		.unwrap();
	assert!(resumed.is_finished());
	let mut implication: Proof = proof(&[], "P -> P");
	implication.submit("P ; assume").unwrap();
	implication.submit("P -> P ; imp-intro ; 1,1").unwrap();
	assert!(implication.is_finished());
}

#[test]
fn semantic_equivalence_does_not_authorize_an_arbitrary_rule() {
	let mut practice: Proof = proof(&["P -> Q"], "~P | Q");
	assert!(formula("P -> Q").equivalent(&formula("~P | Q")).unwrap());
	assert!(practice.submit("~P | Q ; copy ; 1").is_err());
}

#[test]
fn remaining_rules_respect_their_formula_shapes() {
	for (premises, goal, commands) in [
		(
			vec!["P & Q"],
			"P | R",
			vec!["P ; and-left ; 1", "P | R ; or-left ; 2"],
		),
		(vec!["Q"], "P | Q", vec!["P | Q ; or-right ; 1"]),
		(vec!["False"], "P", vec!["P ; bottom-elim ; 1"]),
		(
			vec!["P <-> Q"],
			"Q -> P",
			vec!["P -> Q ; iff-left ; 1", "Q -> P ; iff-right ; 1"],
		),
		(
			vec!["P -> Q", "Q -> P"],
			"P <-> Q",
			vec!["P <-> Q ; iff-intro ; 1,2"],
		),
		(
			vec!["~P"],
			"~P | R",
			vec![
				"P ; assume",
				"False ; not-elim ; 1,2",
				"~P ; not-intro ; 2,3",
				"~P | R ; or-left ; 4",
			],
		),
	] {
		let mut practice: Proof = proof(&premises, goal);
		for command in commands {
			practice
				.submit(command)
				.unwrap_or_else(|error| panic!("{command}: {error}"));
		}
		assert!(practice.is_finished(), "{goal}");
	}
	let mut practice: Proof = proof(&["P"], "Q | R");
	assert!(practice.submit("Q | R ; or-left ; 1").is_err());
	assert!(practice.submit("Q ; bottom-elim ; 1").is_err());
}
