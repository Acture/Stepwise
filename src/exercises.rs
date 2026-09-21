use crate::core::{EvaluationMode, Language, ParseError, Session, Value};
use crate::logic::{self, parse_truth};
use crate::python::{self, parse_value};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exercise {
	pub id: String,
	pub title: String,
	pub expression: String,
	pub goal: String,
	#[serde(default)]
	pub language: Language,
	#[serde(default)]
	pub bindings: BTreeMap<String, String>,
}

impl Exercise {
	/// Bindings are stored as source literals; each language reads its own.
	pub fn session(&self, mode: EvaluationMode) -> Result<Session, ParseError> {
		match self.language {
			Language::Python => {
				let bindings: BTreeMap<String, Value> = self
					.bindings
					.iter()
					.map(|(name, literal)| Ok((name.clone(), parse_value(literal)?)))
					.collect::<Result<_, ParseError>>()?;
				python::session(&self.expression, &bindings, mode)
			}
			Language::Logic => {
				let bindings: BTreeMap<String, bool> = self
					.bindings
					.iter()
					.map(|(name, literal)| Ok((name.clone(), parse_truth(literal)?)))
					.collect::<Result<_, ParseError>>()?;
				logic::session(&self.expression, &bindings, mode)
			}
		}
	}
	pub fn assignments(&self) -> String {
		self.bindings
			.iter()
			.map(|(name, value)| format!("{name}={value}"))
			.collect::<Vec<_>>()
			.join(" ")
	}
	pub fn prompt(&self) -> String {
		if self.bindings.is_empty() {
			self.goal.clone()
		} else {
			format!("赋值：{}。{}", self.assignments(), self.goal)
		}
	}
}

pub fn builtin() -> Result<Vec<Exercise>, serde_json::Error> {
	serde_json::from_str(include_str!("../assets/exercises.json"))
}
