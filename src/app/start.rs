use crate::{
	core::{EvaluationMode, Language, ParseError, RecordedAttempt, Session},
	exercises::Exercise,
	generate,
	progress::Progress,
};

/// The question a launch without a named exercise means: unfinished saved progress when it
/// still replays, otherwise a fresh random question. A finished question gives way to a new
/// one, and a saved question in the other language is ignored rather than reopened.
pub fn resume_or_generate(
	language: Language,
	progress: &Progress,
	builtin: &[Exercise],
) -> Result<Exercise, ParseError> {
	let saved: Option<Exercise> = generate::restore(&progress.current)?
		.filter(|exercise| exercise.language == language)
		.or_else(|| {
			builtin
				.iter()
				.find(|exercise| exercise.id == progress.current)
				.cloned()
		});
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

/// The strategy a launch starts in: what the caller asked for, else the saved strategy when
/// it belongs to this very question, else the language's own default.
pub fn starting_mode(
	requested: Option<EvaluationMode>,
	progress: &Progress,
	exercise: &Exercise,
) -> EvaluationMode {
	requested.unwrap_or_else(|| {
		if progress.current == exercise.id {
			progress.mode
		} else {
			exercise.language.default_mode()
		}
	})
}
