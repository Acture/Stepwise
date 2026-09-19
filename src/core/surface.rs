use std::{collections::BTreeMap, ops::Range};

use super::{Expr, NodeId};

/// Presentation spans are separate from evaluation: replacing a node must not eat its brackets.
#[derive(Clone, Debug)]
pub(super) struct Surface {
	pub text: String,
	pub ranges: BTreeMap<NodeId, Range<usize>>,
}

impl Surface {
	pub fn new(source: &str, root: &Expr) -> Self {
		Self {
			text: source.into(),
			ranges: root
				.rows()
				.into_iter()
				.map(|(_, node)| (node.id, node.source_range.clone()))
				.collect(),
		}
	}

	pub fn replace(&mut self, id: NodeId, replacement: &str, root: &Expr) {
		let removed: Range<usize> = self.ranges[&id].clone();
		let word: fn(char) -> bool = |character| character.is_alphanumeric() || character == '_';
		let before: bool = self.text[..removed.start]
			.chars()
			.next_back()
			.is_some_and(word)
			&& replacement.chars().next().is_some_and(word);
		let after: bool = replacement.chars().next_back().is_some_and(word)
			&& self.text[removed.end..].chars().next().is_some_and(word);
		let replacement: String = format!(
			"{}{}{}",
			if before { " " } else { "" },
			replacement,
			if after { " " } else { "" }
		);
		self.text.replace_range(removed.clone(), &replacement);
		let shift: isize = replacement.len() as isize - removed.len() as isize;
		self.ranges.retain(|node, range| {
			if root.find(*node).is_none() {
				return false;
			}
			if range.start >= removed.end {
				range.start = range
					.start
					.checked_add_signed(shift)
					.expect("valid shifted span");
				range.end = range
					.end
					.checked_add_signed(shift)
					.expect("valid shifted span");
			} else if range.end >= removed.end {
				range.end = range
					.end
					.checked_add_signed(shift)
					.expect("valid enclosing span");
			}
			true
		});
	}
}
