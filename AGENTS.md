# AGENTS.md

ALWAYS KEEP THIS FILE AND README.md UPDATED

## Project Overview

Advent of Code solutions optimized for executed RISC-V instruction count. Rust 2024 solutions are cross-compiled as `no_std + alloc` bare-metal RV64GCV programs and evaluated under QEMU system emulation.

This file documents development of the CLI, runtime, evaluator, and solutions. `SKILL.md` is the separate operator guide for agents that only need to use the CLI.

### Fixed evaluator

- Toolchain: Rust 1.99.0, pinned in `rust-toolchain.toml`
- Target: `riscv64gc-unknown-none-elf` with `+v,+zvl256b`
- QEMU CPU: the bare `rv64i` model with exactly `m,a,f,d,c,v,zicsr,zifencei,pmp` enabled, `vlen=256,elen=64`
- QEMU machine: `virt` with `-nodefaults`, one vCPU, no guest OS
- Guest RAM: 256 MiB at `0x80000000`
- Runtime boot and trap code: `0x8bee0000`; stack guard: `0x8bef0000`; stack: 1 MiB below `0x8c000000`
- Input packet: `0x8c000000`, maximum 16 MiB including header
- Output mailbox: `0x8d000000`; trap record: `0x8d000040`
- Per-solution instruction budget: 2,000,000,000
- Startup allowance before the measured region: 1,024 instructions, plus one per `.bss` byte, plus one per input byte (24 per byte when the input is not pure ASCII)
- Shutdown allowance after the measured region: 1,000,000 instructions
- Host watchdog: 10 seconds without a single executed guest instruction
- Score: instructions executed after `__aoc_roi_start` returns and before the call to `__aoc_roi_end`

Vector instructions count once regardless of VL or LMUL. QEMU cycles are not used.

The CPU model starts from bare `rv64i` because QEMU's `rv64` model enables extensions (Zba, Zbb, Zbc, Zbs, Zfa, and others) that vary between QEMU versions and are not part of the compile target. An instruction outside the target traps.

Every limit that decides a result is an instruction count, so a result never depends on host speed or load. The wall-clock watchdog fires only when the guest stops executing instructions altogether.

### Score stability

A guest's behaviour can depend on its own addresses. The solutions that use `hashbrown`'s default hasher are an example: it seeds itself from the addresses of a stack variable, a function, and a static, so moving code or data by a few bytes changes their instruction counts, sometimes by several percent. The evaluator therefore removes the host-specific inputs to the guest image:

- The toolchain is pinned to an exact version.
- The CLI builds guests with `--remap-path-prefix` for the Cargo home and the toolchain's `rust-src` directory. rustc embeds those paths in panic locations, and their length would otherwise shift everything after them. Guests built by a plain `cargo build` are not remapped and can score differently; only scores from the CLI are comparable.
- Runtime boot and trap code live in their own memory region (`.aoc_boot`, see `runtime/linker.ld`), and the entry stub in the solution image is a fixed 68 bytes. Changing that code never moves solution code or data.
- The solver is inlined into `aoc_runtime::run`. Editing the body of `run` can change the code the compiler generates for every solver, and with it every score. Put new runtime work in the boot code or on the host instead.

Changing any solution's own code still moves its own layout, so an address-dependent solution's score changes with unrelated edits to that solution. A fixed-seed hasher in the solution removes that noise.

### Structure

```text
├── .cargo/config.toml        # RISC-V target features and linker script
├── aoc.db                    # libSQL puzzle inputs and answer hashes
├── cli/                      # Host database and QEMU evaluator CLI
├── evaluator/                # QEMU TCG instruction-counting plugin
│   └── fixtures/             # Guest programs with known behaviour for the evaluator tests
├── runtime/                  # Shared no_std startup, allocator, ROI, mailbox
├── SKILL.md                  # Agent guide for operating the CLI
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
- name its entry function `solve` and register it with `aoc_runtime::solution!(solve)`; the macro accepts only a path to a function, never an expression
- accept the puzzle input as `&str`
- return a type implementing `aoc_runtime::IntoAnswer`: any integer primitive, or text as `&'static str` or `String` for puzzles whose answer is not a number
- perform all answer computation inside `solve`, before returning
- never call `__aoc_roi_start` or `__aoc_roi_end`, define its own entry point, or otherwise run code outside the measured region

