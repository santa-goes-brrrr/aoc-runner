use aoc_runtime::{
    INPUT_ADDRESS, INPUT_CAPACITY, INPUT_HEADER_SIZE, INPUT_MAGIC, OUTPUT_ADDRESS, OUTPUT_MAGIC,
    OUTPUT_MAILBOX_SIZE, OutputMailbox, PROTOCOL_VERSION, RAM_BASE, RAM_SIZE, STATE_OK,
    TRAP_ADDRESS, TRAP_MAGIC, TRAP_RECORD_SIZE, TRAP_VERSION, TrapRecord,
};
use clap::{Parser, Subcommand};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use libsql::{Builder, Connection};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::mem::offset_of;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::task::JoinSet;
use tokio::time::{MissedTickBehavior, interval};

const TARGET: &str = "riscv64gc-unknown-none-elf";
/// The bare RV64I model plus exactly the extensions the guest is compiled for.
/// Starting from the bare model keeps QEMU's version-dependent default
/// extensions out of the instruction set a solution can use.
const QEMU_CPU: &str = "rv64i,m=true,a=true,f=true,d=true,c=true,v=true,\
    zicsr=true,zifencei=true,pmp=true,vlen=256,elen=64";
/// Wall-clock time the guest may go without executing a single instruction.
/// Every phase of a run has an instruction limit, so a guest that keeps
/// executing always ends on one of those and never on this watchdog.
const HOST_STALL_TIMEOUT: Duration = Duration::from_secs(10);
const PROGRESS_POLL_INTERVAL: Duration = Duration::from_millis(25);
const INSTRUCTION_BUDGET: u64 = 2_000_000_000;
/// Longest text answer the host will read back from guest memory.
const ANSWER_TEXT_LIMIT: u64 = 1_024;
/// Instructions the runtime may execute before the measured region; anything
/// beyond this allowance is work a solution moved out of its score. Each term
/// is at least twice what the runtime was measured to need: 160 instructions
/// of fixed work, half an instruction per `.bss` byte to clear it, and UTF-8
/// validation of the input at half an instruction per byte when it is all
/// ASCII and up to nine per byte otherwise.
const STARTUP_BASE_INSTRUCTIONS: u64 = 1_024;
const STARTUP_INSTRUCTIONS_PER_BSS_BYTE: u64 = 1;
const STARTUP_INSTRUCTIONS_PER_ASCII_INPUT_BYTE: u64 = 1;
const STARTUP_INSTRUCTIONS_PER_OTHER_INPUT_BYTE: u64 = 24;
const PROGRESS_INITIAL_THRESHOLD: u64 = 100_000;
const PROGRESS_MAGIC: &[u8; 8] = b"AOCPROG1";
const PROGRESS_VERSION: u32 = 2;
const PROGRESS_MAILBOX_SIZE: u64 = 32;
const RUNS_DIRECTORY: &str = "target/aoc-evaluator/runs";
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
    #[cfg_attr(not(test), allow(dead_code))]
    startup_instructions: u64,
}

#[derive(Clone, Copy)]
struct Limits {
    budget: u64,
    startup: u64,
}

/// A deterministic instruction limit enforced by the counting plugin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LimitFailure {
    Budget,
    Startup,
    Shutdown,
    Markers,
}

impl LimitFailure {
    /// Mirrors the failure values of `enum progress_state` in `evaluator/roi-counter.c`.
    fn from_progress_state(state: u32) -> Option<Self> {
        match state {
            3 => Some(Self::Budget),
            4 => Some(Self::Startup),
            5 => Some(Self::Shutdown),
            6 => Some(Self::Markers),
            _ => None,
        }
    }

    fn from_score_state(state: &str) -> Option<Self> {
        match state {
            "budget-exceeded" => Some(Self::Budget),
            "startup-limit-exceeded" => Some(Self::Startup),
            "shutdown-limit-exceeded" => Some(Self::Shutdown),
            "marker-violation" => Some(Self::Markers),
            _ => None,
        }
    }

    fn message(self, limits: Limits) -> String {
        match self {
            Self::Budget => instruction_budget_error(limits.budget),
            Self::Startup => format!(
                "Startup instruction limit exceeded ({} instructions before the measured region)",
                limits.startup
            ),
            Self::Shutdown => {
                "Shutdown instruction limit exceeded after the measured region".to_string()
            }
            Self::Markers => "ROI markers executed out of order or more than once".to_string(),
        }
    }
}

