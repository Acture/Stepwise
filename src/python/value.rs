use std::cmp::Ordering;

use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{FromPrimitive, One, Signed, ToPrimitive, Zero};

use super::{BinaryOp, CompareOp, UnaryOp};
use crate::core::{EvalError, Value};

pub(super) const MAX_INTEGER_BITS: u64 = 4096;

/// Reject values this first version cannot teach before they enter the tree.
pub(super) fn checked(value: Value) -> Result<Value, EvalError> {
	match &value {
		Value::Int(number) if number.bits() > MAX_INTEGER_BITS => Err(EvalError::Limit(format!(
			"整数最多 {MAX_INTEGER_BITS} 位二进制。"
		))),
		Value::Float(number) if !number.is_finite() => {
			Err(EvalError::Limit("暂不支持非有限浮点数。".into()))
		}
		_ => Ok(value),
	}
}

/// Python's bool() coercion: zero, empty and None are false.
pub(super) fn truthy(value: &Value) -> bool {
	match value {
		Value::Bool(value) => *value,
		Value::Int(value) => !value.is_zero(),
		Value::Float(value) => *value != 0.0,
		Value::None => false,
	}
}

pub(super) fn unary(value: &Value, op: UnaryOp) -> Result<Value, EvalError> {
	if op == UnaryOp::Not {
		return Ok(Value::Bool(!truthy(value)));
	}
	if let Some(number) = integer(value) {
		return checked(Value::Int(if op == UnaryOp::Negative {
			-number
		} else {
			number
		}));
	}
	let number: f64 = float(value)?;
	checked(Value::Float(if op == UnaryOp::Negative {
		-number
	} else {
		number
	}))
}

pub(super) fn binary(left: &Value, op: BinaryOp, right: &Value) -> Result<Value, EvalError> {
	if matches!(left, Value::None) || matches!(right, Value::None) {
		return Err(type_error("None 不能参加数值运算。"));
	}
	if let (Some(left), Some(right)) = (integer(left), integer(right)) {
		return checked(integer_binary(op, left, right)?);
	}
	let left: f64 = float(left)?;
	let right: f64 = float(right)?;
	let value: f64 = match op {
		BinaryOp::Add => left + right,
		BinaryOp::Subtract => left - right,
		BinaryOp::Multiply => left * right,
		BinaryOp::Divide | BinaryOp::FloorDivide | BinaryOp::Modulo if right == 0.0 => {
			return Err(zero_division());
		}
		BinaryOp::Divide => left / right,
		BinaryOp::FloorDivide => float_divmod(left, right).0,
		BinaryOp::Modulo => float_divmod(left, right).1,
		BinaryOp::Power => float_power(left, right)?,
	};
	checked(Value::Float(value))
}

pub(super) fn compare(left: &Value, op: CompareOp, right: &Value) -> Result<Value, EvalError> {
	let ordering: Option<Ordering> = match (left, right) {
		(Value::None, Value::None) => Some(Ordering::Equal),
		(Value::None, _) | (_, Value::None) => None,
		(Value::Float(left), Value::Float(right)) => left.partial_cmp(right),
		(Value::Float(left), _) => {
			integer(right).map(|right| int_float_cmp(&right, *left).reverse())
		}
		(_, Value::Float(right)) => integer(left).map(|left| int_float_cmp(&left, *right)),
		_ => integer(left)
			.zip(integer(right))
			.map(|(left, right)| left.cmp(&right)),
	};
	let answer: bool = match op {
		CompareOp::Equal => ordering == Some(Ordering::Equal),
		CompareOp::NotEqual => ordering != Some(Ordering::Equal),
		_ if matches!(left, Value::None) || matches!(right, Value::None) => {
			return Err(type_error("None 不支持大小比较。"));
		}
		CompareOp::Less => ordering == Some(Ordering::Less),
		CompareOp::LessEqual => matches!(ordering, Some(Ordering::Less | Ordering::Equal)),
		CompareOp::Greater => ordering == Some(Ordering::Greater),
		CompareOp::GreaterEqual => matches!(ordering, Some(Ordering::Greater | Ordering::Equal)),
	};
	Ok(Value::Bool(answer))
}

fn integer(value: &Value) -> Option<BigInt> {
	match value {
		Value::Bool(value) => Some(BigInt::from(u8::from(*value))),
		Value::Int(value) => Some(value.clone()),
		_ => None,
	}
}

fn float(value: &Value) -> Result<f64, EvalError> {
	match value {
		Value::Float(value) => Ok(*value),
		Value::None => Err(type_error("None 不能参加数值运算。")),
		_ => integer(value)
			.and_then(|value| value.to_f64())
			.filter(|value| value.is_finite())
			.ok_or_else(|| EvalError::Raised {
				name: "OverflowError",
				reason: "整数太大，无法转换为浮点数。".into(),
			}),
	}
}

fn type_error(reason: &str) -> EvalError {
	EvalError::Raised {
		name: "TypeError",
		reason: reason.into(),
	}
}

fn zero_division() -> EvalError {
	EvalError::Raised {
		name: "ZeroDivisionError",
		reason: "除数为零；Python 会在这一步停止，不会得到一个数值。".into(),
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
			return Err(zero_division());
		}
		BinaryOp::Divide => {
			let negative_zero: bool = left.is_zero() && right.is_negative();
			let value: f64 = BigRational::new(left, right)
				.to_f64()
				.filter(|value| value.is_finite())
				.ok_or_else(|| EvalError::Raised {
					name: "OverflowError",
					reason: "商太大，无法表示为浮点数。".into(),
				})?;
			return Ok(Value::Float(if negative_zero { -0.0 } else { value }));
		}
		BinaryOp::FloorDivide => left.div_floor(&right),
		BinaryOp::Modulo => left.mod_floor(&right),
		BinaryOp::Power => {
			if right.is_negative() {
				return checked(Value::Float(float_power(
					float(&Value::Int(left))?,
					float(&Value::Int(right))?,
				)?));
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
		return Err(zero_division());
	}
	if left < 0.0 && right.fract() != 0.0 {
		return Err(EvalError::Limit(
			"负数的非整数次幂需要复数，暂不支持。".into(),
		));
	}
	let result: f64 = left.powf(right);
	if result.is_infinite() {
		return Err(EvalError::Raised {
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
