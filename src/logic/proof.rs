use super::{Formula, LogicOp, parse_formula};
use crate::core::ParseError;

pub const RULES: &str = "格式：公式 ; 规则 ; 引用行（逗号分隔的行号）\n前提是第 1 至 n 行，之后每接受一行编号加一\n例：第 3 行 A→B、第 5 行 A，写 B ; mp ; 3,5\nassume 假设，无引用；其后的行缩进，属于这个子证明\nmp 肯定前件 / →消去，引用 A→B 和 A\nand-intro ∧引入，引用 A 和 B\nand-left / and-right ∧消去，引用 A∧B\nor-left / or-right ∨引入，引用对应一侧\nnot-elim 矛盾，引用 A 和 ¬A，填写 ⊥\nimp-intro →引入，引用假设行,末行，关闭子证明\nnot-intro ¬引入，引用假设行,矛盾末行\nraa 反证法，引用否定假设行,矛盾末行\niff-intro ↔引入，引用 A→B 和 B→A\niff-left / iff-right ↔消去，得对应蕴涵\nbottom-elim ⊥消去；copy 重申，引用一行\n子证明必须关闭最内层假设；不允许引用已关闭子证明内部的行。";

#[derive(Clone, Debug)]
pub struct ProofLine {
	pub formula: Formula,
	pub rule: String,
	pub references: Vec<usize>,
	pub scope: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct Proof {
	premises: Vec<Formula>,
	pub goal: Formula,
	lines: Vec<ProofLine>,
	open: Vec<usize>,
	commands: Vec<String>,
}

impl Proof {
	pub fn new(premises: Vec<Formula>, goal: Formula) -> Self {
		let lines: Vec<ProofLine> = premises
			.iter()
			.map(|formula| ProofLine {
				formula: formula.clone(),
				rule: "premise".into(),
				references: Vec::new(),
				scope: Vec::new(),
			})
			.collect();
		Self {
			premises,
			goal,
			lines,
			open: Vec::new(),
			commands: Vec::new(),
		}
	}

	pub fn lines(&self) -> &[ProofLine] {
		&self.lines
	}
	pub fn commands(&self) -> &[String] {
		&self.commands
	}
	pub fn open_assumptions(&self) -> &[usize] {
		&self.open
	}
	pub fn is_finished(&self) -> bool {
		self.open.is_empty()
			&& self
				.lines
				.iter()
				.any(|line| line.scope.is_empty() && line.formula == self.goal)
	}
	pub fn progress_key(&self) -> String {
		format!("proof\n{:?}\n{}", self.premises, self.goal)
	}

	pub fn replay(mut self, commands: &[String]) -> Result<Self, ParseError> {
		for command in commands {
			self.submit(command)?;
		}
		Ok(self)
	}

	pub fn undo(&mut self) -> bool {
		if self.commands.is_empty() {
			return false;
		}
		let commands: Vec<String> = self.commands[..self.commands.len() - 1].to_vec();
		*self = Self::new(self.premises.clone(), self.goal.clone())
			.replay(&commands)
			.expect("previously accepted proof replays");
		true
	}

	fn accessible(&self, number: usize) -> Result<&Formula, ParseError> {
		let line: &ProofLine = number
			.checked_sub(1)
			.and_then(|index| self.lines.get(index))
			.ok_or_else(|| ParseError(format!("第 {number} 行不存在；只能引用已有行。")))?;
		if !self.open.starts_with(&line.scope) {
			return Err(ParseError(format!(
				"第 {number} 行属于已关闭的子证明，当前不可引用。"
			)));
		}
		Ok(&line.formula)
	}

