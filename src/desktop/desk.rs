use std::{collections::BTreeMap, ops::Range};

use super::{
	say,
	view::{
		Blank, Catalog, Choice, Command, CourseView, Draft, EntryView, EvaluationView, Feedback,
		Listed, ProofLineView, ProofView, Run, Tone, View, Written,
	},
};
use crate::{
	app::{self, Archived, Course, Lesson, Practice, ProofPractice, Report, Task, Transcript},
	core::{Language, NodeId, ParseError, Session},
	exercises::{Question, QuestionSet},
	generate,
	logic::proof::{ProofLine, RULES},
	progress::Progress,
};

impl From<Choice> for Language {
	fn from(choice: Choice) -> Self {
		match choice {
			Choice::Python => Self::Python,
			Choice::Logic => Self::Logic,
		}
	}
}

/// The window's state: which screen is up, the lesson in hand and what this window keeps
/// around it. Teaching state lives in the [`Lesson`]; the desk keeps no second copy of the
/// session or the progress, and decides no step.
///
/// Reading a question file and writing the progress file belong to the shell that owns the
/// window: the desk takes text and hands back a snapshot.
pub struct Desk {
	/// The embedded set: random practice resumes from it and its catalog lists it.
	builtin: QuestionSet,
	/// A set opened from a file, walked in order once a language is chosen.
	opened: Option<QuestionSet>,
	stage: Stage,
	/// Something the window has to say that no lesson reported, such as a file that failed.
	message: Option<String>,
}

enum Stage {
	/// Choosing a language, holding the progress snapshot until a lesson takes it.
	Entry(Progress),
	Lesson(Box<Board>),
}

/// A lesson on the board, with what this window keeps around it.
struct Board {
	lesson: Lesson,
	language: Language,
	/// True while an opened set is walked in order; random practice otherwise.
	ordered: bool,
	transcript: Transcript,
	/// The record of the question in hand: what the transcript archived since its block opened.
	record: Vec<Archived>,
	/// The student has pointed at a node, by click or keyboard. Until then nothing is ringed:
	/// the session's own selection starts at the whole expression, which nobody chose.
	pointed: bool,
	/// The last command asked the teaching rules to judge something, so a turned-down report
	/// is a mistake rather than a hint.
	judged: bool,
	rules: bool,
}

impl Board {
	fn new(lesson: Lesson, language: Language, ordered: bool) -> Self {
		let mut board: Self = Self {
			lesson,
			language,
			ordered,
			transcript: Transcript::default(),
			record: Vec::new(),
			pointed: false,
			judged: false,
			rules: false,
		};
		board.sync();
		board
	}

	/// Take what the transcript archived. A break closes the previous question's block, so
	/// the record starts over with the question in hand.
	fn sync(&mut self) {
		for line in self.transcript.sync(&self.lesson) {
			match line {
				Archived::Break => self.record.clear(),
				line => self.record.push(line),
			}
		}
	}

	/// One command on the question in hand. True when the lesson changed in a way worth saving.
	fn apply(&mut self, command: Command) -> Result<bool, ParseError> {
		self.judged = matches!(command, Command::Submit | Command::Select { .. });
		let changed: bool = match command {
			Command::Next | Command::Previous => {
				self.pointed = false;
				self.rules = false;
				if command == Command::Next {
					self.lesson.next_question()?
				} else {
					self.lesson.previous_question()?
				}
			}
			command => match self.lesson.task_mut() {
				Task::Evaluation(practice) => evaluate(practice, command, &mut self.pointed)?,
				Task::Proof(practice) => prove(practice, command, &mut self.rules),
			},
		};
		self.sync();
		Ok(changed)
	}

	fn tone(&self) -> Tone {
		let report: &Report = self.lesson.report();
		if report.good() {
			Tone::Good
		} else if self.judged && matches!(report, Report::Taught { .. }) {
			Tone::Bad
		} else {
			Tone::Plain
		}
	}

	fn feedback(&self) -> Feedback {
		Feedback {
			text: say::sentence(self.lesson.report()),
			tone: self.tone(),
		}
	}

	fn course(&self, set: &QuestionSet) -> CourseView {
		let course: &Course = self.lesson.course();
		let count: usize = course.questions().len();
		CourseView {
			language: match self.lesson.task() {
				Task::Evaluation(_) => say::language(self.language),
				Task::Proof(_) => say::PROOF,
			}
			.into(),
			title: self.lesson.question().title().into(),
			set: self.ordered.then(|| set.title.clone()),
			position: course.index() + 1,
			count: self.ordered.then_some(count),
			back: course.index() > 0,
			forward: !self.ordered || course.index() + 1 < count,
		}
	}

