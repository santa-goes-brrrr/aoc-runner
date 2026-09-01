use aoc_runtime::{
    INPUT_ADDRESS, INPUT_CAPACITY, INPUT_HEADER_SIZE, INPUT_MAGIC, OUTPUT_ADDRESS, OUTPUT_MAGIC,
    OUTPUT_MAILBOX_SIZE, PROTOCOL_VERSION, RAM_BASE, RAM_SIZE, STATE_OK,
};
use clap::{Parser, Subcommand};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use libsql::{Builder, Connection};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::task::JoinSet;
use tokio::time::{MissedTickBehavior, interval, sleep};

const TARGET: &str = "riscv64gc-unknown-none-elf";
const HOST_TIMEOUT: Duration = Duration::from_secs(10);
const PROGRESS_POLL_INTERVAL: Duration = Duration::from_millis(25);
const INSTRUCTION_BUDGET: u64 = 2_000_000_000;
const PROGRESS_INITIAL_THRESHOLD: u64 = 100_000;
const PROGRESS_MAGIC: &[u8; 8] = b"AOCPROG1";
const PROGRESS_VERSION: u32 = 1;
const PROGRESS_MAILBOX_SIZE: u64 = 24;
const PROGRESS_BUDGET_EXCEEDED: u32 = 3;
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

static RUN_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Parser)]
#[command(name = "cli")]
#[command(about = "Build, evaluate, and manage Advent of Code solutions")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage solution inputs and expected-answer hashes
    Solution {
        #[command(subcommand)]
        command: SolutionCommands,
    },
    /// Build and evaluate solutions in the RV64GCV evaluator
    Evaluate {
        #[command(subcommand)]
        command: EvaluateCommands,
    },
}

#[derive(Subcommand)]
enum EvaluateCommands {
    /// Run a single solution and verify its answer
    One {
        #[arg(short, long)]
        year: u16,
        #[arg(short, long)]
        day: u8,
        #[arg(short, long)]
        part: u8,
    },
    /// Run every solution and verify its answer
    All {
        /// Maximum number of concurrent QEMU workers
        #[arg(short, long)]
        jobs: Option<usize>,
    },
}

#[derive(Subcommand)]
enum SolutionCommands {
    /// Initialize the database
    Init,
    /// Delete the solutions table
    Reset,
    /// List all solutions
    List,
    /// Create a solution entry
    Create {
        #[arg(short, long)]
        year: u16,
        #[arg(short, long)]
        day: u8,
        #[arg(short, long)]
        part: u8,
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        solution: String,
    },
    /// Update an existing solution entry
    Update {
        #[command(subcommand)]
        command: UpdateCommands,
    },
    /// Delete an entry
    Delete {
        #[arg(short, long)]
        year: u16,
        #[arg(short, long)]
        day: u8,
        #[arg(short, long)]
        part: u8,
    },
    /// Read data from the database
    Read {
        #[command(subcommand)]
        command: ReadCommands,
    },
}

#[derive(Subcommand)]
enum UpdateCommands {
    /// Replace the stored puzzle input from a file
    Input {
        #[arg(short, long)]
        year: u16,
        #[arg(short, long)]
        day: u8,
        #[arg(short, long)]
        part: u8,
        #[arg(short, long)]
        input: PathBuf,
    },
    /// Replace the stored expected-answer hash
    Hash {
        #[arg(short, long)]
        year: u16,
        #[arg(short, long)]
        day: u8,
        #[arg(short, long)]
        part: u8,
        #[arg(short, long)]
        solution: String,
    },
}

#[derive(Subcommand)]
enum ReadCommands {
    /// Read the stored answer hash
    Hash {
        #[arg(short, long)]
        year: u16,
        #[arg(short, long)]
        day: u8,
        #[arg(short, long)]
        part: u8,
    },
    Input {
        #[arg(short, long)]
        year: u16,
        #[arg(short, long)]
        day: u8,
        #[arg(short, long)]
        part: u8,
    },
}

struct RunResult {
    year: u16,
    day: u8,
    part: u8,
    passed: bool,
    instructions: Option<u64>,
    error: Option<String>,
}

struct SolutionCase {
    year: u16,
    day: u8,
    part: u8,
    input: String,
    expected_hash: String,
    binary: PathBuf,
    label: String,
}

struct BuildReport {
    artifacts: HashSet<String>,
    errors: HashMap<String, String>,
    fallback_error: String,
}

struct Evaluation {
    actual: String,
    instructions: u64,
}

struct RunSummary {
    passed: usize,
    errors: usize,
    timeouts: usize,
    budget_exceeded: usize,
    total_instructions: u128,
}

struct ProgressSnapshot {
    state: u32,
    instructions: u64,
}

struct PreflightIssue {
    dependency: &'static str,
    detail: String,
    guidance: &'static str,
}

struct PreflightError(String);

impl std::fmt::Debug for PreflightError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::fmt::Display for PreflightError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PreflightError {}

struct RunDirectory(PathBuf);

