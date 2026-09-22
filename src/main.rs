use std::{collections::BTreeMap, error::Error, path::PathBuf, process::ExitCode};

use clap::{ArgGroup, Parser};
use stepwise::{
	app::{self, Course, Practice},
	core::{EvaluationMode, ExprKind, Language, Session},
	exercises::{self, Exercise},
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
	/// 从所选模式的内嵌题库选择题目 ID
	#[arg(long, conflicts_with = "expression")]
	exercise: Option<String>,
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
	let mut exercises: Vec<Exercise> = exercises::builtin()?
		.into_iter()
		.filter(|exercise| exercise.language == language)
		.collect();
	if args.list {
		for exercise in &exercises {
			println!(
				"{}\t{}\t{}\t{}",
				exercise.id,
				exercise.title,
				exercise.expression,
				exercise.assignments()
			);
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
	if args.random {
		exercises = vec![generate::generate(
			language,
			args.seed.unwrap_or_else(generate::fresh_seed),
		)?];
	} else if args.expression.is_none() && args.exercise.is_none() && args.assign.is_empty() {
		exercises = vec![app::resume_or_generate(language, &progress, &exercises)?];
	}
	if let Some(expression) = args.expression {
		exercises = vec![Exercise {
			id: if args.logic {
				"custom-logic"
			} else {
				"custom-python"
			}
			.into(),
			title: "自定义练习".into(),
			expression,
			goal: "同名变量一起代入，同优先级的独立子式任选先后。".into(),
			language,
			bindings: BTreeMap::new(),
		}];
	}
	let index: usize = if let Some(id) = args.exercise {
		exercises
			.iter()
			.position(|exercise| exercise.id == id)
			.ok_or_else(|| {
				format!("没有题目 {id}；使用 --python --list 或 --logic --list 查看可用 ID。")
			})?
	} else {
		exercises
			.iter()
			.position(|exercise| exercise.id == progress.current)
			.unwrap_or(0)
	};
	let mut assigned: BTreeMap<String, String> = BTreeMap::new();
	for (name, value) in args.assign {
		if assigned.insert(name.clone(), value).is_some() {
			return Err(format!("变量 {name} 被重复赋值。").into());
		}
	}
	exercises[index].bindings.extend(assigned);
	let requested: Option<EvaluationMode> = match args.evaluation.as_deref() {
		Some("eager") => Some(EvaluationMode::Eager),
		Some("short-circuit") => Some(EvaluationMode::ShortCircuit),
		_ => None,
	};
	let mode: EvaluationMode = app::starting_mode(requested, &progress, &exercises[index]);
	if args.trace {
		if !exercises[index].bindings.is_empty() {
			println!("{}", exercises[index].assignments());
		}
		return trace(exercises[index].session(mode)?);
	}
	// The chosen/default question starts a practice sequence; the course supplies the rest.
	let course: Course = Course::random(vec![exercises.remove(index)], 0)?;
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
