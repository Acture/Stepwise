use super::{BinaryOp, BoolOp, EvaluationMode, Expr, ExprKind, NextStep, UnaryOp, next_step};

/// Bindings and ready groups are independent actions. Computations at the highest
/// ready precedence can be chosen in either order, without reassociating the tree.
pub(super) fn available_steps(root: &Expr, mode: EvaluationMode) -> Vec<NextStep> {
	let mut independent: Vec<NextStep> = root
		.rows()
		.into_iter()
		.filter(|(_, node)| {
			matches!(
				node.kind,
				ExprKind::Variable(..) | ExprKind::Proposition(..)
			)
		})
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
	if matches!(
		root.kind,
		ExprKind::Value(_) | ExprKind::Variable(..) | ExprKind::Proposition(..)
	) {
		return;
	}
	let Some(step) = next_step(root, mode) else {
		return;
	};
	if step.node_id == root.id {
		if matches!(root.kind, ExprKind::Group(_)) {
			independent.push(step);
		} else {
			operations.push((precedence(root), step));
		}
		return;
	}
	match &root.kind {
		ExprKind::Group(child) => collect_scope(child, mode, independent),
		ExprKind::Bool(_, children) if mode == EvaluationMode::ShortCircuit => {
			// Later operands may be skipped; wait until the first unfinished operand decides.
			if let Some(child) = children.iter().find(|child| child.value().is_none()) {
				collect(child, mode, independent, operations);
			}
		}
		ExprKind::Logic(op, left, right)
			if mode == EvaluationMode::ShortCircuit && *op != crate::logic::LogicOp::Iff =>
		{
			collect(
				if left.value().is_none() { left } else { right },
				mode,
				independent,
				operations,
			);
		}
		_ => {
			for child in root.children() {
				collect(child, mode, independent, operations);
			}
		}
	}
}

fn precedence(node: &Expr) -> u8 {
	match node.kind {
		ExprKind::Binary(BinaryOp::Power, ..) => 80,
		ExprKind::Unary(UnaryOp::Positive | UnaryOp::Negative, _) => 70,
		ExprKind::Binary(
			BinaryOp::Multiply | BinaryOp::Divide | BinaryOp::FloorDivide | BinaryOp::Modulo,
			..,
		) => 60,
		ExprKind::Binary(BinaryOp::Add | BinaryOp::Subtract, ..) => 50,
		ExprKind::Compare(..) => 40,
		ExprKind::Unary(UnaryOp::Not, _) => 30,
		ExprKind::Bool(BoolOp::And, _) => 20,
		ExprKind::Bool(BoolOp::Or, _) => 10,
		ExprKind::Unary(UnaryOp::LogicalNot, _) => 90,
		ExprKind::Logic(op, ..) => match op {
			crate::logic::LogicOp::And => 70,
			crate::logic::LogicOp::Or => 50,
			crate::logic::LogicOp::Implies => 30,
			crate::logic::LogicOp::Iff => 10,
		},
		_ => unreachable!("only computation nodes have precedence"),
	}
}
