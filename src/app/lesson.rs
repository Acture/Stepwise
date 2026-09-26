use super::{Course, Notice, Practice, ProofPractice, Report};
use crate::{
	core::{EvaluationMode, ParseError},
	exercises::Question,
	progress::Progress,
};

/// The question in hand: an expression to evaluate or a proof to write. A front end draws
/// and drives whichever this is; moving between questions is the [`Lesson`]'s.
pub enum Task {
	Evaluation(Practice),
	Proof(ProofPractice),
}

impl Task {
	/// Open a question of either kind, replaying whatever the snapshot saved for it.
	fn open(
		question: &Question,
		progress: Progress,
		mode: EvaluationMode,
	) -> Result<Self, ParseError> {
		Ok(match question {
			Question::Evaluation(exercise) => {
				Self::Evaluation(Practice::new(exercise.clone(), progress, mode)?)
			}
			Question::Proof(proof) => Self::Proof(ProofPractice::new(proof.clone(), progress)?),
		})
	}

	pub fn progress(&self) -> &Progress {
		match self {
			Self::Evaluation(practice) => practice.progress(),
			Self::Proof(practice) => practice.progress(),
		}
	}
	pub fn report(&self) -> &Report {
		match self {
			Self::Evaluation(practice) => practice.report(),
			Self::Proof(practice) => practice.report(),
		}
	}
	/// Copy the work on this question into the progress snapshot.
	pub fn record(&mut self) {
		match self {
			Self::Evaluation(practice) => practice.record(),
			Self::Proof(practice) => practice.record(),
		}
	}
	fn notify(&mut self, notice: Notice) {
		match self {
			Self::Evaluation(practice) => practice.notify(notice),
			Self::Proof(practice) => practice.notify(notice),
		}
	}
}

/// A course walked one question at a time. Forward, back, the end of an ordered set and the
/// progress snapshot work the same whether the question in hand is an expression or a proof,
/// so no front end keeps a second way to move between the two kinds.
pub struct Lesson {
	course: Course,
	/// The strategy the next evaluation question opens in. An evaluation question in hand
	/// keeps its own in its session; this carries it across a proof, which has none.
	mode: EvaluationMode,
	task: Task,
}

impl Lesson {
	/// Opens the course's current question in `mode` if it is an evaluation one; either way
	/// `mode` is what later evaluation questions open in.
	pub fn new(
		course: Course,
		progress: Progress,
		mode: EvaluationMode,
	) -> Result<Self, ParseError> {
		let task: Task = Task::open(course.current(), progress, mode)?;
		Ok(Self { course, mode, task })
	}

	pub fn course(&self) -> &Course {
		&self.course
	}
	pub fn question(&self) -> &Question {
		self.course.current()
	}
	pub fn task(&self) -> &Task {
		&self.task
	}
	pub fn task_mut(&mut self) -> &mut Task {
		&mut self.task
	}
	pub fn progress(&self) -> &Progress {
		self.task.progress()
	}
	pub fn report(&self) -> &Report {
		self.task.report()
	}
	/// The strategy an evaluation question opens in next: the one in hand if the student is
	/// evaluating, else the one carried across.
	pub fn mode(&self) -> EvaluationMode {
		match &self.task {
			Task::Evaluation(practice) => practice.session().mode(),
			Task::Proof(_) => self.mode,
		}
	}
	/// Copy the work on the question in hand into the progress snapshot. Writing it out
	/// belongs to the caller.
	pub fn record(&mut self) {
		self.task.record();
	}

	/// Move to the next question the course supplies. False when an ordered set has ended,
	/// which the question in hand reports.
	pub fn next_question(&mut self) -> Result<bool, ParseError> {
		self.record();
		let mut course: Course = self.course.clone();
		if !course.forward()? {
			self.task.notify(Notice::CourseEnded);
			return Ok(false);
		}
		self.enter(course)?;
		Ok(true)
	}

	/// Go back to a question drawn earlier in this run, restoring its recorded work. False at
	/// the first question, which stays exactly as it is: reopening it would throw away a
	/// half-typed answer or proof line for a move that goes nowhere.
	pub fn previous_question(&mut self) -> Result<bool, ParseError> {
		let mut course: Course = self.course.clone();
		course.backward();
		if course.index() == self.course.index() {
			return Ok(false);
		}
		self.record();
		self.enter(course)?;
		Ok(true)
	}

	/// Open the course's current question with the snapshot as it stands. Leaves the lesson
	/// untouched when that question cannot be built.
	fn enter(&mut self, course: Course) -> Result<(), ParseError> {
		let mode: EvaluationMode = self.mode();
		let task: Task = Task::open(course.current(), self.progress().clone(), mode)?;
		*self = Self { course, mode, task };
		Ok(())
	}
}
