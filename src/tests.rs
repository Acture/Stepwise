//! Checks that need core's crate-private rules and both languages at once, so they live above
//! `core` rather than inside it: core names a language only through `core/language.rs`.

use std::collections::{BTreeMap, BTreeSet};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::core::{
	Expr, ExprKind, Language, NextStep, NodeId, Session, Value, own_step, reference_steps,
};

/// The definition `Session::allowed_steps` computes in closed form: the per-strategy selection
/// the two old modes used, with the strategy chosen separately at every short-circuiting
/// operation. `short` holds the operations that short-circuit under this choice.
fn under(root: &Expr, short: &BTreeSet<NodeId>) -> BTreeSet<NodeId> {
	fn scope(root: &Expr, short: &BTreeSet<NodeId>, steps: &mut BTreeSet<NodeId>) {
		let mut operations: Vec<(u8, NodeId)> = Vec::new();
		collect(root, short, steps, &mut operations);
		let highest: Option<u8> = operations.iter().map(|(priority, _)| *priority).max();
		steps.extend(
			operations
				.into_iter()
				.filter_map(|(priority, id)| (Some(priority) == highest).then_some(id)),
		);
	}
	fn collect(
		root: &Expr,
		short: &BTreeSet<NodeId>,
		steps: &mut BTreeSet<NodeId>,
		operations: &mut Vec<(u8, NodeId)>,
	) {
		match &root.kind {
			ExprKind::Value(_) | ExprKind::Binding { .. } => {}
			ExprKind::Group(child) if child.value().is_some() => {
				steps.insert(root.id);
			}
			ExprKind::Group(child) => scope(child, short, steps),
			ExprKind::Operation(op, operands) => {
				let shorting: bool = short.contains(&root.id);
				let ready: bool = if shorting {
					own_step(root).is_some()
				} else {
					operands.iter().all(|operand| operand.value().is_some())
				};
				if ready {
					operations.push((op.rules().precedence(), root.id));
				} else if shorting {
					if let Some(first) = operands.iter().find(|operand| operand.value().is_none()) {
						collect(first, short, steps, operations);
					}
				} else {
					for operand in operands {
						collect(operand, short, steps, operations);
					}
				}
			}
		}
	}
	let mut steps: BTreeSet<NodeId> = root
		.rows()
		.into_iter()
		.filter(|(_, node)| matches!(node.kind, ExprKind::Binding { .. }))
		.map(|(_, node)| node.id)
		.collect();
	scope(root, short, &mut steps);
	steps
}

/// Every choice of short circuit or continue, one per short-circuiting operation.
fn union(root: &Expr) -> BTreeSet<NodeId> {
	let guards: Vec<NodeId> = root
		.rows()
		.into_iter()
		.filter(
			|(_, node)| matches!(&node.kind, ExprKind::Operation(op, _) if op.rules().short_circuits()),
		)
		.map(|(_, node)| node.id)
		.collect();
	assert!(guards.len() <= 12, "too many choices to enumerate");
	(0..1_u32 << guards.len())
		.flat_map(|mask: u32| {
			let short: BTreeSet<NodeId> = guards
				.iter()
				.enumerate()
				.filter(|(bit, _)| mask & (1 << bit) != 0)
				.map(|(_, id)| *id)
				.collect();
			under(root, &short)
		})
		.collect()
}

fn ids(steps: &[NextStep]) -> BTreeSet<NodeId> {
	steps.iter().map(|step| step.node_id).collect()
}

/// Walk one session by allowed steps drawn from `rng`, checking the closed form against the
/// enumeration in every state it passes through. Returns the number of states checked.
fn walk(mut session: Session, rng: &mut ChaCha8Rng) -> usize {
	let mut states: usize = 0;
	while !session.is_finished() {
		let allowed: Vec<NextStep> = session.allowed_steps();
		assert_eq!(ids(&allowed), union(session.root()), "{}", session.render());
		assert!(
			ids(&reference_steps(session.root())).is_subset(&ids(&allowed)),
			"{}",
			session.render()
		);
		states += 1;
		let step: &NextStep = &allowed[rng.gen_range(0..allowed.len())];
		let group: bool = matches!(
			session.root().find(step.node_id).map(|node| &node.kind),
			Some(ExprKind::Group(_))
		);
		let accepted: bool = if group {
			session.remove_group(step.node_id).accepted()
		} else {
			let input: String = match &step.outcome {
				Ok(value) => value.to_string(),
				Err(error) => error.name().expect("a raised exception").into(),
			};
			session.submit(step.node_id, &input).accepted()
		};
		assert!(accepted, "{}", session.render());
	}
	states
}

#[test]
fn the_closed_form_is_the_union_over_every_choice_of_short_circuit_or_continue() {
	let mut states: usize = 0;
	for language in [Language::Python, Language::Logic] {
		for seed in 0..96 {
			let session: Session = crate::generate::generate(language, seed)
				.unwrap()
				.session()
				.unwrap();
			for run in 0..4 {
				let mut rng: ChaCha8Rng = ChaCha8Rng::seed_from_u64(seed * 4 + run);
				states += walk(session.clone(), &mut rng);
			}
		}
	}
	let values: BTreeMap<String, Value> = BTreeMap::new();
	for source in [
		"False and (2 + 3 > 1)",
		"False and 2 + 3 > 1",
		"True or (False and 4 > 5)",
		"(False and (4 > 5)) or ((1 + 1) == 2)",
		"1 + 2 == 3 or False and 4 > 5",
		"0 and 1 + 2 or 3 * 4 and not 5 - 5",
		"False and (1 / 0 > 1) or 2 ** 3 ** 2 > 7 and (True or 1 // 0)",
		"(1 > 2) and (2 + 3 > 1) and 4 * 5 == 20 or -(2 - 12) > 3",
	] {
		let session: Session = crate::python::session(source, &values).unwrap();
		for run in 0..32 {
			states += walk(session.clone(), &mut ChaCha8Rng::seed_from_u64(run));
		}
	}
	for source in [
		"F ∧ (T ∨ F) → ¬T ↔ T",
		"(T → F ∧ T) ∨ (F → T) ∧ ¬(F ∨ T)",
		"T ∨ F ∧ T → F ↔ (F → T ∨ F)",
	] {
		let session: Session = crate::logic::session(source, &BTreeMap::new()).unwrap();
		for run in 0..32 {
			states += walk(session.clone(), &mut ChaCha8Rng::seed_from_u64(run));
		}
	}
	assert!(states > 3000, "only {states} states checked");
}
