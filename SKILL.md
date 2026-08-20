---
name: santa-goes-brrrr
description: Operate the santa-goes-brrrr CLI to create, inspect, update, and delete Advent of Code solution records, and to evaluate one or all solutions by executed RV64GCV instruction count. Use when asked to run the evaluator or manage its puzzle inputs and expected-answer hashes. Do not use this skill to develop or modify the CLI itself.
compatibility: Run from the santa-goes-brrrr repository root on Linux or macOS with Rust, QEMU, CMake, pkg-config, GLib headers, qemu-plugin.h, and nm installed.
---

# Use the santa-goes-brrrr CLI

Use the CLI as the interface to the database and evaluator. Do not edit `aoc.db` directly, invoke QEMU yourself, or call the evaluator plugin manually.

Run every command from the repository root. Use long option names in commands.

## Check the CLI

```bash
cargo run --package cli --release -- --help
```

The top-level commands are `solution` and `evaluate`.

## Manage solution records

A record is identified by `--year`, `--day`, and `--part`. Part 1 and part 2 have separate records, even when they use the same puzzle input.

List existing records before creating or updating one:

```bash
cargo run --package cli --release -- solution list
```

Initialize the database schema when setting up a new checkout:

```bash
cargo run --package cli --release -- solution init
```

### Create a record

Creation requires an input file and the expected numeric answer:

```bash
cargo run --package cli --release -- solution create \
  --year 2025 --day 1 --part 1 \
  --input input.txt \
  --solution 123456
```

The CLI stores the input text and a domain-separated SHA-256 hash of the trimmed expected answer. It does not store the plaintext answer. Creation fails if the record already exists.

### Update an input

```bash
cargo run --package cli --release -- solution update input \
  --year 2025 --day 1 --part 1 \
  --input replacement.txt
```

This command fails if the record does not exist.

### Update an expected-answer hash

Pass the expected answer, not a precomputed hash:

```bash
cargo run --package cli --release -- solution update hash \
  --year 2025 --day 1 --part 1 \
  --solution 654321
```

The CLI computes and stores the hash. This command fails if the record does not exist.

### Read a record

```bash
cargo run --package cli --release -- solution read hash \
  --year 2025 --day 1 --part 1

cargo run --package cli --release -- solution read input \
  --year 2025 --day 1 --part 1
```

Only print a puzzle input when the user explicitly asks for it. Puzzle inputs are personal data.

### Delete records

Delete one record only when the user asks:

```bash
cargo run --package cli --release -- solution delete \
  --year 2025 --day 1 --part 1
```

`solution reset` drops every solution record. Do not run it without explicit confirmation.

## Evaluate solutions

A solution needs both a database record and a source file at:

```text
solutions/year=YYYY/src/bin/aoc-YYYY-DD-P.rs
```

Evaluate one part:

```bash
cargo run --package cli --release -- evaluate one \
  --year 2025 --day 1 --part 1
```

Evaluate every registered part:

```bash
cargo run --package cli --release -- evaluate all
```

Limit parallel QEMU workers when host memory is constrained. Each worker reserves 256 MiB of guest RAM:

```bash
cargo run --package cli --release -- evaluate all --jobs 4
```

The CLI first checks all host evaluator dependencies and reports installation guidance for anything missing. It then builds the required RISC-V binaries and QEMU plugin automatically. Do not run separate Cargo, CMake, or QEMU commands first.

## Interpret results

A passing result reports `Correct after N instructions`. A wrong answer reports `Wrong answer after N instructions` without exposing the returned value.

`evaluate all` ends with output like:

```text
Run completed in 3492314268 instructions
22 passed, 1 errors, 0 timeouts, 0 over budget
```

Treat a nonzero process exit as a failed evaluation. Report build errors, wrong answers, timeouts, and instruction-budget failures separately. Do not describe a partial bulk run as successful.

The solver budget is 2,000,000,000 instructions. `Timeout` is the 10-second host watchdog failure. Interactive progress updates appear in place; redirected output remains line-oriented.

## Protect local data

- Do not commit or publish `aoc.db`.
- Do not copy puzzle inputs into logs, issues, or responses unless requested.
- Do not attempt to recover expected answers from stored hashes.
- Do not modify solution source code unless the user asks for development work. `AGENTS.md` contains the developer instructions for that work.
