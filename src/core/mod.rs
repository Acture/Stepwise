//! The teaching machinery both languages share: the tree a student clicks, stable node
//! identifiers, substitution and source mapping, which steps are allowed, typed feedback,
//! history and replay. Operator rules, type semantics and per-operator explanations belong
//! to [`crate::python`] and [`crate::logic`], reached only through [`Language`] and [`Op`];
//! `language.rs` is the only file here that names them, and each language builds its own
//! session through `Session::new`.
//! The feedback sentences core itself prints are shared by both languages and predate this
//! split; a few still use Python vocabulary and are kept byte-identical on purpose.

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
pub use eval::{EvaluationMode, NextStep, Outcome, next_step};
pub use language::{Language, Op};
pub use session::{Feedback, FeedbackKind, HistoryEntry, RecordedAttempt, Session};
pub use value::Value;

pub(crate) use language::{Layout, Rules, Step};
