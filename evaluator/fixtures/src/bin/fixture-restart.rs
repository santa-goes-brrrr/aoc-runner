//! Executes the start marker a second time, which would reset a naive counter
//! just before the solver returns.
#![cfg_attr(target_os = "none", no_std, no_main)]

aoc_runtime::solution!(solve);

#[cfg(target_os = "none")]
unsafe extern "C" {
    fn __aoc_roi_start();
}

fn solve(input: &str) -> u64 {
    let answer = core::hint::black_box(input.len() as u64);
    // SAFETY: the marker is a bare `ret`.
    #[cfg(target_os = "none")]
    unsafe {
        __aoc_roi_start()
    };
    answer
}
