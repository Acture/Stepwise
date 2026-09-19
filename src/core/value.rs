use std::{cmp::Ordering, fmt};

use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{FromPrimitive, One, Signed, ToPrimitive, Zero};

use super::{BinaryOp, CompareOp, UnaryOp};

pub(super) const MAX_INTEGER_BITS: u64 = 4096;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
	Bool(bool),
	Int(BigInt),
	Float(f64),
	None,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EvalError {
	#[error("{name}: {reason}")]
	Python { name: &'static str, reason: String },
	#[error("超出首版支持范围：{0}")]
	Limit(String),
}

impl EvalError {
	pub fn name(&self) -> Option<&'static str> {
		match self {
			Self::Python { name, .. } => Some(name),
			Self::Limit(_) => None,
		}
	}

	fn zero_division() -> Self {
		Self::Python {
			name: "ZeroDivisionError",
			reason: "除数为零；Python 会在这一步停止，不会得到一个数值。".into(),
		}
	}
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
	pub fn type_name(&self) -> &'static str {
		match self {
			Self::Bool(_) => "bool",
			Self::Int(_) => "int",
			Self::Float(_) => "float",
			Self::None => "NoneType",
		}
	}

	pub fn truthy(&self) -> bool {
		match self {
			Self::Bool(value) => *value,
			Self::Int(value) => !value.is_zero(),
			Self::Float(value) => *value != 0.0,
			Self::None => false,
		}
	}

	pub fn same_answer(&self, other: &Self) -> bool {
		match (self, other) {
			(Self::Float(left), Self::Float(right)) => left.to_bits() == right.to_bits(),
			_ => self == other,
		}
	}

	pub(super) fn checked(self) -> Result<Self, EvalError> {
		match &self {
			Self::Int(value) if value.bits() > MAX_INTEGER_BITS => Err(EvalError::Limit(format!(
				"整数最多 {MAX_INTEGER_BITS} 位二进制。"
			))),
			Self::Float(value) if !value.is_finite() => {
				Err(EvalError::Limit("暂不支持非有限浮点数。".into()))
			}
			_ => Ok(self),
		}
	}

	fn integer(&self) -> Option<BigInt> {
		match self {
			Self::Bool(value) => Some(BigInt::from(u8::from(*value))),
			Self::Int(value) => Some(value.clone()),
			_ => None,
		}
	}

	fn float(&self) -> Result<f64, EvalError> {
		match self {
			Self::Float(value) => Ok(*value),
			Self::None => Err(type_error("None 不能参加数值运算。")),
			_ => self
				.integer()
				.and_then(|value| value.to_f64())
				.filter(|value| value.is_finite())
				.ok_or_else(|| EvalError::Python {
					name: "OverflowError",
					reason: "整数太大，无法转换为浮点数。".into(),
				}),
		}
	}

	pub(super) fn unary(&self, op: UnaryOp) -> Result<Self, EvalError> {
		if matches!(op, UnaryOp::Not | UnaryOp::LogicalNot) {
			return Ok(Self::Bool(!self.truthy()));
		}
		if let Some(value) = self.integer() {
			return Self::Int(if op == UnaryOp::Negative {
				-value
			} else {
				value
			})
			.checked();
		}
		let value: f64 = self.float()?;
		Self::Float(if op == UnaryOp::Negative {
			-value
		} else {
			value
		})
		.checked()
	}

	pub(super) fn binary(&self, op: BinaryOp, right: &Self) -> Result<Self, EvalError> {
		if matches!(self, Self::None) || matches!(right, Self::None) {
			return Err(type_error("None 不能参加数值运算。"));
		}
		if let (Some(left), Some(right)) = (self.integer(), right.integer()) {
			return integer_binary(op, left, right)?.checked();
		}
		let left: f64 = self.float()?;
		let right: f64 = right.float()?;
		let value: f64 = match op {
			BinaryOp::Add => left + right,
			BinaryOp::Subtract => left - right,
			BinaryOp::Multiply => left * right,
			BinaryOp::Divide | BinaryOp::FloorDivide | BinaryOp::Modulo if right == 0.0 => {
				return Err(EvalError::zero_division());
			}
			BinaryOp::Divide => left / right,
			BinaryOp::FloorDivide => float_divmod(left, right).0,
			BinaryOp::Modulo => float_divmod(left, right).1,
			BinaryOp::Power => float_power(left, right)?,
		};
		Self::Float(value).checked()
	}

	pub(super) fn compare(&self, op: CompareOp, right: &Self) -> Result<Self, EvalError> {
		let ordering: Option<Ordering> = match (self, right) {
			(Self::None, Self::None) => Some(Ordering::Equal),
			(Self::None, _) | (_, Self::None) => None,
			(Self::Float(left), Self::Float(right)) => left.partial_cmp(right),
			(Self::Float(left), _) => right
				.integer()
				.map(|right| int_float_cmp(&right, *left).reverse()),
			(_, Self::Float(right)) => self.integer().map(|left| int_float_cmp(&left, *right)),
			_ => self
				.integer()
				.zip(right.integer())
				.map(|(left, right)| left.cmp(&right)),
		};
		let answer: bool = match op {
			CompareOp::Equal => ordering == Some(Ordering::Equal),
			CompareOp::NotEqual => ordering != Some(Ordering::Equal),
			_ if matches!(self, Self::None) || matches!(right, Self::None) => {
				return Err(type_error("None 不支持大小比较。"));
			}
			CompareOp::Less => ordering == Some(Ordering::Less),
			CompareOp::LessEqual => matches!(ordering, Some(Ordering::Less | Ordering::Equal)),
			CompareOp::Greater => ordering == Some(Ordering::Greater),
			CompareOp::GreaterEqual => {
				matches!(ordering, Some(Ordering::Greater | Ordering::Equal))
			}
		};
		Ok(Self::Bool(answer))
	}
}