	fn catalog(&self, set: &QuestionSet) -> Catalog {
		let question: &Question = self.lesson.question();
		let questions: Vec<Question> = set.of_language(self.language);
		Catalog {
			title: set.title.clone(),
			current: questions.iter().position(|candidate| {
				candidate.set() == question.set() && candidate.name() == question.name()
			}),
			questions: questions
				.iter()
				.map(|question| Listed {
					name: question.name().into(),
					title: question.title().into(),
					proof: matches!(question, Question::Proof(_)),
				})
				.collect(),
		}
	}

	fn view(&self, set: &QuestionSet, message: Option<String>) -> View {
		match self.lesson.task() {
			Task::Evaluation(practice) => {
				let line: Line = expression(practice, self.pointed);
				View::Evaluation(EvaluationView {
					course: self.course(set),
					catalog: self.catalog(set),
					bindings: practice
						.question()
						.bindings
						.iter()
						.map(|(name, value)| format!("{name} = {value}"))
						.collect(),
					history: self
						.record
						.iter()
						.filter_map(|line| match line {
							Archived::Expression(text) => Some(Written::Expression(text.clone())),
							Archived::TakenBack(text) => Some(Written::TakenBack(text.clone())),
							// The header and the bindings are drawn from the question itself.
							Archived::Break
							| Archived::Heading(_)
							| Archived::Bindings(_)
							| Archived::ProofLine(_) => None,
						})
						.collect(),
					current: line.current,
					extents: line.extents,
					selected: line.selected,
					draft: line.draft,
					ending: line.ending,
					feedback: self.feedback(),
					message,
				})
			}
			Task::Proof(practice) => View::Proof(proof(
				practice,
				self.course(set),
				self.catalog(set),
				self.feedback(),
				(self.judged && self.tone() == Tone::Good)
					.then_some(practice.proof().lines().len()),
				self.rules,
				message,
			)),
		}
	}
}

/// A command on an expression question. True when the session changed.
fn evaluate(
	practice: &mut Practice,
	command: Command,
	pointed: &mut bool,
) -> Result<bool, ParseError> {
	Ok(match command {
		Command::Select { node } => {
			*pointed = true;
			practice.select(node)
		}
		Command::Draft { text } => {
			if practice.draft().is_some() {
				practice.clear_input();
				practice.paste(&text);
			}
			false
		}
		// With no blank open this opens one at the node pointed at: the keyboard reaches the
		// same node a click does.
		Command::Submit => practice.submit(),
		Command::Cancel => {
			practice.cancel();
			false
		}
		Command::Step { forward } => {
			if practice.draft().is_none() {
				*pointed = true;
				if forward {
					practice.select_next();
				} else {
					practice.select_previous();
				}
			}
			false
		}
		Command::Undo => {
			*pointed = false;
			practice.undo()
		}
		Command::Reset => {
			*pointed = false;
			practice.reset()?
		}
		Command::Hint => {
			practice.hint();
			false
		}
		Command::Help => {
			practice.note(say::HELP);
			false
		}
		Command::Choose { .. }
		| Command::Leave
		| Command::Random
		| Command::Pick { .. }
		| Command::Next
		| Command::Previous
		| Command::Rules => false,
	})
}

/// A command on a proof. True when the proof changed.
fn prove(practice: &mut ProofPractice, command: Command, rules: &mut bool) -> bool {
	match command {
		Command::Draft { text } => {
			practice.clear_input();
			practice.paste(&text);
			false
		}
		Command::Submit => practice.submit(),
		Command::Undo => practice.undo(),
		Command::Rules => {
			*rules = !*rules;
			false
		}
		Command::Help => {
			practice.note(say::PROOF_HELP);
			false
		}
		Command::Choose { .. }
		| Command::Leave
		| Command::Random
		| Command::Pick { .. }
		| Command::Select { .. }
		| Command::Cancel
		| Command::Step { .. }
		| Command::Reset
		| Command::Hint
		| Command::Next
		| Command::Previous => false,
	}
}

/// Maximal stretches of the rendered source whose bytes share one owner. A run never
/// straddles a selectable node's edge: inside a node still waiting for a step, every byte
/// belongs to it or to something deeper.
fn owned_runs(source: &str, owners: &[Option<NodeId>]) -> Vec<(Range<usize>, Option<NodeId>)> {
	let mut runs: Vec<(Range<usize>, Option<NodeId>)> = Vec::new();
	for (index, character) in source.char_indices() {
		let end: usize = index + character.len_utf8();
		match runs.last_mut() {
			Some((range, owner)) if *owner == owners[index] => range.end = end,
			_ => runs.push((index..end, owners[index])),
		}
	}
	runs
}

