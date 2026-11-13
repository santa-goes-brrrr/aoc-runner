//! Returns a text answer borrowed from the input: its first line.
#![cfg_attr(target_os = "none", no_std, no_main)]

aoc_runtime::solution!(solve);

fn solve(input: &'static str) -> &'static str {
    input.lines().next().unwrap_or("")
}
