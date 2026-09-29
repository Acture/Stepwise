//! The words this window puts to what the app layer reports, and its own help. Several name
//! a key or a control only this window has, which is why [`Notice`] arrives as a reason at
//! all: the terminal answers the same reasons in its own words in `tui/say.rs`.
//!
//! The match over [`Notice`] is exhaustive on purpose — a new reason stops compiling here
//! until this window has said it.

use crate::{
	app::{Notice, Report},
	core::Language,
};

/// The keys and controls this window binds, said once for the help the student asks for.
pub const HELP: &str = "点当前式里的一处子式，它下面会多写一行，被点的地方变成空格；在空格上填值，Enter 检查。填错不推进，改了再交；Esc 取消。能短路的运算可以直接点整体，也可以继续计算会被跳过的操作数——那是本练习允许的额外求值，不是 Python 的执行顺序。键盘也能做同样的事：↑ ↓ 选一处，Enter 打开空格；u 撤销，r 重来，n 下一题，p 上一题，h 提示。";

/// The help a proof asks for: where proof lines go and how to move around them.
pub const PROOF_HELP: &str = "每行写：公式 ; 规则 ; 引用行，Enter 检查。前提是第 1 至 n 行，之后每接受一行编号加一。点「规则」看全部规则，「撤销一行」退回上一行及它开关的假设；换题用标题栏的 ‹ ›。";

/// The sentence for a report. Which reports count as good news is the app layer's rule.
pub fn sentence(report: &Report) -> String {
	match report {
		Report::Taught { message, .. } | Report::Note(message) => message.clone(),
		Report::Notice(notice) => notice_sentence(*notice).into(),
	}
}

fn notice_sentence(notice: Notice) -> &'static str {
	match notice {
		Notice::Start => "点一处子式，在下一行写出它的值，Enter 检查。",
		Notice::DraftOpen => "在空格上写出这一步的值，Enter 检查；Esc 取消。",
		Notice::FinalPair => "只剩两个值，直接写出这一步的结果，Enter 检查。",
		Notice::NoNextStep => "这题已经算完。点「下一题」继续，或点「撤销」退回一步。",
		Notice::Undone => "已撤销上一步。",
		Notice::Restarted => "已重新开始本题。",
		Notice::CourseEnded => "已经是本题集的最后一题。",
		Notice::ProofStart => {
			"下一行写：公式 ; 规则 ; 引用行，Enter 检查。点「规则」查看全部规则。"
		}
		Notice::ProofUndone => "已撤销上一行，并恢复对应的假设作用域。",
	}
}

/// The language as the header names it.
pub fn language(language: Language) -> &'static str {
	match language {
		Language::Python => "Python",
		Language::Logic => "命题逻辑",
	}
}

/// The header's name for a proof, which only propositional logic has.
pub const PROOF: &str = "自然演绎";

/// Why a chosen language has nothing to practise in the opened set.
pub fn nothing_in(set: &str, language: Language) -> String {
	format!("题集「{set}」里没有{}的题目。", self::language(language))
}
