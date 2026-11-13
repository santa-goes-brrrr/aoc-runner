//! Returns an owned text answer: the trimmed input reversed.
#![cfg_attr(target_os = "none", no_std, no_main)]

extern crate alloc;

use alloc::string::String;

aoc_runtime::solution!(solve);

fn solve(input: &str) -> String {
    input.trim().chars().rev().collect()
}
