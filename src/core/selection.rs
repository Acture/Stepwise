use super::eval::own_step;
use super::{Expr, ExprKind, NextStep};

/// Every step a student may submit now. A short-circuiting operation may be applied as soon as
/// its operands decide it, or its remaining operands may be computed anyway; the choice is
/// made again at every such operation and in every state, so a student can compute part of
/// an operand and still short-circuit afterwards. A step is allowed when some choice of short
/// circuit or continue at each of those operations allows it under the precedence rule below.
///
/// Bindings and finished brackets are independent actions. Inside one parenthesis scope an
/// operation may apply when no operation that could apply elsewhere in the scope, outside its
/// own subtree, ranks above it. A short circuit is therefore never held back by work inside
/// the operand it skips.
pub(super) fn allowed_steps(root: &Expr) -> Vec<NextStep> {
	let mut steps: Vec<NextStep> = Vec::new();
	allow(root, None, &mut steps);
	steps
}

/// `outside` is the lowest that the highest ready precedence elsewhere in this scope can be
/// while this subtree stays reachable; `None` when nothing out there has to go first.
fn allow(node: &Expr, outside: Option<u8>, steps: &mut Vec<NextStep>) {
	match &node.kind {
		ExprKind::Value(_) => {}
		ExprKind::Binding { .. } => steps.extend(own_step(node)),
		ExprKind::Group(child) => match own_step(node) {
			Some(step) => steps.push(step),
			None => allow(child, None, steps),
		},
		ExprKind::Operation(op, operands) => {
			let precedence: u8 = op.rules().precedence();
			let applied: Option<NextStep> = own_step(node);
			let decided: bool = applied.is_some();
			if let Some(step) = applied
				&& outside.is_none_or(|highest| highest <= precedence)
			{
				steps.push(step);
			}
			let first: Option<usize> = operands
				.iter()
				.position(|operand| operand.value().is_none());
			for (index, operand) in operands.iter().enumerate() {
				if operand.value().is_some() {
					continue;
				}
				// Short-circuiting here leaves the later operands for after the first one
				// decides, so nothing beside that first one competes with it yet.
				let guarded: bool = op.rules().short_circuits() && !decided && Some(index) == first;
				let beside: Option<u8> = if guarded {
					None
				} else {
					operands
						.iter()
						.enumerate()
						.filter(|(other, _)| *other != index)
						.filter_map(|(_, other)| floor(other))
						.max()
				};
				allow(operand, outside.max(beside), steps);
			}
		}
	}
}

/// The lowest that the highest precedence of an operation ready to apply in this subtree can
/// be, over every choice of short circuit or continue inside it; `None` when nothing in it
/// competes in the enclosing scope. Brackets open a scope of their own.
fn floor(node: &Expr) -> Option<u8> {
	let ExprKind::Operation(op, operands) = &node.kind else {
		return None;
	};
	let precedence: u8 = op.rules().precedence();
	let inside = || -> Option<u8> { operands.iter().filter_map(floor).max() };
	match operands.iter().find(|operand| operand.value().is_none()) {
		None => Some(precedence),
		Some(_) if own_step(node).is_some() => Some(precedence).min(inside()),
		Some(first) if op.rules().short_circuits() => floor(first),
		Some(_) => inside(),
	}
}

/// The steps the language's own short-circuit order allows, in a fixed order a hint and a
/// trace choose from: bindings first, then each scope's highest-precedence ready operations,
/// inner scopes before the scope around them. Always a subset of [`allowed_steps`].
pub(crate) fn reference_steps(root: &Expr) -> Vec<NextStep> {
	let mut independent: Vec<NextStep> = root
		.rows()
		.into_iter()
		.filter(|(_, node)| matches!(node.kind, ExprKind::Binding { .. }))
		.filter_map(|(_, node)| own_step(node))
		.collect();
	reference_scope(root, &mut independent);
	independent
}

/// Parentheses create local precedence scopes, not a global depth ranking.
fn reference_scope(root: &Expr, independent: &mut Vec<NextStep>) {
	let mut operations: Vec<(u8, NextStep)> = Vec::new();
	reference_collect(root, independent, &mut operations);
	let highest: Option<u8> = operations.iter().map(|(priority, _)| *priority).max();
	independent.extend(
		operations
			.into_iter()
			.filter_map(|(priority, step)| (Some(priority) == highest).then_some(step)),
	);
}

fn reference_collect(
	root: &Expr,
	independent: &mut Vec<NextStep>,
	operations: &mut Vec<(u8, NextStep)>,
) {
	if matches!(root.kind, ExprKind::Value(_) | ExprKind::Binding { .. }) {
		return;
	}
	if let Some(step) = own_step(root) {
		match &root.kind {
			ExprKind::Operation(op, _) => operations.push((op.rules().precedence(), step)),
			_ => independent.push(step),
		}
		return;
	}
	match &root.kind {
		ExprKind::Group(child) => reference_scope(child, independent),
		ExprKind::Operation(op, operands) if op.rules().short_circuits() => {
			// Later operands may be skipped; wait until the first unfinished operand decides.
			if let Some(child) = operands.iter().find(|child| child.value().is_none()) {
				reference_collect(child, independent, operations);
			}
		}
		_ => {
			for child in root.children() {
				reference_collect(child, independent, operations);
			}
		}
	}
}