/// The expression half of an evaluation view.
struct Line {
	/// The line in hand, run by run.
	current: Vec<Run>,
	extents: BTreeMap<NodeId, [usize; 2]>,
	selected: Option<NodeId>,
	/// The line being written beneath it, while a blank is open.
	draft: Option<Draft>,
	ending: Option<String>,
}

fn expression(practice: &Practice, pointed: bool) -> Line {
	let session: &Session = practice.session();
	let (source, ranges): (&str, &BTreeMap<NodeId, Range<usize>>) = session.render_with_ranges();
	let runs: Vec<(Range<usize>, Option<NodeId>)> = owned_runs(source, &practice.node_owners());
	let extents: BTreeMap<NodeId, [usize; 2]> = session
		.root()
		.rows()
		.into_iter()
		.filter(|(_, node)| node.value().is_none())
		.filter_map(|(_, node)| {
			let range: &Range<usize> = &ranges[&node.id];
			let inside = |(run, _): &(Range<usize>, Option<NodeId>)| {
				run.start >= range.start && run.end <= range.end
			};
			Some((
				node.id,
				[
					runs.iter().position(inside)?,
					runs.iter().rposition(inside)?,
				],
			))
		})
		.collect();
	let draft: Option<Draft> = practice.draft().map(|editing| {
		let spans: Vec<(NodeId, Range<usize>)> = practice.draft_spans();
		let mut written: Vec<Run> = Vec::new();
		for (range, owner) in &runs {
			match spans
				.iter()
				.find(|(_, span)| span.start <= range.start && range.end <= span.end)
			{
				// One blank stands for every run the replaced node covers.
				Some((id, span)) if span.start == range.start => written.push(Run {
					text: source[span.clone()].into(),
					node: Some(*id),
					blank: Some(if *id == editing {
						Blank::Input
					} else {
						Blank::Mirror
					}),
				}),
				Some(_) => {}
				None => written.push(Run {
					text: source[range.clone()].into(),
					node: *owner,
					blank: None,
				}),
			}
		}
		Draft {
			runs: written,
			input: practice.input().into(),
		}
	});
	let selected: Option<NodeId> = (pointed && draft.is_none())
		.then(|| practice.selected())
		.filter(|node| extents.contains_key(node));
	Line {
		current: runs
			.iter()
			.map(|(range, owner)| Run {
				text: source[range.clone()].into(),
				node: *owner,
				blank: None,
			})
			.collect(),
		extents,
		selected,
		draft,
		ending: practice
			.raised()
			.or_else(|| session.is_finished().then(|| session.render().into())),
	}
}

fn proof(
	practice: &ProofPractice,
	course: CourseView,
	catalog: Catalog,
	feedback: Feedback,
	judged: Option<usize>,
	rules: bool,
	message: Option<String>,
) -> ProofView {
	ProofView {
		course,
		catalog,
		goal: practice.proof().goal.to_string(),
		lines: practice
			.proof()
			.lines()
			.iter()
			.enumerate()
			.map(|(index, line): (usize, &ProofLine)| ProofLineView {
				number: index + 1,
				depth: line.scope.len(),
				formula: line.formula.to_string(),
				rule: line.rule.clone(),
				references: line.references.clone(),
				premise: line.rule == "premise",
				assumption: line.rule == "assume",
			})
			.collect(),
		open: practice.proof().open_assumptions().len(),
		finished: practice.is_finished(),
		input: practice.input().into(),
		feedback,
		judged,
		rules: rules.then(|| RULES.into()),
		message,
	}
}

impl Desk {
	/// A window opening on the language choice. `message` is what the shell could not do
	/// before it, such as reading the progress file.
	pub fn new(progress: Progress, builtin: QuestionSet, message: Option<String>) -> Self {
		Self {
			builtin,
			opened: None,
			stage: Stage::Entry(progress),
			message,
		}
	}

	pub fn view(&self) -> View {
		match &self.stage {
			Stage::Entry(_) => View::Entry(EntryView {
				set: self.opened.as_ref().map(|set| set.title.clone()),
				message: self.message.clone(),
			}),
			Stage::Lesson(board) => board.view(self.set_of(board), self.message.clone()),
		}
	}

	/// The progress to write: the question in hand is recorded first.
	pub fn progress(&mut self) -> &Progress {
		match &mut self.stage {
			Stage::Entry(progress) => progress,
			Stage::Lesson(board) => {
				board.lesson.record();
				board.lesson.progress()
			}
		}
	}

