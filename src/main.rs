use std::{
	collections::BTreeMap,
	error::Error,
	fs,
	path::{Path, PathBuf},
	process::ExitCode,
};

use clap::{ArgGroup, Parser};
use stepwise::{
	app::{self, Course, Lesson},
	core::{EvaluationMode, ExprKind, Language, Session},
	exercises::{self, Exercise, ProofQuestion, Question, QuestionSet},
	generate,
	logic::{Formula, parse_formula, proof::Proof},
	progress::Progress,
	tui,
};

/// What no written-out proof and no proof check combines with. clap waives a missing
/// requirement when the missing argument conflicts with one already given, so `--premise` and
/// `--check-proof` repeat these conflicts rather than relying on the `--goal` and
/// `--proof` they require: `--python --premise P` has to be refused, not quietly dropped.
const PROOF_ONLY: [&str; 9] = [
	"python",
	"expression",
	"exercise",
	"list",
	"trace",
	"evaluation",
	"random",
	"assign",
	"equivalent",
];

#[derive(Parser, Debug)]
#[command(
	version,
	group(ArgGroup::new("language").required(true).args(["python", "logic"])),
	group(ArgGroup::new("proof_source").args(["proof", "goal"])),
	about = "选择下一步，理解求值与推理。支持 Python、命题逻辑和自然演绎。"
)]
struct Args {
	/// 自定义表达式或命题公式（用引号包围）
	expression: Option<String>,
	/// Python 表达式练习
	#[arg(long)]
	python: bool,
	/// 命题逻辑求值模式
	#[arg(long)]
	logic: bool,
	/// 变量或命题赋值，可重复传入：--assign x=3 --assign flag=False
	#[arg(long, value_parser = assignment, conflicts_with_all = ["proof", "goal", "list"])]
	assign: Vec<(String, String)>,
	/// 用 BDD 检查两个公式在所有赋值下是否等价，不进入练习
	#[arg(long, requires_all = ["logic", "expression"], conflicts_with_all = ["trace", "exercise", "list", "evaluation", "assign"])]
	equivalent: Option<String>,
	/// 按名称打开当前题集里的一道证明题，与 --exercise 查同一份题集；--list 列出可用名称
	#[arg(long, value_name = "NAME", requires = "logic", conflicts_with_all = ["python", "expression", "exercise", "list", "trace", "evaluation", "goal", "premise"])]
	proof: Option<String>,
	/// 自定义证明的结论，不属于任何题集；前提用 --premise 给出
	#[arg(long, requires = "logic", conflicts_with_all = PROOF_ONLY)]
	goal: Option<String>,
	/// 自定义证明前提，可重复传入；没有前提也可证明
	#[arg(long, requires = "goal", conflicts_with_all = PROOF_ONLY)]
	premise: Vec<String>,
	/// 检查 JSON 字符串数组中的证明步骤，不启动 TUI、不读写进度；证明来自 --proof 或 --goal
	#[arg(long, value_name = "FILE", requires = "proof_source", conflicts_with_all = PROOF_ONLY)]
	check_proof: Option<PathBuf>,
	/// 从当前题集选择题目 ID：默认内置题集，或 --set 指定的文件
	#[arg(long, conflicts_with = "expression")]
	exercise: Option<String>,
	/// 外部 TOML 题集文件；按文件里的顺序练习，取代内置题集
	#[arg(
		long,
		value_name = "FILE",
		conflicts_with_all = [
			"expression",
			"random",
			"seed",
			"goal",
			"premise",
			"equivalent"
		]
	)]
	set: Option<PathBuf>,
	/// 开始新的随机题，跳过上次进度；不指定题目时默认随机出题
	#[arg(long, conflicts_with_all = ["expression", "exercise", "list", "assign", "proof", "goal", "equivalent"])]
	random: bool,
	/// 固定随机种子，复现同一道题
	#[arg(long, requires = "random")]
	seed: Option<u64>,
	#[arg(long)]
	list: bool,
	/// 输出逐步演示，不启动 TUI、不读写进度
	#[arg(long)]
	trace: bool,
	/// 短路开关；关闭时 Python 模式为教学变体
	#[arg(long, value_parser = ["short-circuit", "eager"])]
	evaluation: Option<String>,
	#[arg(long)]
	no_save: bool,
	#[arg(long)]
	progress_file: Option<PathBuf>,
}