	pub fn submit(&mut self, command: &str) -> Result<String, ParseError> {
		if self.is_finished() {
			return Err(ParseError(
				"结论已在所有假设之外得到；证明已完成。可撤销或切换练习。".into(),
			));
		}
		if self.lines.len() >= 256 {
			return Err(ParseError("首版每份证明最多 256 行。".into()));
		}
		let parts: Vec<&str> = command.split(';').map(str::trim).collect();
		if !(2..=3).contains(&parts.len()) {
			return Err(ParseError(
				"格式：公式 ; 规则 ; 引用行。例如 B ; mp ; 3,5".into(),
			));
		}
		let formula: Formula = parse_formula(parts[0])?;
		let rule: &str = parts[1];
		let references: Vec<usize> = parts
			.get(2)
			.filter(|text| !text.is_empty())
			.map(|text| {
				text.split(',')
					.map(|value| {
						value
							.trim()
							.parse::<usize>()
							.map_err(|_| ParseError("引用行必须是以逗号分隔的正整数。".into()))
					})
					.collect()
			})
			.transpose()?
			.unwrap_or_default();
		let mut scope: Vec<usize> = self.open.clone();
		let explanation: &str = if rule == "assume" {
			if !references.is_empty() {
				return Err(ParseError("假设不引用其他行。".into()));
			}
			if scope.len() >= 16 {
				return Err(ParseError("子证明最多嵌套 16 层。".into()));
			}
			scope.push(self.lines.len() + 1);
			"已打开一个假设；子证明中的结论不能直接带到假设之外。"
		} else {
			let cited: Vec<&Formula> = references
				.iter()
				.map(|number| self.accessible(*number))
				.collect::<Result<_, _>>()?;
			let matches: bool = match (rule, cited.as_slice()) {
				("mp", [first, second]) => {
					implies(first, second, &formula) || implies(second, first, &formula)
				}
				("copy", [first]) => **first == formula,
				("and-intro", [left, right]) => {
					formula == Formula::binary(LogicOp::And, (*left).clone(), (*right).clone())
				}
				("and-left", [Formula::Binary(LogicOp::And, left, _)]) => **left == formula,
				("and-right", [Formula::Binary(LogicOp::And, _, right)]) => **right == formula,
				("or-left", [first]) => {
					matches!(&formula, Formula::Binary(LogicOp::Or, left, _) if **left == **first)
				}
				("or-right", [first]) => {
					matches!(&formula, Formula::Binary(LogicOp::Or, _, right) if **right == **first)
				}
				("not-elim", [first, second]) => {
					formula == Formula::Constant(false)
						&& (negates(first, second) || negates(second, first))
				}
				("bottom-elim", [first]) => **first == Formula::Constant(false),
				("iff-left", [Formula::Binary(LogicOp::Iff, left, right)]) => {
					formula == Formula::binary(LogicOp::Implies, *left.clone(), *right.clone())
				}
				("iff-right", [Formula::Binary(LogicOp::Iff, left, right)]) => {
					formula == Formula::binary(LogicOp::Implies, *right.clone(), *left.clone())
				}
				("iff-intro", [first, second]) => match &formula {
					Formula::Binary(LogicOp::Iff, left, right) => {
						(implies(first, left, right) && implies(second, right, left))
							|| (implies(second, left, right) && implies(first, right, left))
					}
					_ => false,
				},
				("imp-intro" | "not-intro" | "raa", [assumption, end]) => {
					if scope.last() != references.first()
						|| references[1] != self.lines.len()
						|| self.lines.last().is_none_or(|line| line.scope != scope)
					{
						return Err(ParseError(
							"必须引用当前最内层的假设行和该子证明的最后一行；不能跨层解除假设。"
								.into(),
						));
					}
					let valid: bool = match rule {
						"imp-intro" => {
							formula
								== Formula::binary(
									LogicOp::Implies,
									(*assumption).clone(),
									(*end).clone(),
								)
						}
						"not-intro" => {
							**end == Formula::Constant(false) && negates(&formula, assumption)
						}
						"raa" => **end == Formula::Constant(false) && negates(assumption, &formula),
						_ => unreachable!(),
					};
					if valid {
						scope.pop();
					}
					valid
				}
				_ => false,
			};
			if !matches {
				return Err(ParseError(format!(
					"这一步不符合 {rule} 的规则或引用数量。请检查公式的结构、引用行及规则说明；语义等价不等于应用了该规则。"
				)));
			}
			match rule {
				"mp" => "肯定前件：由 A→B 和 A 推出 B。不能由 B 反推 A。",
				"raa" => "反证法：假设 ¬A 后推出矛盾，解除该假设，得到 A（经典逻辑）。",
				"imp-intro" => "蕴涵引入：在假设 A 下得到 B，解除假设，得到 A→B。",
				"not-intro" => "否定引入：在假设 A 下得到矛盾，解除假设，得到 ¬A。",
				"not-elim" => "一个命题与它的否定同时成立，得到矛盾 ⊥。",
				_ => "公式、引用行和假设作用域均符合该规则。",
			}
		};
		self.lines.push(ProofLine {
			formula,
			rule: rule.into(),
			references,
			scope: scope.clone(),
		});
		self.open = scope;
		self.commands.push(command.into());
		Ok(format!(
			"正确。{explanation}{}",
			if self.is_finished() {
				" 目标已在所有假设之外成立，证明完成。"
			} else {
				""
			}
		))
	}
}

fn implies(formula: &Formula, premise: &Formula, conclusion: &Formula) -> bool {
	matches!(formula, Formula::Binary(LogicOp::Implies, left, right) if **left == *premise && **right == *conclusion)
}

fn negates(negative: &Formula, positive: &Formula) -> bool {
	matches!(negative, Formula::Not(child) if **child == *positive)
}
