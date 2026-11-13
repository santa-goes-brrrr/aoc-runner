//! Never returns: only the instruction budget can end this run.
#![cfg_attr(target_os = "none", no_std, no_main)]

aoc_runtime::solution!(solve);

fn solve(input: &str) -> u64 {
    let mut value = input.len() as u64;
    loop {
        value = core::hint::black_box(value.wrapping_add(1));
    }
}
