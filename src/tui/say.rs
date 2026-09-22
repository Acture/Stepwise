//! The words this front end puts to what the app layer reports. Several of the sentences
//! below name a key or an input device only this terminal binds, which is why [`Notice`]
//! arrives as a reason at all: another front end answers the same reasons in its own words.
//! The `?` help in [`super::keys`] is the other half of the same rule.
//!
//! The match over [`Notice`] is exhaustive on purpose — a new reason stops compiling here
//! until this terminal has said it.

use crate::app::{Notice, Report};

/// The sentence to draw in the feedback line, and whether it is good news. Which reports
/// count as good is the app layer's rule, so this asks rather than deciding again.
pub(super) fn feedback(report: &Report) -> (String, bool) {
	let sentence: String = match report {
		Report::Taught { message, .. } | Report::Note(message) => message.clone(),
		Report::Notice(notice) => notice_sentence(*notice),
	};
	(sentence, report.good())
}

fn notice_sentence(notice: Notice) -> String {
	match notice {
		Notice::Start => "点击一处 → ____ → 填值 → Enter。".into(),
		Notice::DraftOpen => "在 ____ 处填值，Enter 检查；Esc 取消。".into(),
		Notice::FinalPair => "只剩两个值，直接填入本步结果，Enter 检查。".into(),
		Notice::NoNextStep => "本题已结束。可以按 n 进入下一题，或按 u 撤销。".into(),
		Notice::Undone => "已撤销上一步。".into(),
		Notice::Restarted => "已重新开始本题。".into(),
		Notice::ModeSwitched(mode) => format!("已切换：{}。进度分别保存。", mode.label()),
		Notice::CourseEnded => "已经是本题集的最后一题。".into(),
		Notice::ProofStart => "下一行：公式 ; 规则 ; 引用行。Enter 检查，F1 查看规则。".into(),
		Notice::ProofUndone => "已撤销上一行，并恢复对应的假设作用域。".into(),
	}
}
