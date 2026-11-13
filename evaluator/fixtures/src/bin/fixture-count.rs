//! Runs a loop of exactly two instructions per input byte, so the measured
//! count for an input of N bytes is `2 * N` plus a constant.
#![cfg_attr(target_os = "none", no_std, no_main)]

aoc_runtime::solution!(solve);

fn solve(input: &str) -> u64 {
    let iterations = input.len() as u64;
    #[cfg(target_os = "none")]
    if iterations > 0 {
        // SAFETY: the loop only decrements a scratch register.
        unsafe {
            core::arch::asm!(
                "2:",
                "addi {counter}, {counter}, -1",
                "bnez {counter}, 2b",
                counter = inout(reg) iterations => _,
                options(nomem, nostack),
            );
        }
    }
    iterations
}
