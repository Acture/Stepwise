mod formula;
pub mod proof;

pub(crate) use formula::parse_teaching_formula;
pub use formula::{Formula, LogicOp, parse_formula, parse_truth};
