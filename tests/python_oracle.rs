use std::{
	collections::BTreeMap,
	io::Write,
	ops::Range,
	process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};
use stepwise::{
	core::{EvalError, Language, NextStep, NodeId, Outcome, Session, Value},
	exercises, generate,
	python::{self, parse_value},
};

#[derive(Clone, Serialize)]
struct Case {
	source: String,
	bindings: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct Observation {
	source: String,
	kind: String,
	r#type: String,
	value: String,
	bits: String,
}

/// Two checks share one CPython process. Along the language's own order every displayed state
/// must evaluate, as a whole, to what CPython gives the original. Every step a student may
/// submit in those states is checked locally instead: the selected span alone must evaluate
/// to that step's own outcome. A step that computes an operand a short circuit would skip is
/// real Python semantics for that span, but the route it opens need not end where CPython
/// ends the original, so no whole-expression claim is made about it.
#[test]
#[ignore = "requires python3; run cargo test --test python_oracle -- --ignored --nocapture"]
fn matches_cpython_values_types_exceptions_and_float_bits() {
	let values: [&str; 15] = [
		"-17",
		"-3",
		"-1",
		"0",
		"1",
		"2",
		"7",
		"True",
		"False",
		"-0.0",
		"0.1",
		"-2.5",
		"3.0",
		"9007199254740993",
		"None",
	];
	let operators: [&str; 14] = [
		"+", "-", "*", "/", "//", "%", "==", "!=", "<", "<=", ">", ">=", "and", "or",
	];
	let mut sources: Vec<String> = Vec::new();
	for left in values {
		for right in values {
			for op in operators {
				sources.push(format!("({left}) {op} ({right})"));
			}
		}
	}
	for left in ["-3", "0", "2", "10", "2.5", "True"] {
		for right in ["-3", "-1", "0", "1", "2", "9"] {
			sources.push(format!("({left}) ** ({right})"));
		}
	}
	for source in [
		"False and (3 / 0 > 1)",
		"True or (1 / 0)",
		"1.0 // 0.1",
		"1.0 % 0.1",
		"-2 ** 2",
		"-10",
		"-0",
		"-0.0",
		"-10.5",
		"--10",
		"--0.0",
		"-(2 * 5)",
		"-(2 - 12)",
		"-True",
		"-None",
		"2 ** 3 ** 2",
		"(2 + 3) * (4 + 5)",
		"(0 or 5) and (2 + 3)",
		"0 / -3",
		"10**100 / 10**100",
		"9007199254740993 == 9007199254740992.0",
		"9007199254740993 > 9007199254740992.0",
		"True and False and (1/0)",
		"False or 0 or 3",
		"not (2 > 3)",
		"None == None",
		"1e308 ** 2",
		"(10**1000) + 0.5",
		// Operands a short circuit skips may still be computed, and some of them raise.
		"False and (1 / 0 > 1)",
		"True or (2 % 0)",
		"0 and (1 // 0)",
		"(1 > 2) and (3 / 0 > 1)",
		"False and 2 + 3 > 1",
		"True or (False and 4 > 5)",
		"(False and (4 > 5)) or ((1 + 1) == 2)",
		"0 or (0 and (1 / 0))",
	] {
		sources.push(source.into());
	}
	let mut originals: Vec<Case> = sources
		.into_iter()
		.map(|source| Case {
			source,
			bindings: BTreeMap::new(),
		})
		.collect();
	let sets: [exercises::QuestionSet; 2] = [
		exercises::builtin().unwrap(),
		exercises::QuestionSet::import(include_str!("../questions/example.toml")).unwrap(),
	];
	for exercise in sets.iter().flat_map(exercises::QuestionSet::exercises) {
		if matches!(exercise.language, Language::Python) && !exercise.bindings.is_empty() {
			originals.push(Case {
				source: exercise.expression.clone(),
				bindings: exercise.bindings.clone(),
			});
		}
	}
	for x in ["-3", "0", "2", "False", "2.5"] {
		for y in ["-2", "0", "3", "True"] {
			for source in [
				"(x) ** 2 + y",
				"x + y * (x - y) ** 2",
				"not(x > y) or (x / (y + 1) > 0)",
				"x ** 2 ** 2 + y ** -2",
			] {
				originals.push(Case {
					source: source.into(),
					bindings: BTreeMap::from([("x".into(), x.into()), ("y".into(), y.into())]),
				});
			}
		}
	}
	for seed in 0..64 {
		let exercise: exercises::Exercise = generate::generate(Language::Python, seed).unwrap();
		originals.push(Case {
			source: exercise.expression,
			bindings: exercise.bindings,
		});
	}
	let original_count: usize = originals.len();
	let mut cases: Vec<Case> = Vec::new();
	let mut expected: Vec<Outcome> = Vec::new();
	let mut spans: Vec<Case> = Vec::new();
	let mut span_outcomes: Vec<Outcome> = Vec::new();
	let mut limited: usize = 0;
	for original in originals {
		let bindings: BTreeMap<String, Value> = original
			.bindings
			.iter()
			.map(|(name, literal)| (name.clone(), parse_value(literal).unwrap()))
			.collect();
		let mut session: Session = python::session(&original.source, &bindings).unwrap();
		let mut states: Vec<String> = vec![original.source.clone()];
		if session.render() != original.source {
			states.push(session.render().into());
		}
		while let Some(step) = session.next_step() {
			let allowed: Vec<NextStep> = session.allowed_steps();
			assert!(
				allowed.contains(&step),
				"{}: the hinted step is not submittable in {}",
				original.source,
				session.render()
			);
			let (text, ranges): (&str, &BTreeMap<NodeId, Range<usize>>) =
				session.render_with_ranges();
			for choice in allowed {
				if matches!(choice.outcome, Err(EvalError::Limit(_))) {
					limited += 1;
					continue;
				}
				spans.push(Case {
					source: text[ranges[&choice.node_id].clone()].trim().into(),
					bindings: original.bindings.clone(),
				});
				span_outcomes.push(choice.outcome);
			}
			let answer: String = match step.outcome {
				Ok(value) => value.to_string(),
				Err(error) => error
					.name()
					.unwrap_or_else(|| {
						panic!("unexpected limitation for {}: {error}", original.source)
					})
					.into(),
			};
			let feedback: stepwise::core::Feedback = session.submit(step.node_id, &answer);
			assert!(
				feedback.accepted(),
				"{}: {}",
				original.source,
				feedback.message
			);
			if session.terminal_error().is_none() {
				states.push(session.render().into());
			}
		}
		let outcome: Outcome = match session.terminal_error() {
			Some(error) => Err(error.clone()),
			None => Ok(session.root().value().expect("fully reduced").clone()),
		};
		for source in states {
			cases.push(Case {
				source,
				bindings: original.bindings.clone(),
			});
			expected.push(outcome.clone());
		}
	}
	let state_count: usize = cases.len();
	let span_count: usize = spans.len();
	cases.extend(spans);
	expected.extend(span_outcomes);
	let mut child: std::process::Child = Command::new("python3")
		.arg(concat!(
			env!("CARGO_MANIFEST_DIR"),
			"/tests/python_oracle.py"
		))
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.expect("python3 must be installed for the explicit oracle test");
	let mut input: std::process::ChildStdin = child.stdin.take().unwrap();
	input
		.write_all(&serde_json::to_vec(&cases).unwrap())
		.unwrap();
	drop(input);
	let output: std::process::Output = child.wait_with_output().unwrap();
	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
	let observations: Vec<Observation> = serde_json::from_slice(&output.stdout).unwrap();
	assert_eq!(observations.len(), cases.len());
	for (observation, outcome) in observations.into_iter().zip(expected) {
		if observation.kind == "error" {
			assert_eq!(
				outcome.as_ref().err().and_then(|error| error.name()),
				Some(observation.r#type.as_str()),
				"{}",
				observation.source
			);
		} else {
			let value: &Value = outcome
				.as_ref()
				.unwrap_or_else(|error| panic!("{}: {error}", observation.source));
			assert_eq!(
				value.type_name(),
				observation.r#type,
				"{}",
				observation.source
			);
			assert_eq!(
				value,
				&parse_value(&observation.value).unwrap(),
				"{}",
				observation.source
			);
			if let Value::Float(value) = value {
				assert_eq!(
					format!("{:016x}", value.to_bits()),
					observation.bits,
					"{}",
					observation.source
				);
			}
		}
	}
	eprintln!(
		"CPython oracle: {original_count} original expressions and {state_count} displayed states matched as a whole; {span_count} submittable steps matched their selected span alone; {limited} steps beyond this program's limits skipped (types, values, exceptions, float bits)."
	);
}
