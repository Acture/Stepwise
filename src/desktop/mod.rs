//! The desktop front end's Rust half: an adapter over [`crate::app`] for a window whose page
//! draws and forwards input. It maps each [`Command`] the page sends to an operation of the
//! [`crate::app::Lesson`] and hands back a [`View`] of the state; it keeps no second copy of
//! the session or the progress and re-derives no teaching rule. Runs and extents come from
//! `node_owners` and `render_with_ranges`, blanks from `draft_spans`, the record above the
//! line in hand from the [`crate::app::Transcript`].
//!
//! It owns every word for every [`crate::app::Notice`] in this window — `say.rs`, with its
//! help — and names no window library and no file path: the shell that owns the window reads
//! files and writes the progress snapshot this hands it.

mod desk;
mod say;
mod view;

pub use desk::Desk;
pub use view::{
	Blank, Catalog, Choice, Command, CourseView, Draft, EntryView, EvaluationView, Feedback,
	Listed, ProofLineView, ProofView, Run, Tone, View, Written,
};
