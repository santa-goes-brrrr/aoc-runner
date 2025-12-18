# AGENTS.md

## Project Overview

Advent of Code solutions repository optimized for performance. Solutions are written in Rust 2024 edition.

### Structure

```
advent-of-performance/
├── aoc.db                    # libSQL database with all inputs and solutions
├── cli/                      # CLI tool for managing the database and running solutions
├── inputs/                   # Backup of input/output files
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

```bash
# Initialize the database (create tables)
cargo run -p cli -- init

# Reset the database (delete all data)
cargo run -p cli -- reset

# List all solutions
cargo run -p cli -- list

# Run a solution and verify against expected output
cargo run -p cli -- run -y 2025 -d 1 -p 1

# Run all solutions
cargo run -p cli -- run-all

# Get input for a puzzle
cargo run -p cli -- read-input -y 2025 -d 1 -p 1

# Get expected solution
cargo run -p cli -- read-solution -y 2025 -d 1 -p 1

# Update a solution (input piped from stdin)
cargo run -p cli -- read-input -y 2025 -d 1 -p 1 | \
  cargo run -p cli -- upsert -y 2025 -d 1 -p 1 -s "answer"

# Delete an entry
cargo run -p cli -- delete -y 2025 -d 1 -p 1
```

Short args: `-y` (year), `-d` (day), `-p` (part), `-s` (solution)

## Code Style Guidelines

- Rust 2024 edition
- Optimize for performance over readability when needed
- Use workspace dependencies from root `Cargo.toml` when available: `ahash`, `pathfinding`, `petgraph`, `z3`
- Release profile uses `opt-level = 3` and `lto = true`

## Testing Instructions

Each solution should produce output matching the expected solution in the database:

```bash
# Run and verify a single solution
cargo run -p cli -- run -y 2025 -d 1 -p 1

# Run and verify all solutions
cargo run -p cli -- run-all
```

## Security Considerations

- Puzzle inputs in `aoc.db` are personal to the user (from adventofcode.com)
- Do not commit `aoc.db` or `inputs/` to public repositories
- The database has no authentication (file-system permissions only)