impl RunDirectory {
    fn create(label: &str) -> io::Result<Self> {
        let id = RUN_ID.fetch_add(1, Ordering::Relaxed);
        let path = PathBuf::from("target/aoc-evaluator/runs")
            .join(format!("{label}-{}-{id}", std::process::id()));
        fs::create_dir_all(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for RunDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    if matches!(&cli.command, Commands::Evaluate { .. }) {
        print_progress("Checking host evaluator dependencies...");
        evaluator_preflight()?;
        print_progress("Host evaluator dependencies are available.");
    }

    let database = Builder::new_local("aoc.db").build().await?;
    let connection = database.connect()?;
    create_schema(&connection).await?;

    match cli.command {
        Commands::Solution { command } => run_solution_command(&connection, command).await?,
        Commands::Evaluate { command } => match command {
            EvaluateCommands::One { year, day, part } => {
                let result = run_solution(&connection, year, day, part).await?;
                if !result.passed {
                    std::process::exit(1);
                }
            }
            EvaluateCommands::All { jobs } => {
                let summary = run_all_solutions(&connection, job_count(jobs)).await?;
                println!(
                    "\nRun completed in {} instructions\n{} passed, {} errors, {} timeouts, {} over budget",
                    summary.total_instructions,
                    summary.passed,
                    summary.errors,
                    summary.timeouts,
                    summary.budget_exceeded
                );
                if summary.errors > 0 || summary.timeouts > 0 || summary.budget_exceeded > 0 {
                    std::process::exit(1);
                }
            }
        },
    }

    Ok(())
}

async fn run_solution_command(
    connection: &Connection,
    command: SolutionCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        SolutionCommands::Init => println!("Database initialized"),
        SolutionCommands::Reset => {
            reset_db(connection).await?;
            create_schema(connection).await?;
            println!("Database reset");
        }
        SolutionCommands::List => list_solutions(connection).await?,
        SolutionCommands::Create {
            year,
            day,
            part,
            input,
            solution,
        } => {
            let input_contents = read_input_file(&input)?;
            if !create_solution(connection, year, day, part, &input_contents, &solution).await? {
                return Err(
                    format!("Entry already exists: year={year}, day={day}, part={part}").into(),
                );
            }
            println!("Created: year={year}, day={day}, part={part}");
        }
        SolutionCommands::Update { command } => match command {
            UpdateCommands::Input {
                year,
                day,
                part,
                input,
            } => {
                let input_contents = read_input_file(&input)?;
                if !update_input(connection, year, day, part, &input_contents).await? {
                    return Err(format!(
                        "Entry does not exist: year={year}, day={day}, part={part}"
                    )
                    .into());
                }
                println!("Updated input: year={year}, day={day}, part={part}");
            }
            UpdateCommands::Hash {
                year,
                day,
                part,
                solution,
            } => {
                if !update_answer_hash(connection, year, day, part, &solution).await? {
                    return Err(format!(
                        "Entry does not exist: year={year}, day={day}, part={part}"
                    )
                    .into());
                }
                println!("Updated hash: year={year}, day={day}, part={part}");
            }
        },
        SolutionCommands::Delete { year, day, part } => {
            delete_solution(connection, year, day, part).await?;
            println!("Deleted: year={year}, day={day}, part={part}");
        }
        SolutionCommands::Read { command } => match command {
            ReadCommands::Hash { year, day, part } => {
                print!("{}", read_answer_hash(connection, year, day, part).await?);
            }
            ReadCommands::Input { year, day, part } => {
                print!("{}", read_input(connection, year, day, part).await?);
            }
        },
    }
    Ok(())
}

async fn create_schema(connection: &Connection) -> Result<(), libsql::Error> {
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS solutions (
                year INTEGER NOT NULL,
                day INTEGER NOT NULL,
                part INTEGER NOT NULL,
                input TEXT NOT NULL,
                answer_hash TEXT NOT NULL,
                PRIMARY KEY (year, day, part)
            )",
            (),
        )
        .await?;
    Ok(())
}

async fn reset_db(connection: &Connection) -> Result<(), libsql::Error> {
    connection
        .execute("DROP TABLE IF EXISTS solutions", ())
        .await?;
    Ok(())
}

fn read_input_file(path: &Path) -> io::Result<String> {
    fs::read_to_string(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("Failed to read input file {}: {error}", path.display()),
        )
    })
}

async fn create_solution(
    connection: &Connection,
    year: u16,
    day: u8,
    part: u8,
    input: &str,
    solution: &str,
) -> Result<bool, libsql::Error> {
    let answer_hash = answer_hash(year, day, part, solution);
    let changed = connection
        .execute(
            "INSERT OR IGNORE INTO solutions (year, day, part, input, answer_hash)
             VALUES (?, ?, ?, ?, ?)",
            (year, day, part, input, answer_hash),
        )
        .await?;
    Ok(changed == 1)
}

async fn update_input(
    connection: &Connection,
    year: u16,
    day: u8,
    part: u8,
    input: &str,
) -> Result<bool, libsql::Error> {
    let changed = connection
        .execute(
            "UPDATE solutions SET input = ? WHERE year = ? AND day = ? AND part = ?",
            (input, year, day, part),
        )
        .await?;
    Ok(changed == 1)
}

async fn update_answer_hash(
    connection: &Connection,
    year: u16,
    day: u8,
    part: u8,
    solution: &str,
) -> Result<bool, libsql::Error> {
    let answer_hash = answer_hash(year, day, part, solution);
    let changed = connection
        .execute(
            "UPDATE solutions SET answer_hash = ? WHERE year = ? AND day = ? AND part = ?",
            (answer_hash, year, day, part),
        )
        .await?;
    Ok(changed == 1)
}

async fn read_answer_hash(
    connection: &Connection,
    year: u16,
    day: u8,
    part: u8,
) -> Result<String, Box<dyn std::error::Error>> {
    let hash = read_column(connection, year, day, part, "answer_hash").await?;
    let digest = hash.strip_prefix("sha256:").unwrap_or_default();
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("Invalid answer hash for year={year}, day={day}, part={part}").into());
    }
    Ok(hash)
}