Any Rust crate that supports `no_std + alloc` is allowed and should be declared in the year's shared `Cargo.toml`. Native code, FFI, dynamic linking, OS APIs, filesystem/network access, threads, and subprocesses are forbidden/unavailable. Handwritten RVV is allowed.

The measured region includes parsing, allocation, dependencies, and the solver. Runtime startup, input packet validation, answer mailbox writes, and QEMU shutdown are excluded.

The evaluator enforces the boundary. Each ROI marker is honoured once and in order, and executing either again fails the run. Instructions executed before the region are counted against the startup allowance, so work moved ahead of the region fails the run instead of disappearing from the score. What the evaluator cannot detect is an answer that was not computed at all: each puzzle has a single input, so a hard-coded answer passes. Review solutions for that.

## Build and test commands

```bash
# Run all tests: native unit tests plus the evaluator's end-to-end tests,
# which run the guests in evaluator/fixtures under QEMU
cargo test --workspace --release

# Compile-check all guest programs (not remapped: do not score these builds)
cargo build --workspace --exclude cli --release \
  --target riscv64gc-unknown-none-elf

# Evaluate one solution
cargo run --package cli --release -- evaluate one --year 2025 --day 1 --part 1

# Evaluate all solutions in parallel (defaults to available host threads)
cargo run --package cli --release -- evaluate all

# Limit QEMU concurrency when needed
cargo run --package cli --release -- evaluate all --jobs 4

# Build the host CLI
cargo build --package cli --release

# Reconfigure and rebuild the QEMU plugin manually
cmake -S evaluator -B target/aoc-evaluator \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
cmake --build target/aoc-evaluator --target roi-counter
```

Every evaluation starts with a host dependency pre-flight before opening the database or compiling solutions. It checks Cargo, Rust and the RISC-V target, QEMU RISC-V system emulation and plugin support, CMake, a C compiler, `pkg-config`, GLib, `nm`, and a complete QEMU plugin configure/build. Failures aggregate actionable dependency-specific guidance and platform setup commands, including exact Homebrew commands on macOS and commands for pacman, APT, DNF, and apk on Linux. A successful pre-flight leaves the CMake plugin build ready for all evaluator workers.

The CLI finds the repository root from the directory it is started in and works from there, so it runs from any subdirectory and refuses to run elsewhere. `--input` paths are relative to the directory the CLI was started in.

Bulk runs compile all requested binaries in one `cargo build --keep-going` invocation so Cargo can schedule compilation in parallel, then evaluate successful artifacts in bounded parallel QEMU workers. A failed binary is reported without preventing successful binaries from running, but the overall command still fails. A database record whose source file is missing is reported as a failure of that record alone, and solution sources without a record are listed as not evaluated. An empty database is an error. Each QEMU worker uses one vCPU and 256 MiB of guest RAM; concurrency defaults to available host threads and can be capped with `--jobs`. Compilation messages are append-only. Interactive instruction progress lazily creates in-place multi-progress bars on the first non-zero update, starting at 100,000 instructions and doubling per bar increment; each final result replaces its bar message. Bars are hidden and results remain line-oriented for redirected output.

A solution is successful only when the guest stays within the startup allowance, the 2-billion-instruction budget, and the shutdown allowance, executes each ROI marker exactly once, exits without a trap or panic, and returns an answer whose host-computed hash matches the database. Guest traps are reported with their cause and program counter, and a stack overflow is reported as such.

