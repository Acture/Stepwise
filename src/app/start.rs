use crate::{
	core::{EvaluationMode, Language, ParseError, RecordedAttempt, Session},
	exercises::{Exercise, Question},
	generate,
	logic::proof::Proof,
	progress::Progress,
};

/// The question a launch without a named exercise means in random practice: unfinished saved
/// progress when it still replays, otherwise a fresh random question. A finished question
/// gives way to a new one, and a saved question belonging to another set or to the other
/// language is left where it is rather than reopened. A proof the student left unfinished is
/// saved work like any other and reopens the same way.
pub fn resume_or_generate(
	language: Language,
	progress: &Progress,
	questions: &[Question],
) -> Result<Question, ParseError> {
	// A random question belongs to no set, so an empty set name is what points at one.
	let saved: Option<Question> = if progress.current_set.is_empty() {
		generate::restore(&progress.current)?
			.filter(|exercise| exercise.language == language)
			.map(Question::Evaluation)
	} else {
		questions
			.iter()
			.find(|question| progress.points_at(question.set(), question.name()))
			.filter(|question| question.language() == language)
			.cloned()
	};
	if let Some(question) = saved {
		let begun: bool = match &question {
			Question::Evaluation(exercise) => {
				exercise.name.starts_with("random-")
					|| !progress
						.attempts(&exercise.session(progress.mode)?)
						.is_empty()
			}
			Question::Proof(proof) => !progress.commands(&proof.proof()?).is_empty(),
		};
		if begun && !finished(&question, progress, progress.mode)? {
			return Ok(question);
		}
	}
	Ok(Question::Evaluation(generate::generate(
		language,
		generate::fresh_seed(),
	)?))
}

/// Where an ordered set opens: the first question still unfinished, looked for from where
/// progress left off inside this very set. Only one pointer is saved, so a set the pointer
/// does not name is searched from its first question rather than restarted at it; a set with
/// nothing left opens its last. An ordered set never generates a question.
/// `questions` is the set's practisable questions and must not be empty.
pub fn resume_in_set(
	progress: &Progress,
	questions: &[Question],
	requested: Option<EvaluationMode>,
) -> Result<usize, ParseError> {
	let remembered: usize = questions
		.iter()
		.position(|question| progress.points_at(question.set(), question.name()))
		.unwrap_or(0);
	for (index, question) in questions.iter().enumerate().skip(remembered) {
		if !finished(
			question,
			progress,
			starting_mode(requested, progress, question),
		)? {
			return Ok(index);
		}
	}
	Ok(questions.len() - 1)
}

/// Whether the work saved for this question already finishes it: an evaluation question
/// replayed in `mode`, or a proof replayed from its lines.
fn finished(
	question: &Question,
	progress: &Progress,
	mode: EvaluationMode,
) -> Result<bool, ParseError> {
	Ok(match question {
		Question::Evaluation(exercise) => {
			let initial: Session = exercise.session(mode)?;
			let attempts: &[RecordedAttempt] = progress.attempts(&initial);
			initial.replay(attempts)?.is_finished()
		}
		Question::Proof(question) => {
			let initial: Proof = question.proof()?;
			let commands: &[String] = progress.commands(&initial);
			initial.clone().replay(commands)?.is_finished()
		}
	})
}

/// The strategy a launch starts in: what the caller asked for, else the saved strategy when
/// it belongs to this very question of this very set, else the question's own. A proof has no
/// strategy of its own, so a course that opens on one carries the caller's, or the default,
/// into the evaluation questions after it.
pub fn starting_mode(
	requested: Option<EvaluationMode>,
	progress: &Progress,
	question: &Question,
) -> EvaluationMode {
	requested.unwrap_or_else(|| match question {
		Question::Evaluation(exercise) => evaluation_mode(progress, exercise),
		Question::Proof(_) => EvaluationMode::default(),
	})
}

fn evaluation_mode(progress: &Progress, exercise: &Exercise) -> EvaluationMode {
	if progress.points_at(&exercise.set, &exercise.name) {
		progress.mode
	} else {
		exercise.mode()
	}
}