async fn read_input(
    connection: &Connection,
    year: u16,
    day: u8,
    part: u8,
) -> Result<String, Box<dyn std::error::Error>> {
    read_column(connection, year, day, part, "input").await
}

async fn read_column(
    connection: &Connection,
    year: u16,
    day: u8,
    part: u8,
    column: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let query = format!("SELECT {column} FROM solutions WHERE year = ? AND day = ? AND part = ?");
    let mut rows = connection.query(&query, (year, day, part)).await?;
    if let Some(row) = rows.next().await? {
        Ok(row.get::<String>(0)?)
    } else {
        Err(format!("No entry found for year={year}, day={day}, part={part}").into())
    }
}

async fn delete_solution(
    connection: &Connection,
    year: u16,
    day: u8,
    part: u8,
) -> Result<(), libsql::Error> {
    connection
        .execute(
            "DELETE FROM solutions WHERE year = ? AND day = ? AND part = ?",
            (year, day, part),
        )
        .await?;
    Ok(())
}

async fn list_solutions(connection: &Connection) -> Result<(), libsql::Error> {
    let mut rows = connection
        .query(
            "SELECT year, day, part, answer_hash FROM solutions ORDER BY year, day, part",
            (),
        )
        .await?;
    while let Some(row) = rows.next().await? {
        let year = row.get::<u32>(0)? as u16;
        let day = row.get::<u32>(1)? as u8;
        let part = row.get::<u32>(2)? as u8;
        let answer_hash = row.get::<String>(3)?;
        println!("{year}\t{day}\t{part}\t{answer_hash}");
    }
    Ok(())
}

fn print_progress(message: &str) {
    println!("{message}");
    let _ = io::stdout().flush();
}

fn print_run_result(result: &RunResult) {
    let label = format!("{}-{:02}-{}", result.year, result.day, result.part);
    if result.passed {
        println!(
            "{GREEN}✓ {label} Correct after {} instructions{RESET}",
            result.instructions.unwrap_or(0)
        );
    } else if let Some(error) = &result.error {
        let color = if error == "Timeout" || is_instruction_budget_error(error) {
            YELLOW
        } else {
            RED
        };
        println!("{color}✗ {label} {error}{RESET}");
    } else {
        println!(
            "{RED}✗ {label} Wrong answer after {} instructions{RESET}",
            result.instructions.unwrap_or(0)
        );
    }
}

fn progress_result_message(result: &RunResult) -> String {
    if result.passed {
        format!(
            "Correct after {} instructions",
            result.instructions.unwrap_or(0)
        )
    } else if let Some(error) = &result.error {
        error.clone()
    } else {
        format!(
            "Wrong answer after {} instructions",
            result.instructions.unwrap_or(0)
        )
    }
}

fn finish_progress(progress: &ProgressBar, result: &RunResult) {
    if progress.is_hidden() {
        progress.finish_and_clear();
        print_run_result(result);
    } else {
        progress.abandon_with_message(progress_result_message(result));
    }
}

fn job_count(requested: Option<usize>) -> usize {
    requested
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, usize::from))
        .max(1)
}

async fn load_solution_case(
    connection: &Connection,
    year: u16,
    day: u8,
    part: u8,
) -> Result<SolutionCase, Box<dyn std::error::Error>> {
    let binary_name = format!("aoc-{year}-{day:02}-{part}");
    Ok(SolutionCase {
        year,
        day,
        part,
        input: read_input(connection, year, day, part).await?,
        expected_hash: read_answer_hash(connection, year, day, part).await?,
        binary: PathBuf::from("target")
            .join(TARGET)
            .join("release")
            .join(&binary_name),
        label: format!("{year}-{day:02}-{part}"),
    })
}

fn solution_binary_name(case: &SolutionCase) -> String {
    format!("aoc-{}-{:02}-{}", case.year, case.day, case.part)
}

fn build_solution(case: &SolutionCase) -> Result<(), String> {
    let package = format!("aoc-{}", case.year);
    let binary_name = solution_binary_name(case);
    let build = Command::new("cargo")
        .args([
            "build",
            "-p",
            &package,
            "--bin",
            &binary_name,
            "--release",
            "--target",
            TARGET,
        ])
        .output()
        .map_err(|error| format!("Cannot invoke Cargo: {error}"))?;
    if build.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&build.stderr);
        Err(format!("Build error: {}", last_nonempty_line(&stderr)))
    }
}

fn build_solutions(cases: &[SolutionCase]) -> Result<BuildReport, String> {
    let mut packages = cases
        .iter()
        .map(|case| format!("aoc-{}", case.year))
        .collect::<Vec<_>>();
    packages.sort_unstable();
    packages.dedup();

    let mut binaries = cases.iter().map(solution_binary_name).collect::<Vec<_>>();
    binaries.sort_unstable();
    binaries.dedup();
    let requested = binaries.iter().cloned().collect::<HashSet<_>>();
    let mut command = Command::new("cargo");
    command.args([
        "build",
        "--release",
        "--target",
        TARGET,
        "--keep-going",
        "--message-format=json-render-diagnostics",
    ]);
    for package in packages {
        command.arg("-p").arg(package);
    }
    for binary in &binaries {
        command.arg("--bin").arg(binary);
    }

    let output = command
        .output()
        .map_err(|error| format!("Cannot invoke Cargo: {error}"))?;
    let mut artifacts = HashSet::new();
    let mut errors = HashMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(binary) = message
            .pointer("/target/name")
            .and_then(|name| name.as_str())
        else {
            continue;
        };
        if !requested.contains(binary) {
            continue;
        }
        match message.get("reason").and_then(|reason| reason.as_str()) {
            Some("compiler-artifact")
                if message
                    .get("executable")
                    .is_some_and(|path| !path.is_null()) =>
            {
                artifacts.insert(binary.to_string());
            }
            Some("compiler-message")
                if message
                    .pointer("/message/level")
                    .and_then(|level| level.as_str())
                    == Some("error") =>
            {
                let detail = message
                    .pointer("/message/message")
                    .and_then(|detail| detail.as_str())
                    .unwrap_or("compilation failed");
                errors
                    .entry(binary.to_string())
                    .or_insert_with(|| format!("Build error: {detail}"));
            }
            _ => {}
        }
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let fallback_error = if output.status.success() {
        "Build error: Cargo did not produce the executable".to_string()
    } else {
        format!("Build error: {}", last_nonempty_line(&stderr))
    };
    Ok(BuildReport {
        artifacts,
        errors,
        fallback_error,
    })
}

