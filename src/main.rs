use std::{
	collections::BTreeMap,
	error::Error,
	fs,
	path::{Path, PathBuf},
	process::ExitCode,
};

use clap::{ArgGroup, Parser};
use stepwise::{
	app::{self, Course, Practice},
	core::{EvaluationMode, ExprKind, Language, Session},
	exercises::{self, Exercise, Question, QuestionSet},
	generate,
	logic::{Formula, parse_formula, proof::Proof},
	progress::Progress,
	tui,
};

#[derive(Parser, Debug)]
#[command(
	version,
	group(ArgGroup::new("language").required(true).args(["python", "logic"])),
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
	#[arg(long, value_parser = assignment, conflicts_with_all = ["proof", "list"])]
	assign: Vec<(String, String)>,
	/// 用 BDD 检查两个公式在所有赋值下是否等价，不进入练习
	#[arg(long, requires_all = ["logic", "expression"], conflicts_with_all = ["trace", "exercise", "list", "evaluation", "assign"])]
	equivalent: Option<String>,
	/// 自然演绎练习：mp 肯定前件，raa 反证法，identity 蕴涵引入
	#[arg(long, requires = "logic", value_parser = ["mp", "raa", "identity"], conflicts_with_all = ["python", "expression", "exercise", "list", "trace", "evaluation"])]
	proof: Option<String>,
	/// 自定义证明目标，与 --premise 配合使用
	#[arg(long, requires = "proof")]
	goal: Option<String>,
	/// 自定义证明前提，可重复传入；没有前提也可证明
	#[arg(long, requires = "goal")]
	premise: Vec<String>,
	/// 检查 JSON 字符串数组中的证明步骤，不启动 TUI、不读写进度
	#[arg(long, requires = "proof")]
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
			"proof",
			"goal",
			"premise",
			"check_proof",
			"equivalent"
		]
	)]
	set: Option<PathBuf>,
	/// 开始新的随机题，跳过上次进度；不指定题目时默认随机出题
	#[arg(long, conflicts_with_all = ["expression", "exercise", "list", "assign", "proof", "equivalent"])]
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
	let proofs: bool = set
		.questions()
		.iter()
		.any(|question| question.language() == language && question.evaluation().is_none());
	if proofs {
		format!(
			"题集 {} 里 --{} 只有证明题；用 --exercise ID 选一道，或用 --list 查看。",
			set.name,
			language.key()
		)
	} else {
		format!(
			"题集 {} 里没有 --{} 的题目；用 --list 查看题集内容。",
			set.name,
			language.key()
		)
	}
}

/// Where a named question sits among the ones this language can practise, or why it is not
/// there: absent from the set, or present in the other language.
fn choose(
	set: &QuestionSet,
	questions: &[Exercise],
	id: &str,
	language: Language,
) -> Result<usize, Box<dyn Error>> {
	if let Some(index) = questions.iter().position(|exercise| exercise.name == id) {
		return Ok(index);
	}
	Err(match set.find(id) {
		// A proof question was already opened above, so a match here differs by language.
		Some(question) => format!(
			"题目 {id} 是 --{} 的题目，当前是 --{}。",
			question.language().key(),
			language.key()
		),
		None => format!(
			"题集 {} 里没有题目 {id}；用 --{} --list 查看可用 ID。",
			set.name,
			language.key()
		),
	}
	.into())
}

fn proof_for(args: &Args, name: &str) -> Result<Proof, Box<dyn Error>> {
	let (premises, goal): (Vec<String>, String) = if let Some(goal) = &args.goal {
		(args.premise.clone(), goal.clone())
	} else {
		match name {
			"mp" => (vec!["P -> Q".into(), "P".into()], "Q".into()),
			"raa" => (vec!["~~P".into()], "P".into()),
			_ => (Vec::new(), "P -> P".into()),
		}
	};
	Ok(Proof::new(
		premises
			.iter()
			.map(|source| parse_formula(source))
			.collect::<Result<_, _>>()?,
		parse_formula(&goal)?,
	))
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
	if let (Some(name), Some(path)) = (&args.proof, &args.check_proof) {
		let commands: Vec<String> = serde_json::from_reader(std::fs::File::open(path)?)?;
		let mut proof: Proof = proof_for(&args, name)?;
		for command in commands {
			println!("{command}\n{}", proof.submit(&command)?);
		}
		if !proof.is_finished() {
			return Err("步骤合法，但尚未在所有假设之外得到目标。".into());
		}
		return Ok(());
	}
	// One load path for every question: the embedded set is its default value, and --set
	// replaces it with a file that went through exactly the same parsing and checking.
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
	if let Some(name) = &args.proof {
		return tui::run_proof(proof_for(&args, name)?, progress, path);
	}
	let requested: Option<EvaluationMode> = match args.evaluation.as_deref() {
		Some("eager") => Some(EvaluationMode::Eager),
		Some("short-circuit") => Some(EvaluationMode::ShortCircuit),
		_ => None,
	};
	// A set's proof question opens the same practice, checked by the same rules.
	if let Some(Question::Proof(question)) = args.exercise.as_deref().and_then(|id| set.find(id)) {
		if language != Language::Logic {
			return Err(format!("题目 {} 是证明题，请改用 --logic。", question.name).into());
		}
		if args.trace {
			return Err("证明题没有逐步演示；用 --check-proof 检查已保存的步骤。".into());
		}
		if let Some(flag) = [
			(!args.assign.is_empty()).then_some("--assign"),
			args.evaluation.is_some().then_some("--evaluation"),
		]
		.into_iter()
		.flatten()
		.next()
		{
			return Err(format!("题目 {} 是证明题，{flag} 对它没有意义。", question.name).into());
		}
		return tui::run_proof(question.proof()?, progress, path);
	}
	// --set makes the file the course: its questions, in its order, ending at the last one.
	let ordered: bool = args.set.is_some();
	let mut questions: Vec<Exercise> = set.evaluations(language);
	let index: usize = if args.random {
		questions = vec![generate::generate(
			language,
			args.seed.unwrap_or_else(generate::fresh_seed),
		)?];
		0
	} else if let Some(expression) = args.expression {
		questions = vec![Exercise {
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
		}];
		0
	} else if let Some(id) = &args.exercise {
		choose(&set, &questions, id, language)?
	} else if ordered {
		if questions.is_empty() {
			return Err(no_questions(&set, language).into());
		}
		app::resume_in_set(&progress, &questions, requested)?
	} else if !args.assign.is_empty() {
		// Assignments with no question named stay on the set's current question, as before.
		if questions.is_empty() {
			return Err(no_questions(&set, language).into());
		}
		questions
			.iter()
			.position(|exercise| progress.points_at(&exercise.set, &exercise.name))
			.unwrap_or(0)
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
	questions[index].bindings.extend(assigned);
	let mode: EvaluationMode = app::starting_mode(requested, &progress, &questions[index]);
	if args.trace {
		if !questions[index].bindings.is_empty() {
			println!("{}", questions[index].assignments());
		}
		return trace(questions[index].session(mode)?);
	}
	// The chosen question starts a practice sequence; the course supplies the rest.
	let course: Course = if ordered {
		Course::ordered(questions, index)?
	} else {
		Course::random(vec![questions.remove(index)], 0)?
	};
	tui::run(Practice::new(course, progress, mode)?, path)
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
