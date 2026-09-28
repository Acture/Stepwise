use super::{EvalError, Expr, ExprKind, NodeId, Value};

pub type Outcome = Result<Value, EvalError>;

#[derive(Clone, Debug, PartialEq)]
pub struct NextStep {
	pub node_id: NodeId,
	pub outcome: Outcome,
	pub explanation: String,
}

/// A fixed route through a question, for naming one next step and for bounding random
/// questions. It never decides what a student may submit: every step the rules allow is
/// submittable, whichever route it lies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Path {
	/// The language's own order: an operation applies as soon as its operands decide it, so
	/// an operand it no longer needs is skipped.
	ShortCircuit,
	/// Every operand is reduced before the operation over it applies.
	Complete,
}

/// This node's own rule application, if it has one right now: a binding, a pair of brackets
/// around a value, or an operation its language can apply to the operands as they stand.
/// Bindings and grouping are shared; every operator rule comes from the language that owns
/// the operator.
pub(crate) fn own_step(node: &Expr) -> Option<NextStep> {
	let (outcome, explanation): (Outcome, String) = match &node.kind {
		ExprKind::Value(_) => return None,
		ExprKind::Binding {
			language,
			name,
			value,
		} => (Ok(value.clone()), language.binding_explanation(name, value)),
		ExprKind::Group(child) => (
			Ok(child.value()?.clone()),
			"括号内已经是一个值；去掉这一层分组，值和类型保持不变。".into(),
		),
		ExprKind::Operation(op, operands) => {
			let applied: (Outcome, String) = op.rules().apply(operands)?;
			debug_assert!(
				op.rules().short_circuits()
					|| operands.iter().all(|operand| operand.value().is_some()),
				"only a short-circuiting operation applies before every operand is a value"
			);
			applied
		}
	};
	let explanation: String = match &outcome {
		Ok(_) => explanation,
		Err(error) => error.to_string(),
	};
	Some(NextStep {
		node_id: node.id,
		outcome,
		explanation,
	})
}

/// The next rule application on `path`, found by descending into the first operand still
/// unreduced until a node can reduce on its own.
pub(crate) fn reference_step(root: &Expr, path: Path) -> Option<NextStep> {
	match &root.kind {
		ExprKind::Value(_) => None,
		ExprKind::Binding { .. } => own_step(root),
		ExprKind::Group(child) => reference_step(child, path).or_else(|| own_step(root)),
		ExprKind::Operation(_, operands) => {
			match operands.iter().find(|operand| operand.value().is_none()) {
				None => own_step(root),
				Some(operand) if path == Path::Complete => reference_step(operand, path),
				Some(operand) => own_step(root).or_else(|| reference_step(operand, path)),
			}
		}
	}
}