async fn evaluate_solution(
    case: SolutionCase,
    progress: ProgressBar,
    multi: Option<MultiProgress>,
) -> RunResult {
    let result = match evaluate_binary(
        &case.binary,
        &case.input,
        &case.label,
        &progress,
        multi.as_ref(),
    )
    .await
    {
        Ok(evaluation) => RunResult {
            year: case.year,
            day: case.day,
            part: case.part,
            passed: answer_matches(
                &case.expected_hash,
                case.year,
                case.day,
                case.part,
                &evaluation.actual,
            ),
            instructions: Some(evaluation.instructions),
            error: None,
        },
        Err(error) => failed_result(case.year, case.day, case.part, error),
    };
    finish_progress(&progress, &result);
    result
}

async fn run_solution(
    connection: &Connection,
    year: u16,
    day: u8,
    part: u8,
) -> Result<RunResult, Box<dyn std::error::Error>> {
    let case = load_solution_case(connection, year, day, part).await?;
    print_progress(&format!("{} Compiling for {TARGET}...", case.label));
    if let Err(error) = build_solution(&case) {
        let result = failed_result(year, day, part, error);
        print_run_result(&result);
        return Ok(result);
    }

    print_progress(&format!("{} Evaluating in QEMU...", case.label));
    let progress = solution_progress_bar(&case.label);
    Ok(evaluate_solution(case, progress, None).await)
}

fn failed_result(year: u16, day: u8, part: u8, error: String) -> RunResult {
    RunResult {
        year,
        day,
        part,
        passed: false,
        instructions: None,
        error: Some(error),
    }
}

fn instruction_budget_error() -> String {
    format!("Instruction budget exceeded ({INSTRUCTION_BUDGET} instructions)")
}

fn is_instruction_budget_error(error: &str) -> bool {
    error.starts_with("Instruction budget exceeded")
}

fn read_progress_snapshot(file: &mut File) -> Result<Option<ProgressSnapshot>, String> {
    let mut bytes = [0_u8; PROGRESS_MAILBOX_SIZE as usize];
    file.seek(SeekFrom::Start(0))
        .and_then(|_| file.read_exact(&mut bytes))
        .map_err(|error| format!("Cannot read instruction progress: {error}"))?;
    if &bytes[..8] != PROGRESS_MAGIC {
        return Ok(None);
    }
    let version = read_u32(&bytes, 8);
    if version != PROGRESS_VERSION {
        return Err(format!("Unsupported progress protocol version {version}"));
    }
    Ok(Some(ProgressSnapshot {
        state: read_u32(&bytes, 12),
        instructions: read_u64(&bytes, 16),
    }))
}

fn progress_level(instructions: u64) -> u64 {
    let units = instructions / PROGRESS_INITIAL_THRESHOLD;
    if units == 0 {
        0
    } else {
        u64::from(u64::BITS - units.leading_zeros())
    }
}

fn solution_progress_bar(label: &str) -> ProgressBar {
    let levels = progress_level(INSTRUCTION_BUDGET);
    let template = format!("{{prefix}} [{{bar:{levels}.cyan/blue}}] {{msg}}");
    let bar = ProgressBar::hidden();
    bar.set_length(levels);
    if let Ok(style) = ProgressStyle::with_template(&template) {
        bar.set_style(style.progress_chars("#>-"));
    }
    bar.set_prefix(label.to_string());
    bar.set_message("In progress at 0 instructions");
    bar
}

fn observe_progress(
    file: &mut File,
    bar: &ProgressBar,
    multi: Option<&MultiProgress>,
    attached: &mut bool,
    last_instructions: &mut u64,
) -> Result<bool, String> {
    let Some(snapshot) = read_progress_snapshot(file)? else {
        return Ok(false);
    };

    if snapshot.instructions != *last_instructions {
        *last_instructions = snapshot.instructions;
        bar.set_message(format!(
            "In progress at {} instructions",
            snapshot.instructions
        ));
        bar.set_position(progress_level(snapshot.instructions));
        if !*attached {
            if let Some(multi) = multi {
                multi.add(bar.clone());
            } else {
                bar.set_draw_target(ProgressDrawTarget::stderr());
            }
            bar.tick();
            *attached = true;
        }
    }
    Ok(snapshot.state == PROGRESS_BUDGET_EXCEEDED || snapshot.instructions > INSTRUCTION_BUDGET)
}