fn type_error(reason: &str) -> EvalError {
	EvalError::Python {
		name: "TypeError",
		reason: reason.into(),
	}
}

fn int_float_cmp(integer: &BigInt, float: f64) -> Ordering {
	// All values entering the evaluator are finite. Compare exactly, without rounding the int.
	let truncated: BigInt = BigInt::from_f64(float).expect("finite float has an integer part");
	match integer.cmp(&truncated) {
		Ordering::Equal if float.fract() > 0.0 => Ordering::Less,
		Ordering::Equal if float.fract() < 0.0 => Ordering::Greater,
		ordering => ordering,
	}
}

fn integer_binary(op: BinaryOp, left: BigInt, right: BigInt) -> Result<Value, EvalError> {
	let value: BigInt = match op {
		BinaryOp::Add => left + right,
		BinaryOp::Subtract => left - right,
		BinaryOp::Multiply => left * right,
		BinaryOp::Divide | BinaryOp::FloorDivide | BinaryOp::Modulo if right.is_zero() => {
			return Err(EvalError::zero_division());
		}
		BinaryOp::Divide => {
			let negative_zero: bool = left.is_zero() && right.is_negative();
			let value: f64 = BigRational::new(left, right)
				.to_f64()
				.filter(|value| value.is_finite())
				.ok_or_else(|| EvalError::Python {
					name: "OverflowError",
					reason: "商太大，无法表示为浮点数。".into(),
				})?;
			return Ok(Value::Float(if negative_zero { -0.0 } else { value }));
		}
		BinaryOp::FloorDivide => left.div_floor(&right),
		BinaryOp::Modulo => left.mod_floor(&right),
		BinaryOp::Power => {
			if right.is_negative() {
				return Value::Float(float_power(
					Value::Int(left).float()?,
					Value::Int(right).float()?,
				)?)
				.checked();
			}
			if left.is_zero() {
				return Ok(Value::Int(if right.is_zero() {
					BigInt::one()
				} else {
					BigInt::zero()
				}));
			}
			if left.abs().is_one() {
				return Ok(Value::Int(if left.is_negative() && right.is_odd() {
					-BigInt::one()
				} else {
					BigInt::one()
				}));
			}
			let exponent: u32 = right
				.to_u32()
				.filter(|value| u64::from(*value) * (left.bits() - 1) < MAX_INTEGER_BITS)
				.ok_or_else(|| EvalError::Limit("乘方结果超过整数位数限制。".into()))?;
			left.pow(exponent)
		}
	};
	Ok(Value::Int(value))
}

fn float_power(left: f64, right: f64) -> Result<f64, EvalError> {
	if left == 0.0 && right < 0.0 {
		return Err(EvalError::zero_division());
	}
	if left < 0.0 && right.fract() != 0.0 {
		return Err(EvalError::Limit(
			"负数的非整数次幂需要复数，暂不支持。".into(),
		));
	}
	let result: f64 = left.powf(right);
	if result.is_infinite() {
		return Err(EvalError::Python {
			name: "OverflowError",
			reason: "乘方结果超出浮点数范围。".into(),
		});
	}
	Ok(result)
}

/// CPython's float divmod correction: floor(left / right) alone is wrong for e.g. 1.0 // 0.1.
fn float_divmod(left: f64, right: f64) -> (f64, f64) {
	let mut modulo: f64 = left % right;
	let mut division: f64 = (left - modulo) / right;
	if modulo != 0.0 {
		if modulo.is_sign_negative() != right.is_sign_negative() {
			modulo += right;
			division -= 1.0;
		}
	} else {
		modulo = 0.0_f64.copysign(right);
	}
	let quotient: f64 = if division != 0.0 {
		let floor: f64 = division.floor();
		if division - floor > 0.5 {
			floor + 1.0
		} else {
			floor
		}
	} else {
		0.0_f64.copysign(left / right)
	};
	(quotient, modulo)
}
