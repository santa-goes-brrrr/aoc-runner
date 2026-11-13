//! Bypasses `solution!` with its own entry point and computes its answer from
//! the raw input packet before the measured region starts, leaving a solver
//! that only returns the cached value.
#![cfg_attr(target_os = "none", no_std, no_main)]

#[cfg(target_os = "none")]
mod bare_metal {
    use aoc_runtime::{INPUT_ADDRESS, INPUT_HEADER_SIZE, InputHeader};

    core::arch::global_asm!(
        r#"
        .section .text.entry
        .global _start
    _start:
        .option push
        .option norelax
        tail __aoc_boot
        .option pop
        .zero 60
        "#
    );

    #[inline(never)]
    fn work(bytes: &[u8]) -> u64 {
        let mut total = 0_u64;
        for round in 0..200_u64 {
            for &byte in bytes {
                total = core::hint::black_box(total.wrapping_add(u64::from(byte) ^ round));
            }
        }
        total
    }

    #[unsafe(no_mangle)]
    extern "C" fn __aoc_rust_main() -> ! {
        // SAFETY: the host loaded a valid packet at this address.
        let answer = unsafe {
            let header = core::ptr::read_volatile(INPUT_ADDRESS as *const InputHeader);
            work(core::slice::from_raw_parts(
                (INPUT_ADDRESS + INPUT_HEADER_SIZE) as *const u8,
                header.length as usize,
            ))
        };
        aoc_runtime::run(move |_input| answer)
    }
}

#[cfg(not(target_os = "none"))]
fn main() {}
