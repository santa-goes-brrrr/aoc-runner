# Santa goes brrrr

Advent of Code solutions optimized for the fewest executed RISC-V instructions.

<p align="center">
  <img src="santa-goes-brriscv.png" alt="Santa Goes Brrrr" width="50%">
</p>

Solutions run without an operating system under QEMU on a fixed vector-capable target:

- ISA baseline: **RV64GCV**
- Vector length: **VLEN=256**
- Maximum element width: **ELEN=64**
- Guest RAM: 256 MiB
- Per-solution instruction budget: 2,000,000,000
- Host watchdog: 10 seconds without guest progress
- Score: architectural instructions retired inside the solver region

## Requirements

- Rust 1.99.0 with `riscv64gc-unknown-none-elf`, pinned by `rust-toolchain.toml` and installed by rustup on first use
- QEMU system emulator with RISC-V and plugin support (developed against QEMU 11.1)
- `qemu-plugin.h`, CMake, a C compiler, `pkg-config`, and GLib development headers
- `nm` from binutils or Xcode Command Line Tools

On macOS, Homebrew's QEMU package includes both the RISC-V system emulator and
plugin header; install the host dependencies with:

```bash
xcode-select --install
brew install qemu cmake pkgconf glib
rustup target add riscv64gc-unknown-none-elf
```

On Arch Linux and pacman-based derivatives, install the host dependencies with:

```bash
sudo pacman -S --needed rustup qemu-system-riscv cmake gcc pkgconf glib2 binutils
rustup default stable
rustup target add riscv64gc-unknown-none-elf
```

The Arch `qemu-system-riscv` package pulls in `qemu-common`, which provides
`qemu-plugin.h`.

Other supported Linux package managers:

```bash
# Debian / Ubuntu (APT)
sudo apt update
sudo apt install qemu-system-misc qemu-system-common cmake build-essential pkg-config libglib2.0-dev binutils

# Fedora / RHEL family (DNF)
sudo dnf install qemu-system-riscv-core qemu-common cmake gcc pkgconf-pkg-config glib2-devel binutils

# Alpine Linux (apk)
sudo apk add qemu-system-riscv64 qemu-dev cmake build-base pkgconf glib-dev binutils

# After installing host packages
rustup target add riscv64gc-unknown-none-elf
```

The CLI detects pacman, APT, DNF, and apk and includes the matching commands in
dependency pre-flight failures. Package availability can vary on older distribution
releases.

Before any `evaluate` command compiles a solution, the CLI checks Cargo, Rust and the RISC-V target, `qemu-system-riscv64` with plugin support, CMake, a C compiler, `pkg-config`, GLib, `nm`, `qemu-plugin.h`, and a complete plugin build. A failed check lists each missing dependency, an individual fix, platform setup commands, and this requirements section. The successful pre-flight configures and incrementally builds `evaluator/roi-counter.c` with CMake. Its CMake configuration allows QEMU to resolve the plugin API at load time on macOS, as required by Darwin's linker.

## Agent skill

`SKILL.md` teaches coding agents to operate the CLI. It covers solution records, evaluation commands, failure handling, and data safety. `AGENTS.md` remains the developer guide for changing the project. Pi users can load the operator guide with `pi --skill ./SKILL.md`.

## Evaluating solutions

```bash
# Evaluate one solution and verify its answer
cargo run --package cli --release -- evaluate one --year 2025 --day 1 --part 1

# Evaluate every solution in parallel
cargo run --package cli --release -- evaluate all

# Limit host concurrency when needed
cargo run --package cli --release -- evaluate all --jobs 4
```

The CLI works from any directory inside the repository.

