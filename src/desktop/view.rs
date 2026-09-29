//! What the desktop page is handed and what it hands back. The page draws a [`View`] and
//! forwards [`Command`]s; it computes no byte offset, decides no step and words no teaching
//! sentence. Under `cargo test` these types also write their TypeScript declarations into
//! `desktop/ui/src/lib/protocol`, so the page cannot drift from them unnoticed.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::core::NodeId;

/// One screen of the window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../desktop/ui/src/lib/protocol/")
)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum View {
	Entry(EntryView),
	Evaluation(EvaluationView),
	Proof(ProofView),
}

/// Choosing a language, before any question is open.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../desktop/ui/src/lib/protocol/")
)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
	/// The title of a question set opened from a file, which the chosen language walks.
	pub set: Option<String>,
	pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../desktop/ui/src/lib/protocol/")
)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
	/// A notice, a hint or a note: nothing was judged.
	Plain,
	/// The teaching rules accepted a step.
	Good,
	/// The teaching rules turned down what the student just submitted or picked.
	Bad,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../desktop/ui/src/lib/protocol/")
)]
pub struct Feedback {
	pub text: String,
	pub tone: Tone,
}

/// Where the question in hand sits in the course.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../desktop/ui/src/lib/protocol/")
)]
#[serde(rename_all = "camelCase")]
pub struct CourseView {
	pub language: String,
	pub title: String,
	/// The set's title while a set opened from a file is walked in order.
	pub set: Option<String>,
	pub position: usize,
	/// How many questions an ordered set holds; random practice never runs out.
	pub count: Option<usize>,
	pub back: bool,
	pub forward: bool,
}

/// The questions the student may pick from, in order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../desktop/ui/src/lib/protocol/")
)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
	pub title: String,
	pub questions: Vec<Listed>,
	pub current: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../desktop/ui/src/lib/protocol/")
)]
pub struct Listed {
	pub name: String,
	pub title: String,
	pub proof: bool,
}

/// One stretch of the rendered expression owned by one node, or by none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../desktop/ui/src/lib/protocol/")
)]
pub struct Run {
	pub text: String,
	pub node: Option<NodeId>,
	pub blank: Option<Blank>,
}

/// The open blank takes the typed answer; a mirror repeats it at another occurrence of the
/// same name, since one answer substitutes every one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../desktop/ui/src/lib/protocol/")
)]
#[serde(rename_all = "lowercase")]
pub enum Blank {
	Input,
	Mirror,
}

/// The next line being written: the current one with the chosen node blanked out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../desktop/ui/src/lib/protocol/")
)]
pub struct Draft {
	pub runs: Vec<Run>,
	pub input: String,
}

/// A line of the question's record above the line in hand.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../desktop/ui/src/lib/protocol/")
)]
#[serde(tag = "kind", content = "text", rename_all = "kebab-case")]
pub enum Written {
	Expression(String),
	TakenBack(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../desktop/ui/src/lib/protocol/")
)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationView {
	/// Which state of the board this is; see [`Command::Draft`].
	pub edition: u32,
	pub course: CourseView,
	pub catalog: Catalog,
	pub bindings: Vec<String>,
	pub history: Vec<Written>,
	pub current: Vec<Run>,
	/// The run indices, first and last, each selectable node covers.
	#[cfg_attr(test, ts(type = "Record<number, [number, number]>"))]
	pub extents: BTreeMap<NodeId, [usize; 2]>,
	/// The node the student pointed at with the keyboard or a click, while no blank is open.
	pub selected: Option<NodeId>,
	pub draft: Option<Draft>,
	/// How the question ended: its value, or the sub-expression that raised and what.
	pub ending: Option<String>,
	pub feedback: Feedback,
	pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../desktop/ui/src/lib/protocol/")
)]
pub struct ProofLineView {
	pub number: usize,
	pub depth: usize,
	pub formula: String,
	pub rule: String,
	pub references: Vec<usize>,
	pub premise: bool,
	pub assumption: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../desktop/ui/src/lib/protocol/")
)]
#[serde(rename_all = "camelCase")]
pub struct ProofView {
	/// Which state of the board this is; see [`Command::Draft`].
	pub edition: u32,
	pub course: CourseView,
	pub catalog: Catalog,
	pub goal: String,
	pub lines: Vec<ProofLineView>,
	/// How many assumptions are open: the depth the next line is written at.
	pub open: usize,
	pub finished: bool,
	pub input: String,
	pub feedback: Feedback,
	/// The line the feedback judged, while that verdict is the last word.
	pub judged: Option<usize>,
	pub rules: Option<String>,
	pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../desktop/ui/src/lib/protocol/")
)]
#[serde(rename_all = "lowercase")]
pub enum Choice {
	Python,
	Logic,
}

/// Every input the page forwards; each maps to one operation of the app layer.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[cfg_attr(
	test,
	derive(ts_rs::TS),
	ts(export, export_to = "../desktop/ui/src/lib/protocol/")
)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Command {
	/// Practise a language: the opened set in order, or random questions.
	Choose {
		language: Choice,
	},
	/// Back to choosing a language.
	Leave,
	/// Random questions in the current language, leaving any opened set.
	Random,
	/// A question of the catalog, by its position there.
	Pick {
		index: usize,
	},
	Select {
		node: NodeId,
	},
	/// The whole draft as the student has typed it, for the board of the edition it was typed
	/// on. Every other command moves the board to a new edition, and a draft for an earlier one
	/// belongs to a blank or a line that is gone, so it is dropped.
	Draft {
		text: String,
		edition: u32,
	},
	Submit,
	Cancel,
	/// Point at the next or previous node still waiting for a step.
	Step {
		forward: bool,
	},
	Undo,
	Reset,
	Hint,
	Help,
	Next,
	Previous,
	/// Show or hide the proof rules.
	Rules,
}
