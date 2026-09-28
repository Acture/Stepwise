//! The teaching machinery both languages share: the tree a student clicks, stable node
//! identifiers, substitution and source mapping, which steps are allowed, typed feedback,
//! history and replay. The steps a student may submit are kept apart from the fixed
//! short-circuit routes a hint names and random questions are bounded by. Operator rules,
//! type semantics and per-operator explanations belong to [`crate::python`] and
//! [`crate::logic`], reached only through [`Language`] and [`Op`]; `language.rs` is the only
//! file here that names them, and each language builds its own session through
//! `Session::new`.
//! Core words only what it judged — an attempt, or the next step a hint names. Those
//! sentences are shared by both languages and never reworded per language. Two name Python
//! because only Python raises: the note on a limit of this program, and the note that an
//! exception came from a sub-expression the student chose to compute. It words nothing about
//! the practice around a step; the layer above types those reasons and a front end says them.

mod ast;
mod error;
mod eval;
mod language;
mod selection;
mod session;
mod surface;
mod value;

pub use ast::{Expr, ExprKind, NodeId};
pub use error::{EvalError, ParseError};
pub use eval::{NextStep, Outcome};
pub use language::{Language, Op};
pub use session::{Feedback, FeedbackKind, HistoryEntry, RecordedAttempt, Session};
pub use value::Value;

#[cfg(test)]
pub(crate) use eval::own_step;
pub(crate) use eval::{Path, reference_step};
pub(crate) use language::{Layout, Rules};
#[cfg(test)]
pub(crate) use selection::reference_steps;