fn assignment(input: &str) -> Result<(String, String), String> {
	let (name, value): (&str, &str) = input.split_once('=').ok_or("赋值格式为 x=3 或 P=true")?;
	if name.trim().is_empty() || value.trim().is_empty() {
		return Err("变量名称和值不能为空。".into());
	}
	Ok((name.trim().into(), value.trim().into()))
}

/// Reading a file belongs to the front end: the library takes text and hands back a checked
/// set, and this is the one place where a path becomes part of the message.
fn load_set(path: &Path) -> Result<QuestionSet, Box<dyn Error>> {
	let text: String = fs::read_to_string(path)
		.map_err(|error| format!("无法读取题集 {}：{error}", path.display()))?;
	QuestionSet::import(&text).map_err(|error| -> Box<dyn Error> {
		format!("题集 {}：{error}", path.display()).into()
	})
}

/// The required language selects inside the loaded set; saying so beats an empty screen.
fn no_questions(set: &QuestionSet, language: Language) -> String {
	format!(
		"题集 {} 里没有 --{} 的题目；用 --list 查看题集内容。",
		set.name,
		language.key()
	)
}

/// The question a name means in the loaded set, for the language this launch practises:
/// the one lookup behind both `--exercise` and `--proof`, or why the name means nothing here —
/// absent from the set, or present in the other language.
fn named<'a>(
	set: &'a QuestionSet,
	name: &str,
	language: Language,
) -> Result<&'a Question, Box<dyn Error>> {
	let question: &Question = set.find(name).ok_or_else(|| {
		format!(
			"题集 {} 里没有题目 {name}；用 --{} --list 查看可用 ID。",
			set.name,
			language.key()
		)
	})?;
	if question.language() == language {
		return Ok(question);
	}
	Err(match question {
		Question::Proof(_) => format!("题目 {name} 是证明题，请改用 --logic。"),
		Question::Evaluation(_) => format!(
			"题目 {name} 是 --{} 的题目，当前是 --{}。",
			question.language().key(),
			language.key()
		),
	}
	.into())
}

/// The proof this launch opens, if it opens one: a sequent written out with `--goal`, which
/// belongs to no set, or a proof question the set names. `--proof` insists on a proof;
/// `--exercise` opens whichever kind the name turns out to be.
fn proof_question(
	args: &Args,
	named: Option<&Question>,
) -> Result<Option<ProofQuestion>, Box<dyn Error>> {
	if let Some(goal) = &args.goal {
		return Ok(Some(ProofQuestion {
			set: String::new(),
			name: "custom-proof".into(),
			title: "自定义证明".into(),
			premises: args.premise.clone(),
			conclusion: goal.clone(),
			note: None,
		}));
	}
	match named {
		Some(Question::Proof(question)) => Ok(Some(question.clone())),
		Some(Question::Evaluation(exercise)) if args.proof.is_some() => Err(format!(
			"题目 {0} 是求值题，不是证明题；用 --exercise {0} 打开它。",
			exercise.name
		)
		.into()),
		Some(Question::Evaluation(_)) | None => Ok(None),
	}
}

