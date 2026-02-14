use clap::{Parser, Subcommand};
use libsql::{Builder, Connection};
use std::fs;
use std::io::{self, Read};

#[derive(Parser)]
#[command(name = "cli")]
#[command(about = "Advent of Code CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Database operations (init, reset, list, upsert, delete, read)
    Db {
        #[command(subcommand)]
        command: DbCommands,
    },
}

#[derive(Subcommand)]
enum DbCommands {
    /// Initialize the database (create tables)
    Init,
    /// Reset the database (delete all data)
    Reset,
    /// List all solutions
    List,
    /// Insert or update data
    Upsert {
        #[command(subcommand)]
        command: UpsertCommands,
    },
    /// Delete a solution
    Delete {
        year: u16,
        day: u8,
        part: u8,
    },
    /// Read data from the database
    Read {
        #[command(subcommand)]
        command: ReadCommands,
    },
}

#[derive(Subcommand)]
enum UpsertCommands {
    /// Upsert the input for a given year, day, part (reads from stdin by default)
    Input {
        year: u16,
        day: u8,
        part: u8,
        /// Value as a string
        #[arg(long, conflicts_with = "file")]
        value: Option<String>,
        /// Read value from a file
        #[arg(short, long, conflicts_with = "value")]
        file: Option<String>,
    },
    /// Upsert the expected output for a given year, day, part (reads from stdin by default)
    Output {
        year: u16,
        day: u8,
        part: u8,
        /// Value as a string
        #[arg(long, conflicts_with = "file")]
        value: Option<String>,
        /// Read value from a file
        #[arg(short, long, conflicts_with = "value")]
        file: Option<String>,
    },
}

#[derive(Subcommand)]
enum ReadCommands {
    /// Read the expected output for a given year, day, part
    Output {
        year: u16,
        day: u8,
        part: u8,
    },
    /// Read the input for a given year, day, part
    Input {
        year: u16,
        day: u8,
        part: u8,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let db_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("aoc.db");
    let db = Builder::new_local(db_path).build().await?;
    let conn = db.connect()?;

    match cli.command {
        Commands::Db { command } => match command {
            DbCommands::Init => {
                create_schema(&conn).await?;
                println!("Database initialized");
            }
            DbCommands::Reset => {
                reset_db(&conn).await?;
                println!("Database reset");
            }
            DbCommands::List => {
                list_solutions(&conn).await?;
            }
            DbCommands::Upsert { command } => match command {
                UpsertCommands::Input { year, day, part, value, file } => {
                    let data = read_upsert_value(value, file)?;
                    upsert_input(&conn, year, day, part, &data).await?;
                    println!("Upserted input: year={year}, day={day}, part={part}");
                }
                UpsertCommands::Output { year, day, part, value, file } => {
                    let data = read_upsert_value(value, file)?;
                    upsert_output(&conn, year, day, part, &data).await?;
                    println!("Upserted output: year={year}, day={day}, part={part}");
                }
            },
            DbCommands::Delete { year, day, part } => {
                delete_solution(&conn, year, day, part).await?;
                println!("Deleted: year={year}, day={day}, part={part}");
            }
            DbCommands::Read { command } => match command {
                ReadCommands::Output { year, day, part } => {
                    let solution = read_solution(&conn, year, day, part).await?;
                    print!("{solution}");
                }
                ReadCommands::Input { year, day, part } => {
                    let input = read_input(&conn, year, day, part).await?;
                    print!("{input}");
                }
            },
        },
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

fn read_upsert_value(value: Option<String>, file: Option<String>) -> Result<String, Box<dyn std::error::Error>> {
    match (value, file) {
        (Some(v), None) => Ok(v),
        (None, Some(path)) => Ok(fs::read_to_string(&path)?),
        (None, None) => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
        (Some(_), Some(_)) => unreachable!("clap prevents --value and --file together"),
    }
}

async fn upsert_input(
    conn: &Connection,
    year: u16,
    day: u8,
    part: u8,
    input: &str,
) -> Result<(), libsql::Error> {
    conn.execute(
        "INSERT INTO solutions (year, day, part, input, output) VALUES (?, ?, ?, ?, '')
         ON CONFLICT(year, day, part) DO UPDATE SET input = excluded.input",
        (year, day, part, input),
    )
    .await?;
    Ok(())
}

async fn upsert_output(
    conn: &Connection,
    year: u16,
    day: u8,
    part: u8,
    output: &str,
) -> Result<(), libsql::Error> {
    conn.execute(
        "INSERT INTO solutions (year, day, part, input, output) VALUES (?, ?, ?, '', ?)
         ON CONFLICT(year, day, part) DO UPDATE SET output = excluded.output",
        (year, day, part, output),
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

async fn delete_solution(conn: &Connection, year: u16, day: u8, part: u8) -> Result<(), libsql::Error> {
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
