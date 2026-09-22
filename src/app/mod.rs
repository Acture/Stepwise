//! The application layer: choosing questions, running one teaching session, keeping the
//! student's selection and draft, and holding the progress snapshot. It depends on the
//! teaching rules in [`crate::core`] and the two language modules, and on nothing else: no
//! terminal library, no window toolkit, no event library and no file system. CLAUDE.md
//! spells the grep that keeps it that way.
//!
//! A front end is an adapter over [`Practice`] and [`ProofPractice`]: it maps its own input
//! to their operations and draws their readable state. It never keeps a second copy of the
//! session or the progress, and it never re-derives a teaching rule — which source range a
//! draft stands for, which node a character belongs to and which display states to archive
//! all come from here.
//!
//! Persistence stays at the boundary: the caller loads a [`crate::progress::Progress`],
//! hands it over, and writes it back out after an operation reports a change. Deciding when
//! to touch the disk belongs to the process that owns the terminal or the window.

mod course;
mod practice;
mod proof;
mod start;
mod transcript;

pub use course::{Course, Supply};
pub use practice::Practice;
pub use proof::ProofPractice;
pub use start::{resume_or_generate, starting_mode};
pub use transcript::Transcript;