	/// Say something no lesson reported, such as a file the shell could not read.
	pub fn alert(&mut self, message: impl Into<String>) {
		self.message = Some(message.into());
	}

	/// One command from the page. True when the progress changed and is worth writing. A
	/// question that cannot be opened leaves everything as it was and says why.
	pub fn handle(&mut self, command: Command) -> bool {
		self.message = None;
		match self.apply(command) {
			Ok(changed) => changed,
			Err(error) => {
				self.message = Some(error.to_string());
				false
			}
		}
	}

	/// Open a question set from the text of a file; `source` names the file in messages. A
	/// set that fails to load changes nothing, the progress included. On the language choice
	/// the set waits for a language; in a lesson it replaces the course in the same language.
	pub fn load(&mut self, text: &str, source: &str) -> bool {
		self.message = None;
		let set: QuestionSet = match QuestionSet::import(text) {
			Ok(set) => set,
			Err(error) => {
				self.message = Some(format!("题集 {source}：{error}"));
				return false;
			}
		};
		let Stage::Lesson(board) = &self.stage else {
			self.opened = Some(set);
			return false;
		};
		let language: Language = board.language;
		let progress: Progress = self.taken();
		match self.begin(&set, language, progress) {
			Ok(board) => {
				self.opened = Some(set);
				self.stage = Stage::Lesson(Box::new(board));
				true
			}
			Err(error) => {
				self.message = Some(error.to_string());
				false
			}
		}
	}

	fn set_of(&self, board: &Board) -> &QuestionSet {
		match (&self.opened, board.ordered) {
			(Some(set), true) => set,
			_ => &self.builtin,
		}
	}

	/// The progress snapshot, recorded from the lesson in hand, for a new lesson to take.
	fn taken(&mut self) -> Progress {
		self.progress().clone()
	}

	/// A lesson over an opened set in order, from its first unfinished question.
	fn begin(
		&self,
		set: &QuestionSet,
		language: Language,
		progress: Progress,
	) -> Result<Board, ParseError> {
		let questions: Vec<Question> = set.of_language(language);
		if questions.is_empty() {
			return Err(ParseError(say::nothing_in(&set.title, language)));
		}
		let index: usize = app::resume_in_set(&progress, &questions)?;
		Ok(Board::new(
			Lesson::new(Course::ordered(questions, index)?, progress)?,
			language,
			true,
		))
	}

	/// Random practice, opening on `first`.
	fn practise(
		first: Question,
		language: Language,
		progress: Progress,
	) -> Result<Board, ParseError> {
		Ok(Board::new(
			Lesson::new(Course::random(vec![first], 0)?, progress)?,
			language,
			false,
		))
	}

	fn apply(&mut self, command: Command) -> Result<bool, ParseError> {
		let board: Board = match command {
			Command::Choose { language } => {
				let language: Language = language.into();
				let progress: Progress = self.taken();
				match &self.opened {
					Some(set) => self.begin(set, language, progress)?,
					None => {
						let first: Question = app::resume_or_generate(
							language,
							&progress,
							&self.builtin.of_language(language),
						)?;
						Self::practise(first, language, progress)?
					}
				}
			}
			Command::Leave => {
				let progress: Progress = self.taken();
				self.stage = Stage::Entry(progress);
				return Ok(true);
			}
			Command::Random => {
				let Stage::Lesson(board) = &self.stage else {
					return Ok(false);
				};
				let language: Language = board.language;
				let first: Question =
					Question::Evaluation(generate::generate(language, generate::fresh_seed())?);
				let progress: Progress = self.taken();
				self.opened = None;
				Self::practise(first, language, progress)?
			}
			Command::Pick { index } => {
				let Stage::Lesson(board) = &self.stage else {
					return Ok(false);
				};
				let (language, ordered): (Language, bool) = (board.language, board.ordered);
				let mut questions: Vec<Question> = self.set_of(board).of_language(language);
				if index >= questions.len() {
					return Err(ParseError("没有这道题。".into()));
				}
				let progress: Progress = self.taken();
				// An opened set stays in order from the picked question; the embedded set
				// opens it as a launch naming it would, and random questions follow.
				if ordered {
					Board::new(
						Lesson::new(Course::ordered(questions, index)?, progress)?,
						language,
						true,
					)
				} else {
					Self::practise(questions.swap_remove(index), language, progress)?
				}
			}
			command => {
				let Stage::Lesson(board) = &mut self.stage else {
					return Ok(false);
				};
				return board.apply(command);
			}
		};
		self.stage = Stage::Lesson(Box::new(board));
		Ok(true)
	}
}
