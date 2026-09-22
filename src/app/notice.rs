use crate::core::{EvaluationMode, Feedback};

/// Why the app layer needs something said when no step was judged. It stands beside
/// [`crate::core::FeedbackKind`] and is never merged into it: that one types what the
/// teaching rules made of a student's answer or selection, this one types a notice about
/// the practice around it.
///
/// None of these carries a sentence. Several of them cannot be said without naming a key or
/// an input device, which only a front end knows, and the rest keep the same rule rather
/// than splitting the set — so a front end answers every reason in one place instead of
/// reading some words from here and writing the others itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
	/// A question just opened and nothing has been selected yet.
	Start,
	/// A blank is open at the selected node, waiting for a typed answer.
	DraftOpen,
	/// The whole expression is one operation over two values, so its blank opened with no
	/// selection. The student still types and submits the value.
	FinalPair,
	/// A hint was asked for where no step is left to name.
	NoNextStep,
	/// The last student step was taken back.
	Undone,
	/// The question started again from its source.
	Restarted,
	/// The evaluation strategy changed to this one; each keeps its own progress.
	ModeSwitched(EvaluationMode),
	/// An ordered set has no question after the current one.
	CourseEnded,
	/// A proof just opened and no line has been entered.
	ProofStart,
	/// The last proof line was taken back, reopening its assumption scope.
	ProofUndone,
}

/// What a front end has to show after an operation: a sentence the teaching rules already
/// worded, a [`Notice`] it words itself, or a sentence it handed in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Report {
	/// Worded by the teaching rules — what they made of an attempt, the next step a hint
	/// names, or how a proof line checked out — with their own acceptance.
	Taught { message: String, accepted: bool },
	/// Not worded at all: the reason alone, for the front end to say.
	Notice(Notice),
	/// Worded by the front end and handed back in, such as its own key help.
	Note(String),
}

impl Report {
	/// True only where the teaching rules accepted a step; a notice is never good news.
	pub fn good(&self) -> bool {
		matches!(self, Self::Taught { accepted: true, .. })
	}
}

impl From<Feedback> for Report {
	fn from(feedback: Feedback) -> Self {
		Self::Taught {
			accepted: feedback.accepted(),
			message: feedback.message,
		}
	}
}