`evaluate all` compiles all requested binaries in one Cargo invocation, allowing Cargo to schedule compilation in parallel, and then runs independent QEMU processes concurrently. A database record without a source file fails on its own, and source files without a database record are listed as not evaluated. QEMU defaults to one worker per available host thread; use `--jobs N` to cap it, with each active worker reserving 256 MiB of guest RAM. Cargo uses `--keep-going`, so successfully built solutions are still evaluated when another binary fails to compile, although the overall command reports an error. Compilation messages remain append-only. On an interactive terminal, each active solution gets a logarithmic progress bar on its first non-zero update; one bar step is a doubling of the instruction count, starting at 100,000, and its final result replaces the bar message instead of adding another line. Progress bars are hidden and results remain line-oriented when output is redirected.

## How evaluation works

1. The CLI cross-compiles a solution for `riscv64gc-unknown-none-elf` with RVV enabled.
2. It creates 256 MiB of file-backed guest RAM.
3. QEMU loads a versioned input packet at guest address `0x8c000000`.
4. The bare-metal runtime validates the packet and passes a zero-copy `&str` to the solver.
5. A QEMU TCG plugin counts instructions between the ROI symbols, publishes progress, and fails the run if it exceeds 2 billion instructions, executes a marker twice, or does more before the region than the runtime needs to start.
6. The runtime writes the answer to a mailbox at `0x8d000000` and exits through QEMU's test-finisher device. A trap, panic, or stack overflow is recorded and reported instead.
7. The host reads the mailbox, hashes the returned answer, and compares it with the hash in `aoc.db`.

Parsing, allocation, solver logic, and called library code are measured. Boot, input validation, mailbox writes, and shutdown are excluded. Every limit that decides a result is an instruction count, so results do not depend on host speed. The wall-clock watchdog fires only when the guest executes nothing for 10 seconds.

The emulated CPU is QEMU's bare `rv64i` model with exactly the RV64GCV extensions enabled, so instructions outside the compile target trap. To keep scores comparable between machines, the toolchain is pinned and the CLI builds guests with host-specific source paths remapped to fixed names. See `AGENTS.md` for why that matters.

## Writing a solution

Each year is one Cargo package and each puzzle part is an independently compiled binary crate:

```text
solutions/year=2025/
├── Cargo.toml
└── src/bin/
    ├── aoc-2025-01-1.rs
    ├── aoc-2025-01-2.rs
    └── ...
```

Solutions are `no_std + alloc` bare-metal programs while retaining a generated native `main` for local runs and tests. Name each binary's entry function `solve`; the shared macro selects the correct entry point:

```rust
#![cfg_attr(target_os = "none", no_std, no_main)]

aoc_runtime::solution!(solve);

fn solve(input: &str) -> u64 {
    // Parsing and computation are scored.
    0
}
```

Dependencies are declared once in the year's `Cargo.toml`. Build or run a native solution directly with `cargo run --package aoc-2025 --bin aoc-2025-01-1`.

Supported answer types, through `aoc_runtime::IntoAnswer`, are the Rust signed and unsigned integer primitives and, for puzzles whose answer is text, `&'static str` and `String`. The answer must exist before `solve` returns; conversion into the mailbox representation occurs before the ROI ends.

`solution!` takes the name of a function. All work must happen inside it: the evaluator fails a run that executes an ROI marker itself or does work before the measured region.

Any pure-Rust crate that compiles with `no_std + alloc` is permitted. Native libraries, FFI dependencies, OS APIs, subprocesses, filesystem access, and networking are not available in the guest. Handwritten RVV assembly is permitted.

## Build and test

```bash
# All tests, including end-to-end tests of the evaluator under QEMU
cargo test --workspace --release

# Compile-check every bare-metal program (scores come only from the CLI's own builds)
cargo build --workspace --exclude cli --release \
  --target riscv64gc-unknown-none-elf

# Build only the host CLI
cargo build --package cli --release
```

## Solution database commands

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

`solution create` stores the input text and a domain-separated SHA-256 hash of the trimmed expected answer. The update commands replace one field of an existing entry and fail if that entry does not exist.

A hash prevents directly reading an answer from the database, but small numeric answers may still be found by offline brute force. Puzzle inputs in `aoc.db` remain personal. Do not publish `aoc.db`, input packets, RAM images, or files under `inputs/`.
