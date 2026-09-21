use super::{EvaluationMode, Expr, ExprKind, NextStep, next_step};

/// Bindings and ready groups are independent actions. Computations at the highest
/// ready precedence can be chosen in either order, without reassociating the tree.
pub(super) fn available_steps(root: &Expr, mode: EvaluationMode) -> Vec<NextStep> {
	let mut independent: Vec<NextStep> = root
		.rows()
		.into_iter()
		.filter(|(_, node)| matches!(node.kind, ExprKind::Binding { .. }))
		.filter_map(|(_, node)| next_step(node, mode))
		.collect();
	collect_scope(root, mode, &mut independent);
	independent
}

/// Parentheses create local precedence scopes, not a global depth ranking.
fn collect_scope(root: &Expr, mode: EvaluationMode, independent: &mut Vec<NextStep>) {
	let mut operations: Vec<(u8, NextStep)> = Vec::new();
	collect(root, mode, independent, &mut operations);
	let highest: Option<u8> = operations.iter().map(|(priority, _)| *priority).max();
	independent.extend(
		operations
			.into_iter()
			.filter_map(|(priority, step)| (Some(priority) == highest).then_some(step)),
	);
}

fn collect(
	root: &Expr,
	mode: EvaluationMode,
	independent: &mut Vec<NextStep>,
	operations: &mut Vec<(u8, NextStep)>,
) {
	if matches!(root.kind, ExprKind::Value(_) | ExprKind::Binding { .. }) {
		return;
	}
	let Some(step) = next_step(root, mode) else {
		return;
	};
	if step.node_id == root.id {
		match &root.kind {
			ExprKind::Operation(op, _) => operations.push((op.rules().precedence(), step)),
			_ => independent.push(step),
		}
		return;
	}
	match &root.kind {
		ExprKind::Group(child) => collect_scope(child, mode, independent),
		ExprKind::Operation(op, operands) if op.rules().skips_operands(mode) => {
			// Later operands may be skipped; wait until the first unfinished operand decides.
			if let Some(child) = operands.iter().find(|child| child.value().is_none()) {
				collect(child, mode, independent, operations);
			}
		}
		_ => {
			for child in root.children() {
				collect(child, mode, independent, operations);
			}
		}
	}
}
