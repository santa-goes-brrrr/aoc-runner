//! Panics inside the measured region.
#![cfg_attr(target_os = "none", no_std, no_main)]

aoc_runtime::solution!(solve);

fn solve(input: &str) -> u64 {
    let values = [1_u64, 2, 3];
    values[core::hint::black_box(input.len() + values.len())]
}
