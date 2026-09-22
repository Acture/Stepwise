//! The question model and the one file format behind it.
//!
//! A question set is a versioned TOML file a teacher can edit, distribute and import. The
//! set compiled into the binary is the default value of that one load path, not a second
//! entry with rules of its own: [`builtin`] hands [`QuestionSet::parse`] the embedded text,
//! exactly as a front end hands it a file it read.
//!
//! A set carries questions, their conditions and its own metadata. It never carries a worked
//! solution: which step comes next, what it evaluates to and how it is explained all come
//! from the rules in [`crate::python`] and [`crate::logic`] applied to the source below.

use std::{
	collections::{BTreeMap, BTreeSet},
	fmt,
};

use serde::Deserialize;

use crate::{
	core::{EvaluationMode, Language, ParseError, Session, Value},
	logic::{self, Formula, parse_formula, parse_truth, proof::Proof},
	python::{self, parse_value},
};

/// The name the embedded set carries. An imported file may not claim it: progress is kept per
/// set name, so a file calling itself the embedded set would reopen the embedded questions'
/// work as if it owned it.
pub const BUILTIN_SET: &str = "builtin";

/// The set format this build reads. A file naming another version is refused rather than
/// guessed at, so a binary never half-reads a set written for a different protocol.
pub const FORMAT_VERSION: u32 = 1;

/// What one imported set may weigh. The limits on a single expression, formula or proof stay
/// with the language module that parses it; these bound the file around them.
const MAX_BYTES: usize = 1 << 20;
const MAX_QUESTIONS: usize = 1024;
const MAX_PREMISES: usize = 64;

/// One question set: a stable name, its metadata, and its questions in the order they are
/// practised.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionSet {
	version: u32,
	/// Stable across edits and moves. Saved progress follows this name, never the path the
	/// file happens to sit at, so renaming the file keeps the work and renaming the set
	/// deliberately parts with it.
	pub id: String,
	pub title: String,
	#[serde(default)]
	pub description: Option<String>,
	#[serde(default)]
	questions: Vec<Question>,
}

/// One question in a set. The type is written out in the file rather than inferred from
/// which fields happen to be present, so a field belonging to the other type is refused
/// instead of quietly ignored.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Question {
	/// Step-by-step evaluation, in Python or in propositional logic.
	Evaluation(Exercise),
	/// Classical natural deduction: the premises given and the formula to derive.
	Proof(ProofQuestion),
}

/// An evaluation question: the source to work through, the language whose rules apply, and
/// the value each name stands for.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exercise {
	/// The set this question came from, stamped by [`QuestionSet::parse`] and never a field
	/// of the file. Empty for a random or custom question, which belongs to no set. Progress
	/// pairs it with `id`, so two sets that both name a question `q1` never reopen each
	/// other's work.
	#[serde(skip)]
	pub set: String,
	pub id: String,
	pub title: String,
	pub language: Language,
	pub expression: String,
	/// The prose shown with the question, not a formula.
	pub goal: String,
	/// Source literals of `language`, parsed by that language alone: `"0"` is not `"0.0"`,
	/// `"-0.0"` is not `"0.0"`, and `"True"` is not `"1"`. Writing them as TOML numbers would
	/// lose exactly those distinctions, so they stay strings.
	#[serde(default)]
	pub bindings: BTreeMap<String, String>,
	/// The strategy this question opens in. The student still switches it, and each strategy
	/// keeps its own progress.
	#[serde(default)]
	pub evaluation: Option<EvaluationMode>,
}

/// A natural-deduction question. `conclusion` is the formula to derive; `goal` is prose, as
/// it is on [`Exercise`]. Short circuit is an evaluation strategy and has no meaning here,
/// so `deny_unknown_fields` refuses an `evaluation` field on this type.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofQuestion {
	pub id: String,
	pub title: String,
	/// The prose shown with the question, not a formula.
	pub goal: String,
	#[serde(default)]
	pub premises: Vec<String>,
	pub conclusion: String,
}

/// Why a set was refused. Both spell the question and the field to correct; neither names a
/// file, because reading one belongs to whoever owns the terminal or the window.
#[derive(Debug)]
pub enum SetError {
	/// Refused by TOML itself: bad syntax, a wrong field type, an unknown field or an
	/// unknown question type. The message carries the line, the column and the offending
	/// text already.
	Syntax(Box<toml::de::Error>),
	/// Read as TOML, but saying something that cannot be taught.
	Invalid(Invalid),
}