SIGINT, SIGTERM, and SIGHUP cancel the running command: QEMU workers are killed and their run directories removed before the CLI exits with `128 + signal`. If the CLI is killed outright, each QEMU notices that its parent is gone and exits, and the next evaluation removes the run directories left behind.

## Host/guest protocol

The CLI writes a little-endian input packet containing magic `AOCIN001`, protocol version, flags, input length, and UTF-8 bytes. QEMU's generic loader places it in guest RAM.

The runtime writes output magic `AOCOUT01`, protocol version, state, answer kind, and a 128-bit answer into file-backed guest RAM. It writes the completed state last, fences memory, then exits via the QEMU `virt` test-finisher. The host reads the mailbox only after QEMU exits. Expected answers are never provided to the guest. For a text answer the two answer words hold the guest address and byte length of the text, which the host reads from the RAM image (at most 1,024 bytes).

The boot code installs a trap handler and locks a 64 KiB PMP guard below the stack. On any CPU exception the handler writes a trap record (`AOCTRAP1`, version, `mcause`, `mepc`, `mtval`) at `0x8d000040`, sets the mailbox state to trapped, and exits through the test-finisher with a failure code. The host reports the cause, or a stack overflow when the faulting address lies in the guard.

The TCG plugin receives the ROI addresses, the instruction budget, the startup allowance, and the CLI's process id. It counts with inline TCG operations, one addition per translation block, and corrects for the position of a marker inside its block, so the count is exact as long as no block is left early; the only way to leave one early is a trap, which fails the run. A callback folds the counter and checks the limits every 1,048,576 instructions. Limits checked there allow 4,096 instructions of slack for the block in progress, and the exact comparison happens when a marker is reached. At exit the plugin writes `state`, `instructions`, and `startup_instructions` to a host-side score file. During execution it updates a 32-byte little-endian shared progress mailbox (`AOCPROG1`, version, state, ROI instruction count, total executed instructions). The host polls this mailbox every 25 ms, updates interactive progress bars in place, terminates workers whose state reports a broken limit, and applies the stall watchdog to the total executed count. A thread in the plugin ends QEMU when the CLI process that started it no longer exists. On macOS, CMake passes Darwin's `-undefined dynamic_lookup` linker option because QEMU resolves the plugin API when loading the module; Homebrew's QEMU package supplies `qemu-system-riscv64` and `qemu-plugin.h`.

## CLI solution database commands

```bash
cargo run --package cli -- solution init
cargo run --package cli -- solution reset
cargo run --package cli -- solution list
cargo run --package cli -- solution create --year 2025 --day 1 --part 1 --input input.txt --solution 123456
cargo run --package cli -- solution update input --year 2025 --day 1 --part 1 --input replacement.txt
cargo run --package cli -- solution update hash --year 2025 --day 1 --part 1 --solution 654321
cargo run --package cli -- solution read input --year 2025 --day 1 --part 1
cargo run --package cli -- solution read hash --year 2025 --day 1 --part 1
cargo run --package cli -- solution delete --year 2025 --day 1 --part 1
```

Options: `--year`, `--day`, `--part`, `--input`, and `--solution`. `--solution` is the expected answer, a number or text. Creation stores a domain-separated SHA-256 answer hash; updates fail if the entry does not exist.

## Code style

- Rust 2024 edition
- Optimize for instruction count and RVV use
- Keep the compiler, target, linker, allocator, runtime, QEMU model, and plugin deterministic
- Keep protocol structures versioned and assert/document their layouts
- Use workspace dependencies when shared

## Security and privacy

- `aoc.db` and puzzle inputs are personal; do not publish them
- Expected answers are stored only as domain-separated SHA-256 hashes; small numeric answers may still be brute-forced offline
- Runtime input packets and RAM images live under ignored `target/` paths and are deleted after runs, including interrupted ones; leftovers of a killed CLI are removed by the next evaluation
- The guest has no semihosting and receives no expected answer
- The evaluator executes arbitrary guest machine code; use trusted solutions and keep QEMU updated
