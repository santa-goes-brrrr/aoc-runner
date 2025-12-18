use clap::{Parser, Subcommand};
use libsql::{Builder, Connection};
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::time::Instant;

#[derive(Parser)]
#[command(name = "cli")]
#[command(about = "Advent of Code CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Insert or update a solution (input read from stdin)
    Upsert {
        #[arg(short, long)]
        year: u16,
        #[arg(short, long)]
        day: u8,
        #[arg(short, long)]
        part: u8,
        #[arg(short, long)]
        solution: String,
    },
    /// Read the solution/output for a given year, day, part
    ReadSolution {
        #[arg(short, long)]
        year: u16,
        #[arg(short, long)]
        day: u8,
        #[arg(short, long)]
        part: u8,
    },
    /// Read the input for a given year, day, part
    ReadInput {
        #[arg(short, long)]
        year: u16,
        #[arg(short, long)]
        day: u8,
        #[arg(short, long)]
        part: u8,
    },
    /// Delete a solution
    Delete {
        #[arg(short, long)]
        year: u16,
        #[arg(short, long)]
        day: u8,
        #[arg(short, long)]
        part: u8,
    },
    /// List all solutions
    List,
    /// Run a solution and verify against expected output
    Run {
        #[arg(short, long)]
        year: u16,
        #[arg(short, long)]
        day: u8,
        #[arg(short, long)]
        part: u8,
    },
    /// Run all solutions and verify against expected outputs
    RunAll,
    /// Initialize the database (create tables)
    Init,
    /// Reset the database (delete all data)
    Reset,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let db = Builder::new_local("aoc.db").build().await?;
    let conn = db.connect()?;

    match cli.command {
        Commands::Upsert {
            year,
            day,
            part,
            solution,
        } => {
            let mut input = String::new();
            io::stdin().read_to_string(&mut input)?;

            upsert_solution(&conn, year, day, part, &input, &solution).await?;
            println!("Upserted: year={year}, day={day}, part={part}, solution={solution}");
        }
        Commands::ReadSolution { year, day, part } => {
            let solution = read_solution(&conn, year, day, part).await?;
            print!("{solution}");
        }
        Commands::ReadInput { year, day, part } => {
            let input = read_input(&conn, year, day, part).await?;
            print!("{input}");
        }
        Commands::Delete { year, day, part } => {
            delete_solution(&conn, year, day, part).await?;
            println!("Deleted: year={year}, day={day}, part={part}");
        }
        Commands::List => {
            list_solutions(&conn).await?;
        }
        Commands::Run { year, day, part } => {
            let result = run_solution(&conn, year, day, part).await?;
            print_run_result(&result);
            if !result.passed {
                std::process::exit(1);
            }
        }
        Commands::RunAll => {
            let results = run_all_solutions(&conn).await?;
            let mut failed = 0;
            for result in &results {
                print_run_result(result);
                if !result.passed {
                    failed += 1;
                }
            }
            println!("\n{}/{} passed", results.len() - failed, results.len());
            if failed > 0 {
                std::process::exit(1);
            }
        }
        Commands::Init => {
            create_schema(&conn).await?;
            println!("Database initialized");
        }
        Commands::Reset => {
            reset_db(&conn).await?;
            println!("Database reset");
        }
    }

    Ok(())
}

async fn create_schema(conn: &Connection) -> Result<(), libsql::Error> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS solutions (
            year INTEGER NOT NULL,
            day INTEGER NOT NULL,
            part INTEGER NOT NULL,
            input TEXT NOT NULL,
            output TEXT NOT NULL,
            PRIMARY KEY (year, day, part)
        )",
        (),
    )
    .await?;
    Ok(())
}

async fn reset_db(conn: &Connection) -> Result<(), libsql::Error> {
    conn.execute("DROP TABLE IF EXISTS solutions", ()).await?;
    Ok(())
}

async fn upsert_solution(
    conn: &Connection,
    year: u16,
    day: u8,
    part: u8,
    input: &str,
    output: &str,
) -> Result<(), libsql::Error> {
    conn.execute(
        "INSERT OR REPLACE INTO solutions (year, day, part, input, output) VALUES (?, ?, ?, ?, ?)",
        (year, day, part, input, output),
    )
    .await?;
    Ok(())
}

async fn read_solution(
    conn: &Connection,
    year: u16,
    day: u8,
    part: u8,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut rows = conn
        .query(
            "SELECT output FROM solutions WHERE year = ? AND day = ? AND part = ?",
            (year, day, part),
        )
        .await?;
    if let Some(row) = rows.next().await? {
        Ok(row.get::<String>(0)?)
    } else {
        Err(format!("No solution found for year={year}, day={day}, part={part}").into())
    }
}