/// A set that parsed and still cannot be practised, located well enough to fix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invalid {
	/// The question at fault, or `None` when the set itself is. A question with no usable ID
	/// is named by its position in the file.
	pub question: Option<String>,
	/// The field to correct, empty where the whole file is at fault rather than one field.
	pub field: &'static str,
	pub message: String,
}

impl fmt::Display for Invalid {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match (&self.question, self.field) {
			(Some(id), field) => write!(formatter, "题目 {id} 的 {field} 字段：{}", self.message),
			(None, "") => write!(formatter, "题集文件：{}", self.message),
			(None, field) => write!(formatter, "题集的 {field} 字段：{}", self.message),
		}
	}
}

impl fmt::Display for SetError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Syntax(error) => write!(formatter, "{error}"),
			Self::Invalid(invalid) => write!(formatter, "{invalid}"),
		}
	}
}

impl std::error::Error for SetError {}

impl From<Invalid> for SetError {
	fn from(invalid: Invalid) -> Self {
		Self::Invalid(invalid)
	}
}

impl QuestionSet {
	/// Read one set: TOML first, then the checks its own syntax cannot make. Every set
	/// arrives through here — the embedded one included — so a question is held to the same
	/// rules however it reached the program.
	pub fn parse(text: &str) -> Result<Self, SetError> {
		if text.len() > MAX_BYTES {
			return Err(Invalid::file(format!(
				"题集有 {} 字节，超过上限 {MAX_BYTES} 字节。",
				text.len()
			))
			.into());
		}
		// The version is read on its own first. A set written for a later protocol carries
		// fields this build has never heard of, and "unknown field" would bury the reason.
		#[derive(Deserialize)]
		struct Declared {
			version: u32,
		}
		let declared: Declared =
			toml::from_str(text).map_err(|error| SetError::Syntax(error.into()))?;
		if declared.version != FORMAT_VERSION {
			return Err(Invalid::set(
				"version",
				format!(
					"这个程序读 version = {FORMAT_VERSION} 的题集，文件写的是 {}。",
					declared.version
				),
			)
			.into());
		}
		let mut set: Self = toml::from_str(text).map_err(|error| SetError::Syntax(error.into()))?;
		let name: String = set.id.clone();
		for question in &mut set.questions {
			if let Question::Evaluation(exercise) = question {
				exercise.set = name.clone();
			}
		}
		set.validate()?;
		Ok(set)
	}

	/// One set read from outside the binary: the same load path, plus the one rule that only
	/// a file has to answer — it may not call itself [`BUILTIN_SET`].
	pub fn import(text: &str) -> Result<Self, SetError> {
		let set: Self = Self::parse(text)?;
		if set.id == BUILTIN_SET {
			return Err(Invalid::set(
				"id",
				format!("题集 ID {BUILTIN_SET} 留给内置题集；进度按 ID 保存，请另取一个 ID。"),
			)
			.into());
		}
		Ok(set)
	}

	pub fn version(&self) -> u32 {
		self.version
	}
	pub fn questions(&self) -> &[Question] {
		&self.questions
	}
	pub fn find(&self, id: &str) -> Option<&Question> {
		self.questions.iter().find(|question| question.id() == id)
	}
	/// Every evaluation question, in the order the set lists them. The embedded set stays
	/// reachable this way without going through a file.
	pub fn exercises(&self) -> impl Iterator<Item = &Exercise> {
		self.questions.iter().filter_map(Question::evaluation)
	}
	/// The evaluation questions of one language, in the order the set lists them.
	pub fn evaluations(&self, language: Language) -> Vec<Exercise> {
		self.questions
			.iter()
			.filter_map(Question::evaluation)
			.filter(|exercise| exercise.language == language)
			.cloned()
			.collect()
	}

	fn validate(&self) -> Result<(), Invalid> {
		if self.id.trim().is_empty() {
			return Err(Invalid::set(
				"id",
				"题集 ID 不能为空：进度按它保存，换一个 ID 就是另一套题。".into(),
			));
		}
		if self.title.trim().is_empty() {
			return Err(Invalid::set("title", "题集标题不能为空。".into()));
		}
		if self.questions.is_empty() {
			return Err(Invalid::set("questions", "题集里没有题目。".into()));
		}
		if self.questions.len() > MAX_QUESTIONS {
			return Err(Invalid::set(
				"questions",
				format!(
					"题集有 {} 道题，超过上限 {MAX_QUESTIONS} 道。",
					self.questions.len()
				),
			));
		}
		let mut seen: BTreeSet<&str> = BTreeSet::new();
		for (index, question) in self.questions.iter().enumerate() {
			let id: &str = question.id();
			if id.trim().is_empty() {
				// There is no name to quote, so the position in the file is the locator.
				return Err(Invalid::question(
					&format!("#{}", index + 1),
					"id",
					"题目 ID 不能为空。".into(),
				));
			}
			if !seen.insert(id) {
				return Err(Invalid::question(
					id,
					"id",
					"题目 ID 在同一题集内重复。".into(),
				));
			}
			question.validate()?;
		}
		Ok(())
	}
}

