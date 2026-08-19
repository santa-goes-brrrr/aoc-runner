# AGENTS.md

ALWAYS KEEP THIS FILE AND README.md UPDATED

## Project Overview

Advent of Code solutions optimized for executed RISC-V instruction count. Rust 2024 solutions are cross-compiled as `no_std + alloc` bare-metal RV64GCV programs and evaluated under QEMU system emulation.

### Fixed evaluator

- Target: `riscv64gc-unknown-none-elf` with `+v,+zvl256b`
- QEMU CPU: `rv64,v=true,vlen=256,elen=64`
- Guest RAM: 256 MiB at `0x80000000`
- Input packet: `0x8c000000`, maximum 16 MiB including header
- Output mailbox: `0x8d000000`
- Per-solution instruction budget: 2,000,000,000
- Host watchdog timeout: 10 seconds
- One vCPU; no guest OS
- Score: instructions between `__aoc_roi_start` and `__aoc_roi_end`

Vector instructions count once regardless of VL or LMUL. QEMU cycles are not used.

### Structure

```text
├── .cargo/config.toml        # RISC-V target features and linker script
├── aoc.db                    # libSQL puzzle inputs and answer hashes
├── cli/                      # Host database and QEMU evaluator CLI
├── evaluator/                # QEMU TCG instruction-counting plugin
├── runtime/                  # Shared no_std startup, allocator, ROI, mailbox
├── solutions/year=YYYY/       # One package per year
│   ├── Cargo.toml
│   └── src/bin/aoc-YYYY-DD-P.rs
├── Cargo.toml
└── rust-toolchain.toml
```

## Solution contract

Every solution must:

- compile for `riscv64gc-unknown-none-elf`
- start with `#![cfg_attr(target_os = "none", no_std, no_main)]`
- live at `solutions/year=YYYY/src/bin/aoc-YYYY-DD-P.rs`
- name its entry function `solve` and register it with `aoc_runtime::solution!(solve)`
- accept the puzzle input as `&str`
- return an integer type implementing `aoc_runtime::IntoAnswer`
- perform all answer computation before returning

Any Rust crate that supports `no_std + alloc` is allowed and should be declared in the year's shared `Cargo.toml`. Native code, FFI, dynamic linking, OS APIs, filesystem/network access, threads, and subprocesses are forbidden/unavailable. Handwritten RVV is allowed.

The measured region includes parsing, allocation, dependencies, and the solver. Runtime startup, input packet validation, answer mailbox writes, and QEMU shutdown are excluded.

## Build and test commands

```bash
# Run native unit tests
cargo test --workspace --release

# Cross-compile all guest solutions
cargo build --workspace --exclude cli --release \
  --target riscv64gc-unknown-none-elf

# Evaluate one solution
cargo run -p cli --release -- run one -y 2025 -d 1 -p 1

# Evaluate all solutions in parallel (defaults to available host threads)
cargo run -p cli --release -- run all

# Limit QEMU concurrency when needed
cargo run -p cli --release -- run all --jobs 4

# Compare current branch with main (also accepts --jobs)
cargo run -p cli --release -- compare

# Build the host CLI
cargo build -p cli --release

# Reconfigure and rebuild the QEMU plugin manually
cmake -S evaluator -B target/aoc-evaluator \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
cmake --build target/aoc-evaluator --target roi-counter
```

Bulk runs compile all requested binaries in one `cargo build --keep-going` invocation so Cargo can schedule compilation in parallel, then evaluate successful artifacts in bounded parallel QEMU workers. A failed binary is reported without preventing successful binaries from running, but the overall command still fails. Each QEMU worker uses one vCPU and 256 MiB of guest RAM; concurrency defaults to available host threads and can be capped with `--jobs`. Compilation messages are append-only. Interactive instruction progress lazily creates in-place multi-progress bars on the first non-zero update, starting at 100,000 instructions and doubling per bar increment; each final result replaces its bar message. Bars are hidden and results remain line-oriented for redirected output. A solution is successful only when it stays within the 2-billion-instruction budget, QEMU exits within the 10-second watchdog, the guest reports a complete ROI, and the host-computed answer hash matches the database.

## Host/guest protocol

The CLI writes a little-endian input packet containing magic `AOCIN001`, protocol version, flags, input length, and UTF-8 bytes. QEMU's generic loader places it in guest RAM.

The runtime writes output magic `AOCOUT01`, protocol version, state, signed/unsigned answer kind, and a 128-bit answer into file-backed guest RAM. It writes the completed state last, fences memory, then exits via the QEMU `virt` test-finisher. The host reads the mailbox only after QEMU exits. Expected answers are never provided to the guest.

The TCG plugin receives ROI addresses extracted from the ELF with `nm` and writes `completed` plus the final instruction count to a host-side score file. During execution it updates a 24-byte little-endian shared progress mailbox (`AOCPROG1`, version, state, instruction count) at doubling thresholds and when the instruction budget is exceeded. The host polls this mailbox every 25 ms, updates interactive progress bars in place, and terminates over-budget QEMU workers. The wall-clock watchdog remains a fallback for pre-ROI and infrastructure stalls.

## CLI database commands

```bash
cargo run -p cli -- db init
cargo run -p cli -- db reset
cargo run -p cli -- db list
cargo run -p cli -- db upsert -y 2025 -d 1 -p 1 -i input.txt -s 123456
cargo run -p cli -- db read input -y 2025 -d 1 -p 1
cargo run -p cli -- db read hash -y 2025 -d 1 -p 1
cargo run -p cli -- db delete -y 2025 -d 1 -p 1
```

Short arguments: `-y` year, `-d` day, `-p` part, `-i` input file, `-s` expected solution. Upserts store only a domain-separated SHA-256 hash.

## Code style

- Rust 2024 edition
- Optimize for instruction count and RVV use
- Keep the compiler, target, linker, allocator, runtime, QEMU model, and plugin deterministic
- Keep protocol structures versioned and assert/document their layouts
- Use workspace dependencies when shared

## Security and privacy

- `aoc.db` and puzzle inputs are personal; do not publish them
- Expected answers are stored only as domain-separated SHA-256 hashes; small numeric answers may still be brute-forced offline
- Runtime input packets and RAM images live under ignored `target/` paths and are deleted after runs
- The guest has no semihosting and receives no expected answer
- The evaluator executes arbitrary guest machine code; use trusted solutions and keep QEMU updated