async fn read_input(
    conn: &Connection,
    year: u16,
    day: u8,
    part: u8,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut rows = conn
        .query(
            "SELECT input FROM solutions WHERE year = ? AND day = ? AND part = ?",
            (year, day, part),
        )
        .await?;
    if let Some(row) = rows.next().await? {
        Ok(row.get::<String>(0)?)
    } else {
        Err(format!("No input found for year={year}, day={day}, part={part}").into())
    }
}

async fn delete_solution(
    conn: &Connection,
    year: u16,
    day: u8,
    part: u8,
) -> Result<(), libsql::Error> {
    conn.execute(
        "DELETE FROM solutions WHERE year = ? AND day = ? AND part = ?",
        (year, day, part),
    )
    .await?;
    Ok(())
}

async fn list_solutions(conn: &Connection) -> Result<(), libsql::Error> {
    let mut rows = conn
        .query(
            "SELECT year, day, part, output FROM solutions ORDER BY year, day, part",
            (),
        )
        .await?;
    while let Some(row) = rows.next().await? {
        let year: u16 = row.get::<u32>(0)? as u16;
        let day: u8 = row.get::<u32>(1)? as u8;
        let part: u8 = row.get::<u32>(2)? as u8;
        let output: String = row.get(3)?;
        println!("{year}\t{day}\t{part}\t{output}");
    }
    Ok(())
}

struct RunResult {
    year: u16,
    day: u8,
    part: u8,
    passed: bool,
    expected: String,
    actual: String,
    duration_ms: u128,
    error: Option<String>,
}

fn print_run_result(result: &RunResult) {
    let status = if result.passed { "✓" } else { "✗" };
    let time_str = format!("{:.2}ms", result.duration_ms as f64);
    
    if result.passed {
        println!(
            "{} {}-{:02}-{} {} ({})",
            status, result.year, result.day, result.part, result.actual.trim(), time_str
        );
    } else if let Some(ref err) = result.error {
        println!(
            "{} {}-{:02}-{} ERROR: {}",
            status, result.year, result.day, result.part, err
        );
    } else {
        println!(
            "{} {}-{:02}-{} expected '{}', got '{}' ({})",
            status,
            result.year,
            result.day,
            result.part,
            result.expected.trim(),
            result.actual.trim(),
            time_str
        );
    }
}

async fn run_solution(
    conn: &Connection,
    year: u16,
    day: u8,
    part: u8,
) -> Result<RunResult, Box<dyn std::error::Error>> {
    // Get input and expected output from database
    let input = read_input(conn, year, day, part).await?;
    let expected = read_solution(conn, year, day, part).await?;

    // Build the package name
    let package_name = format!("aoc-{}-{:02}-{}", year, day, part);

    // Build the solution first
    let build_output = Command::new("cargo")
        .arg("build")
        .arg("-p")
        .arg(&package_name)
        .arg("--release")
        .output()?;

    if !build_output.status.success() {
        return Ok(RunResult {
            year,
            day,
            part,
            passed: false,
            expected,
            actual: String::new(),
            duration_ms: 0,
            error: Some(format!(
                "Build failed: {}",
                String::from_utf8_lossy(&build_output.stderr)
            )),
        });
    }

    // Run the solution
    let mut run_cmd = Command::new("cargo");
    run_cmd
        .arg("run")
        .arg("-p")
        .arg(&package_name)
        .arg("--quiet")
        .arg("--release")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let start = Instant::now();
    let mut child = run_cmd.spawn()?;

    // Write input to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    let duration_ms = start.elapsed().as_millis();

    if !output.status.success() {
        return Ok(RunResult {
            year,
            day,
            part,
            passed: false,
            expected,
            actual: String::new(),
            duration_ms,
            error: Some(format!(
                "Runtime error: {}",
                String::from_utf8_lossy(&output.stderr)
            )),
        });
    }

    let actual = String::from_utf8_lossy(&output.stdout).to_string();
    let passed = actual.trim() == expected.trim();

    Ok(RunResult {
        year,
        day,
        part,
        passed,
        expected,
        actual,
        duration_ms,
        error: None,
    })
}

async fn run_all_solutions(
    conn: &Connection,
) -> Result<Vec<RunResult>, Box<dyn std::error::Error>> {
    let mut results = Vec::new();

    let mut rows = conn
        .query(
            "SELECT year, day, part FROM solutions ORDER BY year, day, part",
            (),
        )
        .await?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next().await? {
        let year: u16 = row.get::<u32>(0)? as u16;
        let day: u8 = row.get::<u32>(1)? as u8;
        let part: u8 = row.get::<u32>(2)? as u8;
        entries.push((year, day, part));
    }

    for (year, day, part) in entries {
        let result = run_solution(conn, year, day, part).await?;
        results.push(result);
    }

    Ok(results)
}