async fn evaluate_binary(
    binary: &Path,
    input: &str,
    label: &str,
    progress: &ProgressBar,
    multi: Option<&MultiProgress>,
) -> Result<Evaluation, String> {
    let run_directory = RunDirectory::create(label).map_err(|error| error.to_string())?;
    let ram_path = run_directory.path().join("guest.ram");
    let input_path = run_directory.path().join("input.packet");
    let progress_path = run_directory.path().join("progress.bin");
    let score_path = run_directory.path().join("score.txt");

    File::create(&ram_path)
        .and_then(|file| file.set_len(RAM_SIZE as u64))
        .map_err(|error| format!("Cannot create guest RAM: {error}"))?;
    File::create(&progress_path)
        .and_then(|file| file.set_len(PROGRESS_MAILBOX_SIZE))
        .map_err(|error| format!("Cannot create progress mailbox: {error}"))?;
    let mut progress_file = File::open(&progress_path)
        .map_err(|error| format!("Cannot open progress mailbox: {error}"))?;
    write_input_packet(&input_path, input.as_bytes())?;

    let start = find_symbol(binary, "__aoc_roi_start")?;
    let stop = find_symbol(binary, "__aoc_roi_end")?;
    let plugin = fs::canonicalize("target/aoc-evaluator/roi-counter.so")
        .map_err(|error| format!("Cannot locate QEMU plugin: {error}"))?;
    let ram_path = fs::canonicalize(&ram_path).map_err(|error| error.to_string())?;
    let input_path = fs::canonicalize(&input_path).map_err(|error| error.to_string())?;
    let progress_path = fs::canonicalize(&progress_path).map_err(|error| error.to_string())?;
    let score_absolute = std::env::current_dir()
        .map_err(|error| error.to_string())?
        .join(&score_path);

    let object = format!(
        "memory-backend-file,id=mem,size={RAM_SIZE},mem-path={},share=on",
        ram_path.display()
    );
    let loader = format!(
        "loader,file={},addr=0x{INPUT_ADDRESS:x},force-raw=on",
        input_path.display()
    );
    let plugin_argument = format!(
        "{},start=0x{start:x},stop=0x{stop:x},budget={INSTRUCTION_BUDGET},out={},progress={}",
        plugin.display(),
        score_absolute.display(),
        progress_path.display()
    );

    let child = tokio::process::Command::new("qemu-system-riscv64")
        .args([
            "-machine",
            "virt,memory-backend=mem",
            "-object",
            &object,
            "-m",
            "256M",
            "-smp",
            "1",
            "-display",
            "none",
            "-monitor",
            "none",
            "-serial",
            "none",
            "-bios",
            "none",
            "-cpu",
            "rv64,v=true,vlen=256,elen=64",
            "-kernel",
        ])
        .arg(binary)
        .args(["-device", &loader, "-plugin", &plugin_argument])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("Failed to start QEMU: {error}"))?;

    let mut progress_timer = interval(PROGRESS_POLL_INTERVAL);
    progress_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
    progress_timer.tick().await;
    let deadline = sleep(HOST_TIMEOUT);
    tokio::pin!(deadline);
    let wait = child.wait_with_output();
    tokio::pin!(wait);
    let mut progress_attached = false;
    let mut last_progress = 0;
    let output = loop {
        tokio::select! {
            result = &mut wait => {
                if observe_progress(
                    &mut progress_file,
                    progress,
                    multi,
                    &mut progress_attached,
                    &mut last_progress,
                )? {
                    return Err(instruction_budget_error());
                }
                break result.map_err(|error| format!("QEMU failed: {error}"))?;
            }
            _ = &mut deadline => {
                let _ = observe_progress(
                    &mut progress_file,
                    progress,
                    multi,
                    &mut progress_attached,
                    &mut last_progress,
                );
                return Err("Timeout".to_string());
            }
            _ = progress_timer.tick() => {
                if observe_progress(
                    &mut progress_file,
                    progress,
                    multi,
                    &mut progress_attached,
                    &mut last_progress,
                )? {
                    return Err(instruction_budget_error());
                }
            }
        }
    };

    let mailbox = read_mailbox(&ram_path)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(match mailbox.state {
            aoc_runtime::STATE_INVALID_INPUT => "Guest rejected the input packet".to_string(),
            aoc_runtime::STATE_PANIC => "Guest panicked".to_string(),
            aoc_runtime::STATE_RUNNING => {
                format!("Guest exited during solve: {}", last_nonempty_line(&stderr))
            }
            _ => format!("QEMU error: {}", last_nonempty_line(&stderr)),
        });
    }
    if mailbox.state != STATE_OK {
        return Err(format!("Guest returned mailbox state {}", mailbox.state));
    }

    let actual = match mailbox.answer_kind {
        aoc_runtime::ANSWER_UNSIGNED => mailbox.bits.to_string(),
        aoc_runtime::ANSWER_SIGNED => (mailbox.bits as i128).to_string(),
        kind => return Err(format!("Unknown answer kind {kind}")),
    };
    let instructions = read_score(&score_path)?;
    if instructions > INSTRUCTION_BUDGET {
        return Err(instruction_budget_error());
    }

    Ok(Evaluation {
        actual,
        instructions,
    })
}

fn command_diagnostics(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let lines = stdout
        .lines()
        .chain(stderr.lines())
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let first = lines.len().saturating_sub(20);
    if lines.is_empty() {
        format!("process exited with {}", output.status)
    } else {
        lines[first..].join("\n")
    }
}

fn probe_tool(
    issues: &mut Vec<PreflightIssue>,
    dependency: &'static str,
    command: &str,
    arguments: &[&str],
    guidance: &'static str,
) -> Option<std::process::Output> {
    match Command::new(command).args(arguments).output() {
        Ok(output) if output.status.success() => Some(output),
        Ok(output) => {
            issues.push(PreflightIssue {
                dependency,
                detail: command_diagnostics(&output),
                guidance,
            });
            None
        }
        Err(error) => {
            issues.push(PreflightIssue {
                dependency,
                detail: format!("cannot run `{command}`: {error}"),
                guidance,
            });
            None
        }
    }
}

