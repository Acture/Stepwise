/// Reported by either language's parser and by progress replay.
#[derive(Clone, Debug, thiserror::Error)]
#[error("{0}")]
pub struct ParseError(pub String);

/// A step that produces no ordinary value. Languages raise these; core only reports them.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EvalError {
	#[error("{name}: {reason}")]
	Raised { name: &'static str, reason: String },
	#[error("超出首版支持范围：{0}")]
	Limit(String),
}

impl EvalError {
	/// The answer a student types for a raised error; `Limit` is not one of them.
	pub fn name(&self) -> Option<&'static str> {
		match self {
			Self::Raised { name, .. } => Some(name),
			Self::Limit(_) => None,
		}
	}
}