fn trace(mut session: Session) -> Result<(), Box<dyn Error>> {
	println!("{}\n{}", session.language().label(), session.render());
	while let Some(step) = session.next_step() {
		let input: String = match step.outcome {
			Ok(value) => value.to_string(),
			Err(error) => error.name().ok_or_else(|| error.to_string())?.into(),
		};
		let (source, ranges) = session.render_with_ranges();
		let selected: String = source[ranges[&step.node_id].clone()].into();
		let group: bool = matches!(
			session.root().find(step.node_id).map(|node| &node.kind),
			Some(ExprKind::Group(_))
		);
		let feedback: stepwise::core::Feedback = if group {
			session.remove_group(step.node_id)
		} else {
			session.submit(step.node_id, &input)
		};
		if !feedback.accepted() {
			return Err(feedback.message.into());
		}
		let action: String = if group {
			format!("去掉「{selected}」的一层括号")
		} else {
			format!("将「{selected}」替换为 {input}")
		};
		println!(
			"{}. {action}\n   {}",
			session.history().len(),
			feedback.message
		);
	}
	println!(
		"完成：{}",
		session
			.terminal_error()
			.map(ToString::to_string)
			.unwrap_or_else(|| session.render().into())
	);
	Ok(())
}

fn execute(args: Args) -> Result<(), Box<dyn Error>> {
	let language: Language = match (args.python, args.logic) {
		(true, false) => Language::Python,
		(false, true) => Language::Logic,
		_ => unreachable!("clap requires exactly one language"),
	};
	if let Some(other) = &args.equivalent {
		let left: Formula = parse_formula(
			args.expression
				.as_deref()
				.expect("clap requires expression"),
		)?;
		let right: Formula = parse_formula(other)?;
		println!(
			"{}",
			if left.equivalent(&right)? {
				"等价：所有赋值下真值相同。"
			} else {
				"不等价：存在使两者真值不同的赋值。"
			}
		);
		return Ok(());
	}
	// One load path for every question: the embedded set is its default value, and --set
	// replaces it with a file that went through exactly the same parsing and checking. The
	// built-in proofs live there too, so --proof and --check-proof read the same set.
	let set: QuestionSet = match &args.set {
		Some(path) => load_set(path)?,
		None => exercises::builtin()?,
	};
	if args.list {
		let mut listed: usize = 0;
		for question in set
			.questions()
			.iter()
			.filter(|question| question.language() == language)
		{
			let (source, extra): (String, String) = match question {
				Question::Evaluation(exercise) => {
					(exercise.expression.clone(), exercise.assignments())
				}
				Question::Proof(proof) => (proof.sequent(), String::new()),
			};
			println!(
				"{}\t{}\t{source}\t{extra}",
				question.name(),
				question.title()
			);
			listed += 1;
		}
		if listed == 0 {
			eprintln!("{}", no_questions(&set, language));
		}
		return Ok(());
	}
	let chosen: Option<&Question> = args
		.proof
		.as_deref()
		.or(args.exercise.as_deref())
		.map(|name| named(&set, name, language))
		.transpose()?;
	let proof: Option<ProofQuestion> = proof_question(&args, chosen)?;
	if let Some(path) = &args.check_proof {
		let commands: Vec<String> = serde_json::from_reader(fs::File::open(path)?)?;
		let mut checked: Proof = proof.expect("clap requires --proof or --goal").proof()?;
		for command in commands {
			println!("{command}\n{}", checked.submit(&command)?);
		}
		if !checked.is_finished() {
			return Err("步骤合法，但尚未在所有假设之外得到目标。".into());
		}
		return Ok(());
	}
	let path: Option<PathBuf> = if args.no_save || args.trace {
		None
	} else {
		Some(
			args.progress_file
				.clone()
				.map_or_else(Progress::default_path, Ok)?,
		)
	};
	let progress: Progress = path
		.as_deref()
		.map(Progress::load)
		.transpose()?
		.unwrap_or_default();
	let requested: Option<EvaluationMode> = match args.evaluation.as_deref() {
		Some("eager") => Some(EvaluationMode::Eager),
		Some("short-circuit") => Some(EvaluationMode::ShortCircuit),
		_ => None,
	};
	// --set makes the file the course: its questions of this language, evaluation and proof
	// alike, in its order, ending at the last one.
	let ordered: bool = args.set.is_some();
	let mut questions: Vec<Question> = set.of_language(language);
	let index: usize = if args.random {
		questions = vec![Question::Evaluation(generate::generate(
			language,
			args.seed.unwrap_or_else(generate::fresh_seed),
		)?)];
		0
	} else if let Some(expression) = args.expression {
		questions = vec![Question::Evaluation(Exercise {
			set: String::new(),
			name: if args.logic {
				"custom-logic"
			} else {
				"custom-python"
			}
			.into(),
			title: "自定义练习".into(),
			language,
			expression,
			bindings: BTreeMap::new(),
			evaluation: None,
			note: None,
		})];
		0
	} else if args.goal.is_some() {
		questions = vec![Question::Proof(proof.expect("--goal writes out a proof"))];
		0
	} else if let Some(question) = chosen {
		// `named` already found it among this language's questions.
		questions
			.iter()
			.position(|candidate| candidate.name() == question.name())
			.expect("a named question of this language is in its list")
	} else if !args.assign.is_empty() {
		// Assignments with no question named bind an evaluation question — a proof has
		// nothing to assign — so the set is searched among those alone: in order from where
		// progress left off with --set, else its current one or its first.
		let evaluations: Vec<usize> = (0..questions.len())
			.filter(|index| questions[*index].evaluation().is_some())
			.collect();
		if evaluations.is_empty() {
			return Err(format!(
				"题集 {} 里 --{} 没有求值题，--assign 没有可赋值的题目。",
				set.name,
				language.key()
			)
			.into());
		}
		let candidates: Vec<Question> = evaluations
			.iter()
			.map(|index| questions[*index].clone())
			.collect();
		evaluations[if ordered {
			app::resume_in_set(&progress, &candidates, requested)?
		} else {
			candidates
				.iter()
				.position(|question| progress.points_at(question.set(), question.name()))
				.unwrap_or(0)
		}]
	} else if ordered {
		if questions.is_empty() {
			return Err(no_questions(&set, language).into());
		}
		app::resume_in_set(&progress, &questions, requested)?
	} else {
		questions = vec![app::resume_or_generate(language, &progress, &questions)?];
		0
	};
	let mut assigned: BTreeMap<String, String> = BTreeMap::new();
	for (name, value) in args.assign {
		if assigned.insert(name.clone(), value).is_some() {
			return Err(format!("变量 {name} 被重复赋值。").into());
		}
	}
	match &mut questions[index] {
		Question::Evaluation(exercise) => exercise.bindings.extend(assigned),
		// Accepting assignments here would drop them: a proof binds no variable.
		Question::Proof(question) if !assigned.is_empty() => {
			return Err(format!("题目 {} 是证明题，--assign 对它没有意义。", question.name).into());
		}
		Question::Proof(_) => {}
	}
	// --evaluation is the course's strategy, so it stands even when the course opens on a
	// proof: the evaluation questions after it open in it.
	let mode: EvaluationMode = app::starting_mode(requested, &progress, &questions[index]);
	if args.trace {
		let Question::Evaluation(exercise) = &questions[index] else {
			return Err(format!(
				"证明题没有逐步演示；写好的证明用 --proof {} --check-proof FILE 检查。",
				questions[index].name()
			)
			.into());
		};
		if !exercise.bindings.is_empty() {
			println!("{}", exercise.assignments());
		}
		return trace(exercise.session(mode)?);
	}
	// The chosen question starts a practice sequence; the course supplies the rest.
	let course: Course = if ordered {
		Course::ordered(questions, index)?
	} else {
		Course::random(vec![questions.remove(index)], 0)?
	};
	tui::run(Lesson::new(course, progress, mode)?, path)
}

fn main() -> ExitCode {
	match execute(Args::parse()) {
		Ok(()) => ExitCode::SUCCESS,
		Err(error) => {
			eprintln!("Stepwise: {error}");
			ExitCode::FAILURE
		}
	}
}