#[derive(Clone, Copy)]
enum LinuxPackageManager {
    Pacman,
    Apt,
    Dnf,
    Apk,
    Other,
}

fn command_on_path(command: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|directory| directory.join(command).is_file())
    })
}

fn linux_package_manager() -> LinuxPackageManager {
    if command_on_path("pacman") || Path::new("/etc/arch-release").is_file() {
        LinuxPackageManager::Pacman
    } else if command_on_path("apt") || command_on_path("apt-get") {
        LinuxPackageManager::Apt
    } else if command_on_path("dnf") {
        LinuxPackageManager::Dnf
    } else if command_on_path("apk") {
        LinuxPackageManager::Apk
    } else {
        LinuxPackageManager::Other
    }
}

fn linux_setup_message(package_manager: LinuxPackageManager) -> &'static str {
    match package_manager {
        LinuxPackageManager::Pacman => {
            "\nArch Linux / pacman setup:\n  sudo pacman -S --needed rustup qemu-system-riscv cmake gcc pkgconf glib2 binutils\n  rustup default stable\n"
        }
        LinuxPackageManager::Apt => {
            "\nDebian / Ubuntu APT setup:\n  sudo apt update\n  sudo apt install qemu-system-misc qemu-system-common cmake build-essential pkg-config libglib2.0-dev binutils\n"
        }
        LinuxPackageManager::Dnf => {
            "\nFedora / RHEL-family DNF setup:\n  sudo dnf install qemu-system-riscv-core qemu-common cmake gcc pkgconf-pkg-config glib2-devel binutils\n"
        }
        LinuxPackageManager::Apk => {
            "\nAlpine Linux apk setup:\n  sudo apk add qemu-system-riscv64 qemu-dev cmake build-base pkgconf glib-dev binutils\n"
        }
        LinuxPackageManager::Other => {
            "\nLinux setup: install your distribution's QEMU RISC-V system emulator with plugin headers, CMake, a C compiler, pkg-config, GLib development headers, and binutils.\n"
        }
    }
}

fn preflight_failure_message(issues: &[PreflightIssue]) -> String {
    let mut message = "Host evaluator dependency pre-flight failed:\n".to_string();
    for issue in issues {
        message.push_str(&format!(
            "\n- {}: {}\n  Fix: {}\n",
            issue.dependency, issue.detail, issue.guidance
        ));
    }

    if cfg!(target_os = "macos") {
        message.push_str(
            "\nmacOS setup:\n  xcode-select --install\n  brew install qemu cmake pkgconf glib\n",
        );
    } else if cfg!(target_os = "linux") {
        message.push_str(linux_setup_message(linux_package_manager()));
    }
    message.push_str(&format!(
        "  rustup target add {TARGET}\n\nSee README.md#requirements for details."
    ));
    message
}

fn evaluator_preflight() -> Result<(), PreflightError> {
    let mut issues = Vec::new();
    let _ = probe_tool(
        &mut issues,
        "Cargo",
        "cargo",
        &["--version"],
        "install the Rust toolchain and Cargo with rustup",
    );
    let rustc_available = probe_tool(
        &mut issues,
        "Rust compiler",
        "rustc",
        &["--version"],
        "install the stable Rust toolchain with rustup",
    )
    .is_some();

    if rustc_available {
        match Command::new("rustc")
            .args(["--print", "target-libdir", "--target", TARGET])
            .output()
        {
            Ok(output) if output.status.success() => {
                let path = String::from_utf8_lossy(&output.stdout);
                if !Path::new(path.trim()).is_dir() {
                    issues.push(PreflightIssue {
                        dependency: "Rust RISC-V target",
                        detail: format!("{TARGET} standard libraries are not installed"),
                        guidance: "install the target with `rustup target add riscv64gc-unknown-none-elf`",
                    });
                }
            }
            Ok(output) => issues.push(PreflightIssue {
                dependency: "Rust RISC-V target",
                detail: command_diagnostics(&output),
                guidance: "install the target with `rustup target add riscv64gc-unknown-none-elf`",
            }),
            Err(error) => issues.push(PreflightIssue {
                dependency: "Rust RISC-V target",
                detail: format!("cannot query rustc: {error}"),
                guidance: "install the target with `rustup target add riscv64gc-unknown-none-elf`",
            }),
        }
    }

    let qemu = probe_tool(
        &mut issues,
        "QEMU RISC-V system emulator",
        "qemu-system-riscv64",
        &["-help"],
        "install QEMU with RISC-V system emulation and plugin support",
    );
    let qemu_available = qemu.is_some();
    if let Some(output) = qemu {
        let help = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if !help.contains("-plugin") {
            issues.push(PreflightIssue {
                dependency: "QEMU plugin support",
                detail: "`qemu-system-riscv64 -help` does not advertise `-plugin`".to_string(),
                guidance: "install a QEMU build configured with TCG plugin support",
            });
        }
    }

    let cmake_available = probe_tool(
        &mut issues,
        "CMake",
        "cmake",
        &["--version"],
        "install CMake",
    )
    .is_some();
    let compiler_available = probe_tool(
        &mut issues,
        "C compiler",
        "cc",
        &["--version"],
        "install Xcode Command Line Tools on macOS or a C build toolchain on Linux",
    )
    .is_some();
    let pkg_config_available = probe_tool(
        &mut issues,
        "pkg-config",
        "pkg-config",
        &["--version"],
        "install pkg-config (Homebrew package `pkgconf` on macOS)",
    )
    .is_some();
    let mut glib_available = false;
    if pkg_config_available {
        glib_available = probe_tool(
            &mut issues,
            "GLib development files",
            "pkg-config",
            &["--exists", "glib-2.0"],
            "install the GLib development package (Homebrew package `glib` on macOS)",
        )
        .is_some();
    }
    let _ = probe_tool(
        &mut issues,
        "nm",
        "nm",
        &["--version"],
        "install binutils or Xcode Command Line Tools",
    );

    if qemu_available
        && cmake_available
        && compiler_available
        && pkg_config_available
        && glib_available
        && let Err(error) = ensure_plugin()
    {
        issues.push(PreflightIssue {
            dependency: "QEMU plugin build",
            detail: error,
            guidance: "install qemu-plugin.h and the host build dependencies listed in README.md",
        });
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(PreflightError(preflight_failure_message(&issues)))
    }
}

