//! Uses `cpop` from Zbb, which is not part of the RV64GCV evaluator target.
#![cfg_attr(target_os = "none", no_std, no_main)]

aoc_runtime::solution!(solve);

fn solve(input: &str) -> u64 {
    count_ones(core::hint::black_box(input.len() as u64))
}

#[cfg(target_os = "none")]
fn count_ones(value: u64) -> u64 {
    let bits: u64;
    // SAFETY: a register-only instruction; it traps on the evaluator CPU.
    unsafe {
        core::arch::asm!(
            ".option push",
            ".option arch, +zbb",
            "cpop {bits}, {value}",
            ".option pop",
            value = in(reg) value,
            bits = out(reg) bits,
            options(nomem, nostack),
        );
    }
    bits
}

#[cfg(not(target_os = "none"))]
fn count_ones(value: u64) -> u64 {
    u64::from(value.count_ones())
}
