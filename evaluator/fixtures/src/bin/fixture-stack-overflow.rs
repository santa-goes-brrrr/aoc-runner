//! Recurses without bound until the stack runs into the guard below it.
#![cfg_attr(target_os = "none", no_std, no_main)]

aoc_runtime::solution!(solve);

#[inline(never)]
fn recurse(depth: u64) -> u64 {
    let frame = core::hint::black_box([depth; 64]);
    if core::hint::black_box(depth) == u64::MAX {
        return frame[0];
    }
    recurse(depth + 1).wrapping_add(frame[1])
}

fn solve(input: &str) -> u64 {
    recurse(input.len() as u64)
}