fn ensure_plugin() -> Result<(), String> {
    let configure = Command::new("cmake")
        .args([
            "-UQEMU_PLUGIN_INCLUDE_DIR",
            "-S",
            "evaluator",
            "-B",
            "target/aoc-evaluator",
            "-DCMAKE_BUILD_TYPE=Release",
            "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON",
        ])
        .output()
        .map_err(|error| format!("failed to configure QEMU plugin: {error}"))?;
    if !configure.status.success() {
        return Err(format!(
            "failed to configure QEMU plugin:\n{}",
            command_diagnostics(&configure)
        ));
    }

    let build = Command::new("cmake")
        .args([
            "--build",
            "target/aoc-evaluator",
            "--config",
            "Release",
            "--target",
            "roi-counter",
        ])
        .output()
        .map_err(|error| format!("failed to build QEMU plugin: {error}"))?;
    if build.status.success() {
        Ok(())
    } else {
        Err(format!(
            "failed to build QEMU plugin:\n{}",
            command_diagnostics(&build)
        ))
    }
}

fn write_input_packet(path: &Path, input: &[u8]) -> Result<(), String> {
    if input.len() > INPUT_CAPACITY - INPUT_HEADER_SIZE {
        return Err(format!(
            "Input exceeds {} byte limit",
            INPUT_CAPACITY - INPUT_HEADER_SIZE
        ));
    }

    let mut packet = File::create(path).map_err(|error| error.to_string())?;
    packet
        .write_all(&INPUT_MAGIC)
        .and_then(|_| packet.write_all(&PROTOCOL_VERSION.to_le_bytes()))
        .and_then(|_| packet.write_all(&0_u32.to_le_bytes()))
        .and_then(|_| packet.write_all(&(input.len() as u64).to_le_bytes()))
        .and_then(|_| packet.write_all(input))
        .map_err(|error| format!("Cannot write input packet: {error}"))
}

fn find_symbol(binary: &Path, name: &str) -> Result<u64, String> {
    let output = Command::new("nm")
        .arg(binary)
        .output()
        .map_err(|error| format!("Failed to run nm: {error}"))?;
    if !output.status.success() {
        return Err("nm could not read the solution ELF".to_string());
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            let address = fields.next()?;
            let _kind = fields.next()?;
            (fields.next()? == name).then(|| u64::from_str_radix(address, 16).ok())?
        })
        .ok_or_else(|| format!("ELF does not export {name}"))
}

struct MailboxValue {
    state: u32,
    answer_kind: u32,
    bits: u128,
}

