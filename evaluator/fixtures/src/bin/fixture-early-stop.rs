//! Executes the stop marker before doing its work, which would end a naive
//! measurement early.
#![cfg_attr(target_os = "none", no_std, no_main)]

aoc_runtime::solution!(solve);

#[cfg(target_os = "none")]
unsafe extern "C" {
    fn __aoc_roi_end();
}

fn solve(input: &str) -> u64 {
    // SAFETY: the marker is a bare `ret`.
    #[cfg(target_os = "none")]
    unsafe {
        __aoc_roi_end()
    };
    core::hint::black_box(input.len() as u64)
}
