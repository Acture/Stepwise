use crate::{
	core::{EvaluationMode, Language, ParseError, RecordedAttempt, Session},
	exercises::Exercise,
	generate,
	progress::Progress,
};

/// The question a launch without a named exercise means in random practice: unfinished saved
/// progress when it still replays, otherwise a fresh random question. A finished question
/// gives way to a new one, and a saved question belonging to another set or to the other
/// language is left where it is rather than reopened.
pub fn resume_or_generate(
	language: Language,
	progress: &Progress,
	questions: &[Exercise],
) -> Result<Exercise, ParseError> {
	// A random question belongs to no set, so an empty set name is what points at one.
	let saved: Option<Exercise> = if progress.current_set.is_empty() {
		generate::restore(&progress.current)?.filter(|exercise| exercise.language == language)
	} else {
		questions
			.iter()
			.find(|exercise| progress.points_at(&exercise.set, &exercise.id))
			.filter(|exercise| exercise.language == language)
			.cloned()
	};
	if let Some(exercise) = saved {
		let initial: Session = exercise.session(progress.mode)?;
		let attempts: &[RecordedAttempt] = progress.attempts(&initial);
		let resume: bool = exercise.id.starts_with("random-") || !attempts.is_empty();
		if resume && !initial.replay(attempts)?.is_finished() {
			return Ok(exercise);
		}
	}
	generate::generate(language, generate::fresh_seed())
}

/// Where an ordered set opens: the first question still unfinished, looked for from where
/// progress left off inside this very set. Only one pointer is saved, so a set the pointer
/// does not name is searched from its first question rather than restarted at it; a set with
/// nothing left opens its last. An ordered set never generates a question.
/// `questions` is the set's practisable questions and must not be empty.
pub fn resume_in_set(
	progress: &Progress,
	questions: &[Exercise],
	requested: Option<EvaluationMode>,
) -> Result<usize, ParseError> {
	let remembered: usize = questions
		.iter()
		.position(|exercise| progress.points_at(&exercise.set, &exercise.id))
		.unwrap_or(0);
	for (index, exercise) in questions.iter().enumerate().skip(remembered) {
		let initial: Session = exercise.session(starting_mode(requested, progress, exercise))?;
		let attempts: &[RecordedAttempt] = progress.attempts(&initial);
		if !initial.replay(attempts)?.is_finished() {
			return Ok(index);
		}
	}
	Ok(questions.len() - 1)
}

/// The strategy a launch starts in: what the caller asked for, else the saved strategy when
/// it belongs to this very question of this very set, else the question's own.
pub fn starting_mode(
	requested: Option<EvaluationMode>,
	progress: &Progress,
	exercise: &Exercise,
) -> EvaluationMode {
	requested.unwrap_or_else(|| {
		if progress.points_at(&exercise.set, &exercise.id) {
			progress.mode
		} else {
			exercise.mode()
		}
	})
}
