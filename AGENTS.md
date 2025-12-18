# AGENTS.md

ALWAYS KEEP THIS FILE AND README.md UPDATED

## Project Overview

Advent of Code solutions repository optimized for performance. Solutions are written in Rust 2024 edition.

### Structure

```
advent-of-performance/
├── aoc.db                    # libSQL database with all inputs and solutions
├── cli/                      # CLI tool for managing the database and running solutions
├── docs/                     # Documentation assets (demo.gif, demo.cast)
├── inputs/                   # Backup of input/output files
├── README.md                 # Project readme
└── solutions/
    └── year=YYYY/
        └── day=DD/
            └── part=P/
                ├── Cargo.toml
                └── src/main.rs
```

Solutions use Hive-style partitioning: `solutions/year=YYYY/day=DD/part=P/`

### Database

All puzzle inputs and expected outputs are stored in `aoc.db` (libSQL):

```sql
CREATE TABLE solutions (
    year INTEGER NOT NULL,
    day INTEGER NOT NULL,
    part INTEGER NOT NULL,
    input TEXT NOT NULL,
    output TEXT NOT NULL,
    PRIMARY KEY (year, day, part)
)
```

## Build and Test Commands

```bash
# Build all solutions
cargo build --release

# Run a specific solution
cargo run -p aoc-2025-01-1 --release

# Build the CLI
cargo build -p cli
```

### CLI

The CLI uses libSQL for database access and rayon for parallel benchmarking.

```bash
# Run a solution and verify against expected output (10 benchmark runs by default)
cargo run -p cli -- run one -y 2025 -d 1 -p 1

# Run with custom number of benchmark iterations
cargo run -p cli -- run one -y 2025 -d 1 -p 1 -r 20

# Run all solutions
cargo run -p cli -- run all

# Run all with custom iterations
cargo run -p cli -- run all -r 5

# Compare performance between current branch and main
cargo run -p cli -- compare

# Compare with custom iterations
cargo run -p cli -- compare -r 20
```

#### Compare Command

The `compare` command benchmarks all solutions on the current branch, then switches to `main`, benchmarks again, and prints a side-by-side comparison report. If already on `main`, it outputs "Nothing to compare".

The report shows:
- Per-solution timing on both branches
- Absolute and percentage difference
- Summary of improvements (▼) and regressions (▲)

#### Database Commands

All database operations are under the `db` subcommand:

```bash
# Initialize the database (create tables)
cargo run -p cli -- db init

# Reset the database (delete all data)
cargo run -p cli -- db reset

# List all solutions
cargo run -p cli -- db list

# Get input for a puzzle
cargo run -p cli -- db read input -y 2025 -d 1 -p 1

# Get expected solution
cargo run -p cli -- db read solution -y 2025 -d 1 -p 1

# Update a solution (input piped from stdin)
cargo run -p cli -- db read input -y 2025 -d 1 -p 1 | \
  cargo run -p cli -- db upsert -y 2025 -d 1 -p 1 -s "answer"

# Delete an entry
cargo run -p cli -- db delete -y 2025 -d 1 -p 1
```

Short args: `-y` (year), `-d` (day), `-p` (part), `-s` (solution), `-r` (runs)

## Code Style Guidelines

- Rust 2024 edition
- Optimize for performance over readability when needed
- Use workspace dependencies from root `Cargo.toml` when available: `ahash`, `pathfinding`, `petgraph`, `z3`
- Release profile uses `opt-level = 3` and `lto = true`

## Testing Instructions

Each solution should produce output matching the expected solution in the database. Solutions have a 1 second timeout. Benchmarking runs solutions multiple times in parallel using rayon.

```bash
# Run and verify a single solution
cargo run -p cli -- run one -y 2025 -d 1 -p 1

# Run and verify all solutions
cargo run -p cli -- run all
```

Output uses colored indicators:
- **Green** (✓): Correct solution with timing (mean ± spread)
- **Yellow** (✗): Timeout (exceeded 1 second)
- **Red** (✗): Build error, runtime error, or wrong answer

Progress is shown during execution:
- `{label} Compiling...`
- `{label} Verifying...`
- `{label} Benchmarking ({runs} runs)...`

## Security Considerations

- Puzzle inputs in `aoc.db` are personal to the user (from adventofcode.com)
- Do not commit `aoc.db` or `inputs/` to public repositories
- The database has no authentication (file-system permissions only)
