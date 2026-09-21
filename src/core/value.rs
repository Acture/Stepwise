use std::fmt;

use num_bigint::BigInt;

/// The value a teaching step produces. Both languages answer with one of these;
/// only Python builds the numeric ones, so its arithmetic lives in `crate::python`.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
	Bool(bool),
	Int(BigInt),
	Float(f64),
	None,
}

impl fmt::Display for Value {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Bool(true) => write!(f, "True"),
			Self::Bool(false) => write!(f, "False"),
			Self::Int(value) => write!(f, "{value}"),
			Self::Float(value) => write!(f, "{value:?}"),
			Self::None => write!(f, "None"),
		}
	}
}

impl Value {
	/// Answers are checked by type as well as by value, in both languages.
	pub fn type_name(&self) -> &'static str {
		match self {
			Self::Bool(_) => "bool",
			Self::Int(_) => "int",
			Self::Float(_) => "float",
			Self::None => "NoneType",
		}
	}

	/// Equal answers, distinguishing signed zeros a student can actually type.
	pub fn same_answer(&self, other: &Self) -> bool {
		match (self, other) {
			(Self::Float(left), Self::Float(right)) => left.to_bits() == right.to_bits(),
			_ => self == other,
		}
	}
}