impl Invalid {
	fn set(field: &'static str, message: String) -> Self {
		Self {
			question: None,
			field,
			message,
		}
	}
	/// The file as a whole is at fault, so no single field can be named.
	fn file(message: String) -> Self {
		Self {
			question: None,
			field: "",
			message,
		}
	}
	fn question(id: &str, field: &'static str, message: String) -> Self {
		Self {
			question: Some(id.into()),
			field,
			message,
		}
	}
}

impl Question {
	pub fn id(&self) -> &str {
		match self {
			Self::Evaluation(exercise) => &exercise.id,
			Self::Proof(question) => &question.id,
		}
	}
	pub fn title(&self) -> &str {
		match self {
			Self::Evaluation(exercise) => &exercise.title,
			Self::Proof(question) => &question.title,
		}
	}
	/// Natural deduction is propositional, so a proof question is a logic question.
	pub fn language(&self) -> Language {
		match self {
			Self::Evaluation(exercise) => exercise.language,
			Self::Proof(_) => Language::Logic,
		}
	}
	pub fn evaluation(&self) -> Option<&Exercise> {
		match self {
			Self::Evaluation(exercise) => Some(exercise),
			Self::Proof(_) => None,
		}
	}

	fn validate(&self) -> Result<(), Invalid> {
		if self.title().trim().is_empty() {
			return Err(Invalid::question(
				self.id(),
				"title",
				"题目标题不能为空。".into(),
			));
		}
		match self {
			Self::Evaluation(exercise) => exercise.validate(),
			Self::Proof(question) => question.validate(),
		}
	}
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

	/// The strategy this question opens in when nothing else decides: its own field, else
	/// the language's own default.
	pub fn mode(&self) -> EvaluationMode {
		self.evaluation
			.unwrap_or_else(|| self.language.default_mode())
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

	/// A question a student can actually be given: every literal is one this language reads,
	/// and the source parses against exactly these names. A question that ends in an
	/// exception is legal teaching material and passes.
	fn validate(&self) -> Result<(), Invalid> {
		for (name, literal) in &self.bindings {
			let parsed: Result<(), ParseError> = match self.language {
				Language::Python => parse_value(literal).map(drop),
				Language::Logic => parse_truth(literal).map(drop),
			};
			// The language's own words about its own literals; a file never reads advice
			// meant for a student typing an answer.
			if parsed.is_err() {
				return Err(Invalid::question(
					&self.id,
					"bindings",
					format!(
						"{name} = \"{literal}\" 不是可用的取值；这里要写{}。",
						self.language.literal_form()
					),
				));
			}
		}
		self.session(self.mode())
			.map(drop)
			.map_err(|error| Invalid::question(&self.id, "expression", error.to_string()))
	}
}

impl ProofQuestion {
	/// The proof this question opens, checked by the same rules a built-in one is.
	pub fn proof(&self) -> Result<Proof, ParseError> {
		Ok(Proof::new(
			self.premises
				.iter()
				.map(|source| parse_formula(source))
				.collect::<Result<Vec<Formula>, ParseError>>()?,
			parse_formula(&self.conclusion)?,
		))
	}

	/// Premises and conclusion on one line, for a listing.
	pub fn sequent(&self) -> String {
		format!("{} ⊢ {}", self.premises.join("，"), self.conclusion)
	}

	fn validate(&self) -> Result<(), Invalid> {
		if self.premises.len() > MAX_PREMISES {
			return Err(Invalid::question(
				&self.id,
				"premises",
				format!(
					"证明题有 {} 条前提，超过上限 {MAX_PREMISES} 条。",
					self.premises.len()
				),
			));
		}
		for (index, source) in self.premises.iter().enumerate() {
			parse_formula(source).map_err(|error| {
				Invalid::question(
					&self.id,
					"premises",
					format!("第 {} 条「{source}」：{error}", index + 1),
				)
			})?;
		}
		parse_formula(&self.conclusion)
			.map(drop)
			.map_err(|error| Invalid::question(&self.id, "conclusion", error.to_string()))
	}
}

/// The set that ships inside the binary: the default value of the one load path, read from
/// the same TOML protocol an imported file uses.
pub fn builtin() -> Result<QuestionSet, SetError> {
	QuestionSet::parse(include_str!("../assets/exercises.toml"))
}
