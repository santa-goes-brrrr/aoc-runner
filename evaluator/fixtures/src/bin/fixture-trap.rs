//! Executes an all-zero word, which is an illegal instruction.
#![cfg_attr(target_os = "none", no_std, no_main)]

aoc_runtime::solution!(solve);

fn solve(input: &str) -> u64 {
    // SAFETY: deliberately faults; the runtime's trap handler takes over.
    #[cfg(target_os = "none")]
    unsafe {
        core::arch::asm!(".4byte 0", options(nomem, nostack))
    };
    input.len() as u64
}