struct GuestSymbols {
    roi_start: u64,
    roi_end: u64,
    bss_size: u64,
    stack_guard: Range<u64>,
}

struct Score {
    state: String,
    instructions: u64,
    startup_instructions: u64,
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
    executed: u64,
}

struct ProgressView {
    attached: bool,
    shown: u64,
}

enum Outcome {
    Finished(Result<ExitCode, Box<dyn std::error::Error>>),
    Interrupted(io::Result<i32>),
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
        // The owning process id comes first so that a later run can tell
        // whether a leftover directory still belongs to a live evaluator.
        let id = RUN_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            PathBuf::from(RUNS_DIRECTORY).join(format!("{}-{id}-{label}", std::process::id()));
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
async fn main() -> ExitCode {
    let cli = Cli::parse();
    // Dropping the command future kills its QEMU children and removes their
    // run directories, so a signal cancels the command instead of ending the
    // process underneath it.
    let outcome = tokio::select! {
        biased;
        signal = termination_signal() => Outcome::Interrupted(signal),
        result = run(cli) => Outcome::Finished(result),
    };
    match outcome {
        Outcome::Finished(Ok(code)) => code,
        Outcome::Finished(Err(error)) => {
            eprintln!("Error: {error:?}");
            ExitCode::FAILURE
        }
        Outcome::Interrupted(Ok(signal)) => {
            eprintln!("Interrupted: stopping QEMU workers and removing their run directories");
            ExitCode::from(128_u8.saturating_add(u8::try_from(signal).unwrap_or(0)))
        }
        Outcome::Interrupted(Err(error)) => {
            eprintln!("Error: cannot install signal handlers: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn termination_signal() -> io::Result<i32> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;
    let mut hangup = signal(SignalKind::hangup())?;
    Ok(tokio::select! {
        _ = interrupt.recv() => libc::SIGINT,
        _ = terminate.recv() => libc::SIGTERM,
        _ = hangup.recv() => libc::SIGHUP,
    })
}

async fn run(cli: Cli) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let invocation_directory = std::env::current_dir()?;
    enter_repository_root(&invocation_directory)?;

    if matches!(&cli.command, Commands::Evaluate { .. }) {
        print_progress("Checking host evaluator dependencies...");
        evaluator_preflight()?;
        print_progress("Host evaluator dependencies are available.");
        sweep_stale_run_directories();
    }

    let database = Builder::new_local("aoc.db").build().await?;
    let connection = database.connect()?;
    create_schema(&connection).await?;

    match cli.command {
        Commands::Solution { command } => {
            run_solution_command(&connection, command, &invocation_directory).await?;
            Ok(ExitCode::SUCCESS)
        }
        Commands::Evaluate { command } => match command {
            EvaluateCommands::One { year, day, part } => {
                let result = run_solution(&connection, year, day, part).await?;
                Ok(if result.passed {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                })
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
                Ok(
                    if summary.errors > 0 || summary.timeouts > 0 || summary.budget_exceeded > 0 {
                        ExitCode::FAILURE
                    } else {
                        ExitCode::SUCCESS
                    },
                )
            }
        },
    }
}

/// Every path the CLI uses (`aoc.db`, `target/`, `evaluator/`, the linker
/// script named in `.cargo/config.toml`) is relative to the repository root,
/// so find it from wherever the CLI was started and work from there.
fn enter_repository_root(start: &Path) -> Result<(), String> {
    let root = start
        .ancestors()
        .find(|directory| {
            directory.join("runtime/linker.ld").is_file()
                && directory.join("evaluator/roi-counter.c").is_file()
                && directory.join("Cargo.toml").is_file()
        })
        .ok_or_else(|| {
            format!(
                "{} is not inside the santa-goes-brrrr repository; run the CLI from the repository",
                start.display()
            )
        })?;
    std::env::set_current_dir(root)
        .map_err(|error| format!("Cannot enter {}: {error}", root.display()))
}

/// A run directory outlives its evaluator only when that process was killed
/// outright. It holds the plaintext puzzle input and a 256 MiB RAM image.
fn sweep_stale_run_directories() {
    let Ok(entries) = fs::read_dir(RUNS_DIRECTORY) else {
        return;
    };
    for entry in entries.flatten() {
        let owner = entry
            .file_name()
            .to_str()
            .and_then(|name| name.split('-').next()?.parse::<u32>().ok());
        if let Some(owner) = owner
            && owner != std::process::id()
            && !process_is_alive(owner)
        {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn process_is_alive(process: u32) -> bool {
    let Ok(process) = libc::pid_t::try_from(process) else {
        return true;
    };
    // SAFETY: signal 0 only performs the existence and permission checks.
    let delivered = unsafe { libc::kill(process, 0) } == 0;
    delivered || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

async fn run_solution_command(
    connection: &Connection,
    command: SolutionCommands,
    invocation_directory: &Path,
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
            let input_contents = read_input_file(&invocation_directory.join(input))?;
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
                let input_contents = read_input_file(&invocation_directory.join(input))?;
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
        let color = if is_timeout_error(error) || is_instruction_budget_error(error) {
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

fn solution_source(year: u16, day: u8, part: u8) -> PathBuf {
    PathBuf::from(format!(
        "solutions/year={year}/src/bin/aoc-{year}-{day:02}-{part}.rs"
    ))
}

fn missing_source_error(year: u16, day: u8, part: u8) -> String {
    format!(
        "No solution source at {}",
        solution_source(year, day, part).display()
    )
}

/// The `(year, day, part)` of every solution source in the documented layout.
fn solution_sources() -> Vec<(u16, u8, u8)> {
    let mut sources = Vec::new();
    let Ok(years) = fs::read_dir("solutions") else {
        return sources;
    };
    for year in years.flatten() {
        let Ok(files) = fs::read_dir(year.path().join("src/bin")) else {
            continue;
        };
        for file in files.flatten() {
            let key = file.file_name().to_str().and_then(|name| {
                let mut fields = name.strip_prefix("aoc-")?.strip_suffix(".rs")?.split('-');
                let key = (
                    fields.next()?.parse().ok()?,
                    fields.next()?.parse().ok()?,
                    fields.next()?.parse().ok()?,
                );
                fields.next().is_none().then_some(key)
            });
            if let Some((year, day, part)) = key
                && solution_source(year, day, part) == file.path()
            {
                sources.push((year, day, part));
            }
        }
    }
    sources.sort_unstable();
    sources
}

/// rustc embeds the source paths of dependencies and of the standard library
/// in the guest, for panic locations. Those paths contain the user's Cargo
/// home and, when `rust-src` is installed, the toolchain directory. Their
/// length moves every address that follows, and a guest whose behaviour
/// depends on addresses (a hash map seeded from one, for example) then scores
/// differently from one machine to the next. Mapping the prefixes to fixed
/// names makes the guest the same wherever it is built.
fn path_remapping() -> Vec<(PathBuf, String)> {
    let mut prefixes = Vec::new();

    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")));
    if let Some(cargo_home) = cargo_home {
        prefixes.push((cargo_home, "/cargo".to_string()));
    }

    // Without `rust-src` the standard library already appears under this
    // virtual name, so the two cases produce the same guest.
    let query = |arguments: &[&str]| {
        let output = Command::new("rustc").args(arguments).output().ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    };
    let commit = query(&["-vV"]).and_then(|version| {
        version
            .lines()
            .find_map(|line| line.strip_prefix("commit-hash: ").map(str::to_string))
    });
    if let (Some(sysroot), Some(commit)) = (query(&["--print", "sysroot"]), commit)
        && commit.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        prefixes.push((
            PathBuf::from(sysroot.trim()).join("lib/rustlib/src/rust"),
            format!("/rustc/{commit}"),
        ));
    }

    // Cargo and rustc may name the same directory through a symbolic link.
    for index in 0..prefixes.len() {
        if let Ok(resolved) = fs::canonicalize(&prefixes[index].0)
            && resolved != prefixes[index].0
        {
            prefixes.push((resolved, prefixes[index].1.clone()));
        }
    }
    prefixes
}

/// `cargo build --release` for the guest target with the path remapping
/// applied. Every guest that is scored must be built through this.
fn guest_build_command() -> Command {
    let flags = path_remapping()
        .into_iter()
        .map(|(from, to)| {
            serde_json::Value::from(format!("--remap-path-prefix={}={to}", from.display()))
                .to_string()
        })
        .collect::<Vec<_>>();
    let mut command = Command::new("cargo");
    command.args(["build", "--release", "--target", TARGET]);
    if !flags.is_empty() {
        // Cargo appends this list to the `rustflags` in `.cargo/config.toml`.
        command
            .arg("--config")
            .arg(format!("target.{TARGET}.rustflags=[{}]", flags.join(",")));
    }
    command
}

fn build_solution(case: &SolutionCase) -> Result<(), String> {
    let package = format!("aoc-{}", case.year);
    let binary_name = solution_binary_name(case);
    let build = guest_build_command()
        .args(["-p", &package, "--bin", &binary_name])
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
    let mut command = guest_build_command();
    command.args(["--keep-going", "--message-format=json-render-diagnostics"]);
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
    if !solution_source(year, day, part).is_file() {
        let result = failed_result(year, day, part, missing_source_error(year, day, part));
        print_run_result(&result);
        return Ok(result);
    }
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

fn instruction_budget_error(budget: u64) -> String {
    format!("Instruction budget exceeded ({budget} instructions)")
}

fn is_instruction_budget_error(error: &str) -> bool {
    error.starts_with("Instruction budget exceeded")
}

fn timeout_error() -> String {
    format!(
        "Timeout: the guest executed no instructions for {} seconds",
        HOST_STALL_TIMEOUT.as_secs()
    )
}

fn is_timeout_error(error: &str) -> bool {
    error.starts_with("Timeout")
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
        executed: read_u64(&bytes, 24),
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

/// Reads the plugin's progress mailbox and reflects it in the progress bar.
/// Returns `None` until the plugin has initialised the mailbox.
fn observe_progress(
    file: &mut File,
    bar: &ProgressBar,
    multi: Option<&MultiProgress>,
    view: &mut ProgressView,
) -> Result<Option<ProgressSnapshot>, String> {
    let Some(snapshot) = read_progress_snapshot(file)? else {
        return Ok(None);
    };

    if snapshot.instructions != view.shown {
        view.shown = snapshot.instructions;
        bar.set_message(format!(
            "In progress at {} instructions",
            snapshot.instructions
        ));
        bar.set_position(progress_level(snapshot.instructions));
        if !view.attached {
            if let Some(multi) = multi {
                multi.add(bar.clone());
            } else {
                bar.set_draw_target(ProgressDrawTarget::stderr());
            }
            bar.tick();
            view.attached = true;
        }
    }
    Ok(Some(snapshot))
}

/// The most the runtime can execute before the measured region for this
/// binary and input. See the `STARTUP_*` constants.
fn startup_limit(symbols: &GuestSymbols, input: &[u8]) -> u64 {
    // One byte outside ASCII moves validation of the bytes after it onto a
    // slower path, so the whole input gets the higher rate.
    let per_input_byte = if input.is_ascii() {
        STARTUP_INSTRUCTIONS_PER_ASCII_INPUT_BYTE
    } else {
        STARTUP_INSTRUCTIONS_PER_OTHER_INPUT_BYTE
    };
    STARTUP_BASE_INSTRUCTIONS
        + STARTUP_INSTRUCTIONS_PER_BSS_BYTE * symbols.bss_size
        + per_input_byte * input.len() as u64
}

async fn evaluate_binary(
    binary: &Path,
    input: &str,
    label: &str,
    progress: &ProgressBar,
    multi: Option<&MultiProgress>,
) -> Result<Evaluation, String> {
    let symbols = read_guest_symbols(binary)?;
    let limits = Limits {
        budget: INSTRUCTION_BUDGET,
        startup: startup_limit(&symbols, input.as_bytes()),
    };
    evaluate_with_limits(binary, &symbols, input, label, limits, progress, multi).await
}

async fn evaluate_with_limits(
    binary: &Path,
    symbols: &GuestSymbols,
    input: &str,
    label: &str,
    limits: Limits,
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
    // `parent` makes the plugin end QEMU if this process dies without being
    // able to kill it, for example on SIGKILL.
    let plugin_argument = format!(
        "{},start=0x{:x},stop=0x{:x},budget={},startup_limit={},parent={},out={},progress={}",
        plugin.display(),
        symbols.roi_start,
        symbols.roi_end,
        limits.budget,
        limits.startup,
        std::process::id(),
        score_absolute.display(),
        progress_path.display()
    );

    let child = tokio::process::Command::new("qemu-system-riscv64")
        .args([
            "-nodefaults",
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
            QEMU_CPU,
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
    let wait = child.wait_with_output();
    tokio::pin!(wait);
    let mut view = ProgressView {
        attached: false,
        shown: 0,
    };
    let mut last_executed = 0;
    let mut last_advance = Instant::now();
    let output = loop {
        tokio::select! {
            result = &mut wait => {
                break result.map_err(|error| format!("QEMU failed: {error}"))?;
            }
            _ = progress_timer.tick() => {
                let snapshot = observe_progress(&mut progress_file, progress, multi, &mut view)?;
                if let Some(snapshot) = snapshot {
                    if let Some(failure) = LimitFailure::from_progress_state(snapshot.state) {
                        return Err(failure.message(limits));
                    }
                    if snapshot.executed != last_executed {
                        last_executed = snapshot.executed;
                        last_advance = Instant::now();
                    }
                }
                if last_advance.elapsed() >= HOST_STALL_TIMEOUT {
                    return Err(timeout_error());
                }
            }
        }
    };

    // The plugin's verdict comes first: a guest that broke a limit may still
    // have gone on to exit cleanly with a plausible answer.
    let final_progress = observe_progress(&mut progress_file, progress, multi, &mut view)?;
    let score = read_score(&score_path);
    let limit_failure = score
        .as_ref()
        .ok()
        .and_then(|score| LimitFailure::from_score_state(&score.state))
        .or_else(|| {
            final_progress.and_then(|snapshot| LimitFailure::from_progress_state(snapshot.state))
        });
    if let Some(failure) = limit_failure {
        return Err(failure.message(limits));
    }

    let mailbox = read_mailbox(&ram_path);
    if !output.status.success() {
        let diagnostic = qemu_diagnostic(&output);
        return Err(match &mailbox {
            Ok(mailbox) => match mailbox.state {
                aoc_runtime::STATE_INVALID_INPUT => "Guest rejected the input packet".to_string(),
                aoc_runtime::STATE_PANIC => "Guest panicked".to_string(),
                aoc_runtime::STATE_TRAP => match read_trap_record(&ram_path) {
                    Ok(trap) => trap_message(&trap, symbols),
                    Err(error) => format!("Guest trapped: {error}"),
                },
                aoc_runtime::STATE_RUNNING => format!("Guest exited during solve: {diagnostic}"),
                _ => format!("QEMU error: {diagnostic}"),
            },
            // QEMU stopped before the guest wrote anything, so the failure is QEMU's own.
            Err(_) => format!("QEMU error: {diagnostic}"),
        });
    }
    let mailbox = mailbox?;
    if mailbox.state != STATE_OK {
        return Err(format!("Guest returned mailbox state {}", mailbox.state));
    }

    let actual = match mailbox.answer_kind {
        aoc_runtime::ANSWER_UNSIGNED => mailbox.bits.to_string(),
        aoc_runtime::ANSWER_SIGNED => (mailbox.bits as i128).to_string(),
        aoc_runtime::ANSWER_TEXT => {
            // The guest reports where its text lives; the bytes are read from the RAM image.
            let address = mailbox.bits as u64;
            let length = (mailbox.bits >> 64) as u64;
            if length > ANSWER_TEXT_LIMIT {
                return Err(format!(
                    "Guest answer text is {length} bytes; the limit is {ANSWER_TEXT_LIMIT}"
                ));
            }
            let text = read_guest_memory(&ram_path, address, length as usize)
                .map_err(|error| format!("Cannot read guest answer text: {error}"))?;
            String::from_utf8(text)
                .map_err(|_| "Guest returned answer text that is not UTF-8".to_string())?
        }
        kind => return Err(format!("Unknown answer kind {kind}")),
    };
    let score = score?;
    if score.state != "completed" {
        return Err("Instruction counter did not observe the complete ROI".to_string());
    }
    if score.instructions > limits.budget {
        return Err(instruction_budget_error(limits.budget));
    }

    Ok(Evaluation {
        actual,
        instructions: score.instructions,
        startup_instructions: score.startup_instructions,
    })
}

fn qemu_diagnostic(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    match stderr.lines().rev().find(|line| !line.trim().is_empty()) {
        Some(line) => line.to_string(),
        None => format!("QEMU ended with {}", output.status),
    }
}

fn trap_cause_name(cause: u64) -> String {
    let name = match cause {
        0 => "instruction address misaligned",
        1 => "instruction access fault",
        2 => "illegal instruction",
        3 => "breakpoint",
        4 => "load address misaligned",
        5 => "load access fault",
        6 => "store address misaligned",
        7 => "store access fault",
        8..=11 => "environment call",
        12 => "instruction page fault",
        13 => "load page fault",
        15 => "store page fault",
        _ => return format!("exception {cause}"),
    };
    name.to_string()
}

fn trap_message(trap: &TrapValue, symbols: &GuestSymbols) -> String {
    let is_access_fault = matches!(trap.cause, 1 | 5 | 7);
    if is_access_fault && symbols.stack_guard.contains(&trap.value) {
        return format!(
            "Guest stack overflow: reached the guard below the stack at pc=0x{:x}",
            trap.pc
        );
    }
    format!(
        "Guest trapped: {} at pc=0x{:x} (mtval=0x{:x})",
        trap_cause_name(trap.cause),
        trap.pc,
        trap.value
    )
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

fn read_guest_symbols(binary: &Path) -> Result<GuestSymbols, String> {
    let output = Command::new("nm")
        .arg(binary)
        .output()
        .map_err(|error| format!("Failed to run nm: {error}"))?;
    if !output.status.success() {
        return Err("nm could not read the solution ELF".to_string());
    }

    let listing = String::from_utf8_lossy(&output.stdout);
    let mut addresses = HashMap::new();
    for line in listing.lines() {
        let mut fields = line.split_whitespace();
        if let (Some(address), Some(_kind), Some(name)) =
            (fields.next(), fields.next(), fields.next())
            && let Ok(address) = u64::from_str_radix(address, 16)
        {
            addresses.insert(name, address);
        }
    }
    let symbol = |name: &str| {
        addresses
            .get(name)
            .copied()
            .ok_or_else(|| format!("ELF does not export {name}"))
    };

    Ok(GuestSymbols {
        roi_start: symbol("__aoc_roi_start")?,
        roi_end: symbol("__aoc_roi_end")?,
        bss_size: symbol("__bss_end")?.saturating_sub(symbol("__bss_start")?),
        stack_guard: symbol("__stack_guard_start")?..symbol("__stack_guard_end")?,
    })
}

struct MailboxValue {
    state: u32,
    answer_kind: u32,
    bits: u128,
}

struct TrapValue {
    cause: u64,
    pc: u64,
    value: u64,
}

/// Reads guest memory from the file that backs guest RAM, after QEMU has exited.
fn read_guest_memory(ram_path: &Path, address: u64, length: usize) -> Result<Vec<u8>, String> {
    let ram = RAM_BASE as u64..(RAM_BASE + RAM_SIZE) as u64;
    let end = address
        .checked_add(length as u64)
        .filter(|end| ram.contains(&address) && *end <= ram.end)
        .ok_or_else(|| format!("0x{address:x}+{length} is outside guest RAM"))?;
    let mut bytes = vec![0_u8; (end - address) as usize];
    let mut file = File::open(ram_path).map_err(|error| error.to_string())?;
    file.seek(SeekFrom::Start(address - ram.start))
        .and_then(|_| file.read_exact(&mut bytes))
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

fn read_mailbox(ram_path: &Path) -> Result<MailboxValue, String> {
    let bytes = read_guest_memory(ram_path, OUTPUT_ADDRESS as u64, OUTPUT_MAILBOX_SIZE)
        .map_err(|error| format!("Cannot read output mailbox: {error}"))?;

    let magic = offset_of!(OutputMailbox, magic);
    if bytes[magic..magic + OUTPUT_MAGIC.len()] != OUTPUT_MAGIC
        || read_u32(&bytes, offset_of!(OutputMailbox, version)) != PROTOCOL_VERSION
    {
        return Err("Guest did not write a valid output mailbox".to_string());
    }

    let low = read_u64(&bytes, offset_of!(OutputMailbox, answer_low)) as u128;
    let high = read_u64(&bytes, offset_of!(OutputMailbox, answer_high)) as u128;
    Ok(MailboxValue {
        state: read_u32(&bytes, offset_of!(OutputMailbox, state)),
        answer_kind: read_u32(&bytes, offset_of!(OutputMailbox, answer_kind)),
        bits: low | (high << 64),
    })
}

fn read_trap_record(ram_path: &Path) -> Result<TrapValue, String> {
    let bytes = read_guest_memory(ram_path, TRAP_ADDRESS as u64, TRAP_RECORD_SIZE)
        .map_err(|error| format!("cannot read the trap record: {error}"))?;

    let magic = offset_of!(TrapRecord, magic);
    if bytes[magic..magic + TRAP_MAGIC.len()] != TRAP_MAGIC
        || read_u32(&bytes, offset_of!(TrapRecord, version)) != TRAP_VERSION
    {
        return Err("the trap record is invalid".to_string());
    }
    Ok(TrapValue {
        cause: read_u64(&bytes, offset_of!(TrapRecord, cause)),
        pc: read_u64(&bytes, offset_of!(TrapRecord, pc)),
        value: read_u64(&bytes, offset_of!(TrapRecord, value)),
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

fn read_score(path: &Path) -> Result<Score, String> {
    let score = fs::read_to_string(path)
        .map_err(|error| format!("Instruction counter produced no score: {error}"))?;
    let field = |name: &str| {
        score
            .lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix('='))
            .ok_or_else(|| "Instruction counter produced an invalid score".to_string())
    };
    let number = |name: &str| {
        field(name)?
            .parse::<u64>()
            .map_err(|_| "Instruction counter produced an invalid score".to_string())
    };
    Ok(Score {
        state: field("state")?.to_string(),
        instructions: number("instructions")?,
        startup_instructions: number("startup_instructions")?,
    })
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

    let entries = solution_entries(connection).await?;
    if entries.is_empty() {
        return Err("aoc.db holds no solution records, so there is nothing to evaluate".into());
    }

    // A source without a record cannot be evaluated; say so rather than let a
    // bulk run look complete.
    let registered = entries.iter().copied().collect::<HashSet<_>>();
    let unregistered = solution_sources()
        .into_iter()
        .filter(|key| !registered.contains(key))
        .map(|(year, day, part)| format!("{year}-{day:02}-{part}"))
        .collect::<Vec<_>>();
    if !unregistered.is_empty() {
        println!(
            "{YELLOW}Not evaluated, no database record: {}{RESET}",
            unregistered.join(", ")
        );
    }

    for (year, day, part) in entries {
        let case = load_solution_case(connection, year, day, part).await?;
        // Cargo rejects the whole build when asked for a binary that does not
        // exist, so a record without a source must not reach it.
        if solution_source(year, day, part).is_file() {
            cases.push(case);
        } else {
            let result = failed_result(year, day, part, missing_source_error(year, day, part));
            print_run_result(&result);
            results.push(result);
        }
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
        } else if result.error.as_deref().is_some_and(is_timeout_error) {
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

/// End-to-end checks of the evaluator itself: each one runs a fixture guest
/// from `evaluator/fixtures` under QEMU with the counting plugin.
#[cfg(test)]
mod evaluator_tests {
    use super::*;
    use std::sync::OnceLock;

    fn prepare_fixtures() -> Result<(), String> {
        let start = std::env::current_dir().map_err(|error| error.to_string())?;
        enter_repository_root(&start)?;
        ensure_plugin()?;
        let build = guest_build_command()
            .args(["--package", "aoc-fixtures"])
            .output()
            .map_err(|error| format!("Cannot invoke Cargo: {error}"))?;
        if build.status.success() {
            Ok(())
        } else {
            Err(command_diagnostics(&build))
        }
    }

    /// Builds the plugin and every fixture once, then returns one fixture's ELF.
    fn fixture(name: &str) -> PathBuf {
        static READY: OnceLock<Result<(), String>> = OnceLock::new();
        if let Err(error) = READY.get_or_init(prepare_fixtures) {
            panic!("cannot prepare the evaluator fixtures:\n{error}");
        }
        PathBuf::from("target")
            .join(TARGET)
            .join("release")
            .join(name)
    }

    async fn evaluate(name: &str, input: &str) -> Result<Evaluation, String> {
        evaluate_binary(&fixture(name), input, name, &ProgressBar::hidden(), None).await
    }

    /// Evaluates with the default limits adjusted by `adjust`.
    async fn evaluate_adjusted(
        name: &str,
        input: &str,
        adjust: impl FnOnce(&mut Limits),
    ) -> Result<Evaluation, String> {
        let binary = fixture(name);
        let symbols = read_guest_symbols(&binary)?;
        let mut limits = Limits {
            budget: INSTRUCTION_BUDGET,
            startup: startup_limit(&symbols, input.as_bytes()),
        };
        adjust(&mut limits);
        let progress = ProgressBar::hidden();
        evaluate_with_limits(&binary, &symbols, input, name, limits, &progress, None).await
    }

    fn failure(result: Result<Evaluation, String>) -> String {
        match result {
            Ok(evaluation) => panic!(
                "expected a failure, got answer {} after {} instructions",
                evaluation.actual, evaluation.instructions
            ),
            Err(error) => error,
        }
    }

    #[tokio::test]
    async fn counts_every_instruction_exactly_once() {
        let short = evaluate("fixture-count", &"x".repeat(1_000)).await.unwrap();
        // Long enough to cross several of the plugin's flush intervals.
        let long = evaluate("fixture-count", &"x".repeat(3_000_000))
            .await
            .unwrap();

        assert_eq!(short.actual, "1000");
        assert_eq!(long.actual, "3000000");
        // The fixture executes two instructions per input byte; the rest of
        // the measured region is the same handful of instructions either way.
        assert_eq!(
            long.instructions - short.instructions,
            2 * (3_000_000 - 1_000)
        );
        let overhead = short.instructions - 2 * 1_000;
        assert!(overhead < 64, "{overhead} instructions outside the loop");
    }

    #[tokio::test]
    async fn budget_decision_is_exact() {
        let input = "x".repeat(2_000_000);
        let count = evaluate("fixture-count", &input)
            .await
            .unwrap()
            .instructions;

        let at_budget = evaluate_adjusted("fixture-count", &input, |limits| limits.budget = count)
            .await
            .unwrap();
        assert_eq!(at_budget.instructions, count);

        let over_budget =
            evaluate_adjusted("fixture-count", &input, |limits| limits.budget = count - 1).await;
        assert_eq!(failure(over_budget), instruction_budget_error(count - 1));
    }

    #[tokio::test]
    async fn budget_ends_a_guest_that_never_returns() {
        let result =
            evaluate_adjusted("fixture-spin", "x", |limits| limits.budget = 20_000_000).await;
        assert_eq!(failure(result), instruction_budget_error(20_000_000));
    }

    #[tokio::test]
    async fn each_marker_is_honoured_once() {
        let limits = Limits {
            budget: INSTRUCTION_BUDGET,
            startup: 0,
        };
        for name in ["fixture-restart", "fixture-early-stop"] {
            assert_eq!(
                failure(evaluate(name, "input").await),
                LimitFailure::Markers.message(limits),
                "{name}"
            );
        }
    }

    #[tokio::test]
    async fn work_before_the_measured_region_is_rejected() {
        // The fixture solves from the raw input packet before the region
        // starts, at 200 instructions per input byte and more.
        let input = "x".repeat(4_000);
        let error = failure(evaluate("fixture-early-work", &input).await);
        assert!(
            error.starts_with("Startup instruction limit exceeded"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn startup_allowance_has_headroom() {
        let binary = fixture("fixture-count");
        let symbols = read_guest_symbols(&binary).unwrap();
        for (description, input) in [
            ("empty", String::new()),
            ("ascii", "line of text\n".repeat(100_000)),
            ("two-byte characters", "é".repeat(200_000)),
            ("mixed", "añb€c𝄞d\n".repeat(50_000)),
            // The costliest pattern found: every character boundary falls
            // off the validator's word-at-a-time path.
            ("sparse", "éaaaaa".repeat(50_000)),
        ] {
            let evaluation = evaluate("fixture-count", &input).await.unwrap();
            let limit = startup_limit(&symbols, input.as_bytes());
            assert!(
                evaluation.startup_instructions * 2 <= limit,
                "{description}: startup took {} of {limit} allowed instructions",
                evaluation.startup_instructions
            );
        }
    }

    #[tokio::test]
    async fn guest_faults_are_reported() {
        for (name, expected) in [
            ("fixture-trap", "Guest trapped: illegal instruction"),
            ("fixture-zbb", "Guest trapped: illegal instruction"),
            ("fixture-stack-overflow", "Guest stack overflow"),
            ("fixture-panic", "Guest panicked"),
        ] {
            let error = failure(evaluate(name, "input").await);
            assert!(error.starts_with(expected), "{name}: {error}");
        }
    }

    #[tokio::test]
    async fn text_answers_are_returned() {
        let owned = evaluate("fixture-text", "  abc,def\n").await.unwrap();
        assert_eq!(owned.actual, "fed,cba");

        let borrowed = evaluate("fixture-borrowed-text", "first line\nsecond line\n")
            .await
            .unwrap();
        assert_eq!(borrowed.actual, "first line");
    }
}