fn read_mailbox(ram_path: &Path) -> Result<MailboxValue, String> {
    let mut ram = File::open(ram_path).map_err(|error| error.to_string())?;
    ram.seek(SeekFrom::Start((OUTPUT_ADDRESS - RAM_BASE) as u64))
        .map_err(|error| error.to_string())?;
    let mut bytes = [0_u8; OUTPUT_MAILBOX_SIZE];
    ram.read_exact(&mut bytes)
        .map_err(|error| format!("Cannot read output mailbox: {error}"))?;

    if bytes[..8] != OUTPUT_MAGIC || read_u32(&bytes, 8) != PROTOCOL_VERSION {
        return Err("Guest did not write a valid output mailbox".to_string());
    }

    let low = read_u64(&bytes, 24) as u128;
    let high = read_u64(&bytes, 32) as u128;
    Ok(MailboxValue {
        state: read_u32(&bytes, 12),
        answer_kind: read_u32(&bytes, 16),
        bits: low | (high << 64),
    })
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

fn read_score(path: &Path) -> Result<u64, String> {
    let score = fs::read_to_string(path)
        .map_err(|error| format!("Instruction counter produced no score: {error}"))?;
    let completed = score.lines().any(|line| line == "completed=1");
    let instructions = score.lines().find_map(|line| {
        line.strip_prefix("instructions=")
            .and_then(|value| value.parse().ok())
    });
    if !completed {
        return Err("Instruction counter did not observe the complete ROI".to_string());
    }
    instructions.ok_or_else(|| "Instruction counter produced an invalid score".to_string())
}

fn answer_hash(year: u16, day: u8, part: u8, answer: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"santa-goes-brrrr:answer:v1\0");
    hasher.update(year.to_le_bytes());
    hasher.update([day, part]);
    hasher.update(answer.trim().as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn answer_matches(expected_hash: &str, year: u16, day: u8, part: u8, actual: &str) -> bool {
    expected_hash == answer_hash(year, day, part, actual)
}

#[cfg(test)]
mod tests {
    use super::{
        LinuxPackageManager, PreflightIssue, answer_hash, linux_setup_message,
        preflight_failure_message,
    };

    #[test]
    fn answer_hash_is_canonical_and_domain_separated() {
        assert_eq!(
            answer_hash(2025, 1, 1, "42\n"),
            "sha256:d6518393aaa2c3654a82a7ddc576bfe1abb68a27cde227f8ad2222e7280dd60f"
        );
        assert_eq!(
            answer_hash(2025, 1, 1, " 42 "),
            answer_hash(2025, 1, 1, "42")
        );
        assert_ne!(answer_hash(2025, 1, 1, "42"), answer_hash(2025, 1, 2, "42"));
    }

    #[test]
    fn linux_package_manager_setups_are_actionable() {
        for (package_manager, command, qemu_package) in [
            (
                LinuxPackageManager::Pacman,
                "sudo pacman -S --needed",
                "qemu-system-riscv",
            ),
            (
                LinuxPackageManager::Apt,
                "sudo apt install",
                "qemu-system-misc",
            ),
            (
                LinuxPackageManager::Dnf,
                "sudo dnf install",
                "qemu-system-riscv-core",
            ),
            (
                LinuxPackageManager::Apk,
                "sudo apk add",
                "qemu-system-riscv64",
            ),
        ] {
            let message = linux_setup_message(package_manager);
            assert!(message.contains(command));
            assert!(message.contains(qemu_package));
        }
    }

    #[test]
    fn preflight_failure_is_actionable() {
        let message = preflight_failure_message(&[PreflightIssue {
            dependency: "QEMU RISC-V system emulator",
            detail: "cannot run `qemu-system-riscv64`: command not found".to_string(),
            guidance: "install QEMU with RISC-V system emulation and plugin support",
        }]);

        assert!(message.contains("Host evaluator dependency pre-flight failed"));
        assert!(message.contains("qemu-system-riscv64"));
        assert!(message.contains("Fix: install QEMU"));
        assert!(message.contains("rustup target add riscv64gc-unknown-none-elf"));
        assert!(message.contains("README.md#requirements"));
    }
}

fn last_nonempty_line(message: &str) -> &str {
    message
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("unknown error")
}

async fn solution_entries(
    connection: &Connection,
) -> Result<Vec<(u16, u8, u8)>, Box<dyn std::error::Error>> {
    let mut rows = connection
        .query(
            "SELECT year, day, part FROM solutions ORDER BY year, day, part",
            (),
        )
        .await?;
    let mut entries = Vec::new();
    while let Some(row) = rows.next().await? {
        entries.push((
            row.get::<u32>(0)? as u16,
            row.get::<u32>(1)? as u8,
            row.get::<u32>(2)? as u8,
        ));
    }
    Ok(entries)
}

async fn run_solutions_parallel(
    connection: &Connection,
    jobs: usize,
) -> Result<Vec<RunResult>, Box<dyn std::error::Error>> {
    let mut cases = Vec::new();
    let mut results = Vec::new();

    for (year, day, part) in solution_entries(connection).await? {
        cases.push(load_solution_case(connection, year, day, part).await?);
    }
    if cases.is_empty() {
        return Ok(results);
    }

    print_progress(&format!(
        "Compiling {} solutions for {TARGET} with Cargo...",
        cases.len()
    ));
    let report = build_solutions(&cases).unwrap_or_else(|error| BuildReport {
        artifacts: HashSet::new(),
        errors: HashMap::new(),
        fallback_error: error,
    });
    cases.retain(|case| {
        let binary = solution_binary_name(case);
        if report.artifacts.contains(&binary) {
            true
        } else {
            let error = report
                .errors
                .get(&binary)
                .unwrap_or(&report.fallback_error)
                .clone();
            let result = failed_result(case.year, case.day, case.part, error);
            print_run_result(&result);
            results.push(result);
            false
        }
    });
    if cases.is_empty() {
        return Ok(results);
    }

    let worker_count = jobs.min(cases.len());
    println!(
        "Evaluating {} solutions with {worker_count} QEMU worker{}...",
        cases.len(),
        if worker_count == 1 { "" } else { "s" }
    );

    let progress = MultiProgress::new();
    let mut pending = cases
        .into_iter()
        .map(|case| (solution_progress_bar(&case.label), case));
    let mut workers = JoinSet::new();
    for (bar, case) in pending.by_ref().take(worker_count) {
        workers.spawn(evaluate_solution(case, bar, Some(progress.clone())));
    }
    while let Some(completed) = workers.join_next().await {
        let result = completed.map_err(|error| format!("QEMU worker failed: {error}"))?;
        results.push(result);
        if let Some((bar, case)) = pending.next() {
            workers.spawn(evaluate_solution(case, bar, Some(progress.clone())));
        }
    }

    results.sort_by_key(|result| (result.year, result.day, result.part));
    Ok(results)
}

async fn run_all_solutions(
    connection: &Connection,
    jobs: usize,
) -> Result<RunSummary, Box<dyn std::error::Error>> {
    let mut summary = RunSummary {
        passed: 0,
        errors: 0,
        timeouts: 0,
        budget_exceeded: 0,
        total_instructions: 0,
    };

    for result in run_solutions_parallel(connection, jobs).await? {
        if result.passed {
            summary.passed += 1;
            summary.total_instructions += u128::from(result.instructions.unwrap_or(0));
        } else if result.error.as_deref() == Some("Timeout") {
            summary.timeouts += 1;
        } else if result
            .error
            .as_deref()
            .is_some_and(is_instruction_budget_error)
        {
            summary.budget_exceeded += 1;
        } else {
            summary.errors += 1;
        }
    }

    Ok(summary)
}
