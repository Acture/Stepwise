use crate::{
	core::{Language, ParseError},
	exercises::Exercise,
	generate,
};

/// Where practice continues once the student passes the questions already drawn. Another
/// question source adds a variant here rather than a second way to change question.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Supply {
	/// Draw a fresh seeded question in the current question's language; never runs out.
	Random,
	/// The questions are exactly this list, in order; practice ends after the last one.
	Ordered,
}

/// The questions this run has drawn and the one in hand. Which question comes next is a
/// question-source decision, so every front end walks the same course the same way.
#[derive(Clone, Debug)]
pub struct Course {
	questions: Vec<Exercise>,
	index: usize,
	supply: Supply,
}

impl Course {
	pub fn new(questions: Vec<Exercise>, index: usize, supply: Supply) -> Result<Self, ParseError> {
		if index >= questions.len() {
			return Err(ParseError("没有这道题。".into()));
		}
		Ok(Self {
			questions,
			index,
			supply,
		})
	}

	/// The default practice: walk the questions already chosen, then keep generating.
	pub fn random(questions: Vec<Exercise>, index: usize) -> Result<Self, ParseError> {
		Self::new(questions, index, Supply::Random)
	}

	/// A fixed set practised in order, for a question file the student works through.
	pub fn ordered(questions: Vec<Exercise>, index: usize) -> Result<Self, ParseError> {
		Self::new(questions, index, Supply::Ordered)
	}

	pub fn current(&self) -> &Exercise {
		&self.questions[self.index]
	}
	pub fn questions(&self) -> &[Exercise] {
		&self.questions
	}
	pub fn index(&self) -> usize {
		self.index
	}
	pub fn supply(&self) -> Supply {
		self.supply
	}
	/// Each question names its own language; a course may mix them.
	pub fn language(&self) -> Language {
		self.current().language
	}

	/// Move to the next question, drawing one when the supply allows it. False means an
	/// ordered set has ended, and the course is left untouched.
	pub fn forward(&mut self) -> Result<bool, ParseError> {
		if self.index + 1 == self.questions.len() {
			if self.supply == Supply::Ordered {
				return Ok(false);
			}
			let next: Exercise = generate::generate(self.language(), generate::fresh_seed())?;
			self.questions.push(next);
		}
		self.index += 1;
		Ok(true)
	}

	/// Move back through the questions already drawn. False at the first one.
	pub fn backward(&mut self) -> bool {
		let moved: bool = self.index > 0;
		self.index = self.index.saturating_sub(1);
		moved
	}
}
