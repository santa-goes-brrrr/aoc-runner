#![no_std]

#[cfg(target_os = "none")]
use core::{
    arch::asm,
    panic::PanicInfo,
    ptr::{read_volatile, write_volatile},
};
#[cfg(target_os = "none")]
use linked_list_allocator::LockedHeap;

pub const RAM_BASE: usize = 0x8000_0000;
pub const RAM_SIZE: usize = 256 * 1024 * 1024;
pub const INPUT_ADDRESS: usize = 0x8C00_0000;
pub const INPUT_CAPACITY: usize = 16 * 1024 * 1024;
pub const OUTPUT_ADDRESS: usize = 0x8D00_0000;
pub const TEST_DEVICE_ADDRESS: usize = 0x0010_0000;

pub const INPUT_MAGIC: [u8; 8] = *b"AOCIN001";
pub const OUTPUT_MAGIC: [u8; 8] = *b"AOCOUT01";
pub const PROTOCOL_VERSION: u32 = 1;

pub const STATE_EMPTY: u32 = 0;
pub const STATE_RUNNING: u32 = 1;
pub const STATE_OK: u32 = 2;
pub const STATE_INVALID_INPUT: u32 = 3;
pub const STATE_PANIC: u32 = 4;

pub const ANSWER_UNSIGNED: u32 = 1;
pub const ANSWER_SIGNED: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct InputHeader {
    pub magic: [u8; 8],
    pub version: u32,
    pub flags: u32,
    pub length: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OutputMailbox {
    pub magic: [u8; 8],
    pub version: u32,
    pub state: u32,
    pub answer_kind: u32,
    pub reserved: u32,
    pub answer_low: u64,
    pub answer_high: u64,
}

pub const INPUT_HEADER_SIZE: usize = size_of::<InputHeader>();
pub const OUTPUT_MAILBOX_SIZE: usize = size_of::<OutputMailbox>();
const _: [(); 24] = [(); INPUT_HEADER_SIZE];
const _: [(); 40] = [(); OUTPUT_MAILBOX_SIZE];

#[derive(Clone, Copy)]
#[cfg_attr(not(target_os = "none"), allow(dead_code))]
pub struct Answer {
    kind: u32,
    bits: u128,
}

pub trait IntoAnswer {
    fn into_answer(self) -> Answer;
}

macro_rules! unsigned_answers {
    ($($type:ty),+ $(,)?) => {
        $(
            impl IntoAnswer for $type {
                #[inline]
                fn into_answer(self) -> Answer {
                    Answer { kind: ANSWER_UNSIGNED, bits: self as u128 }
                }
            }
        )+
    };
}

macro_rules! signed_answers {
    ($($type:ty),+ $(,)?) => {
        $(
            impl IntoAnswer for $type {
                #[inline]
                fn into_answer(self) -> Answer {
                    Answer { kind: ANSWER_SIGNED, bits: (self as i128) as u128 }
                }
            }
        )+
    };
}

unsigned_answers!(u8, u16, u32, u64, u128, usize);
signed_answers!(i8, i16, i32, i64, i128, isize);

#[cfg(target_os = "none")]
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[cfg(target_os = "none")]
core::arch::global_asm!(
    r#"
    .section .text.roi
    .global __aoc_roi_start
    .type __aoc_roi_start, @function
__aoc_roi_start:
    ret
    .size __aoc_roi_start, . - __aoc_roi_start

    .global __aoc_roi_end
    .type __aoc_roi_end, @function
__aoc_roi_end:
    ret
    .size __aoc_roi_end, . - __aoc_roi_end
    "#
);

#[cfg(target_os = "none")]
unsafe extern "C" {
    static __heap_start: u8;
    static __heap_end: u8;
    fn __aoc_roi_start();
    fn __aoc_roi_end();
}

#[cfg(target_os = "none")]
#[inline(always)]
fn mailbox() -> *mut OutputMailbox {
    OUTPUT_ADDRESS as *mut OutputMailbox
}

#[cfg(target_os = "none")]
unsafe fn set_state(state: u32) {
    unsafe {
        write_volatile(&raw mut (*mailbox()).state, state);
        asm!("fence rw, rw", options(nostack));
    }
}

#[cfg(target_os = "none")]
unsafe fn finish(success: bool, code: u16) -> ! {
    let signature = if success { 0x5555 } else { 0x3333 };
    let value = (u32::from(code) << 16) | signature;
    unsafe {
        asm!("fence rw, rw", options(nostack));
        write_volatile(TEST_DEVICE_ADDRESS as *mut u32, value);
    }
    loop {
        unsafe { asm!("wfi", options(nomem, nostack)) };
    }
}

#[cfg(target_os = "none")]
unsafe fn fail(state: u32) -> ! {
    unsafe {
        set_state(state);
        finish(false, state as u16)
    }
}

#[cfg(target_os = "none")]
pub fn run<S, T>(solve: S) -> !
where
    S: FnOnce(&str) -> T,
    T: IntoAnswer,
{
    unsafe {
        let heap_start = &raw const __heap_start as usize;
        let heap_end = &raw const __heap_end as usize;
        ALLOCATOR
            .lock()
            .init(heap_start as *mut u8, heap_end - heap_start);

        write_volatile(
            mailbox(),
            OutputMailbox {
                magic: OUTPUT_MAGIC,
                version: PROTOCOL_VERSION,
                state: STATE_RUNNING,
                answer_kind: 0,
                reserved: 0,
                answer_low: 0,
                answer_high: 0,
            },
        );

        let header = read_volatile(INPUT_ADDRESS as *const InputHeader);
        if header.magic != INPUT_MAGIC
            || header.version != PROTOCOL_VERSION
            || header.flags != 0
            || header.length as usize > INPUT_CAPACITY - size_of::<InputHeader>()
        {
            fail(STATE_INVALID_INPUT);
        }

        let bytes = core::slice::from_raw_parts(
            (INPUT_ADDRESS + size_of::<InputHeader>()) as *const u8,
            header.length as usize,
        );
        let Ok(input) = core::str::from_utf8(bytes) else {
            fail(STATE_INVALID_INPUT);
        };

        __aoc_roi_start();
        let answer = core::hint::black_box(solve(input)).into_answer();
        __aoc_roi_end();

        write_volatile(&raw mut (*mailbox()).answer_kind, answer.kind);
        write_volatile(&raw mut (*mailbox()).answer_low, answer.bits as u64);
        write_volatile(
            &raw mut (*mailbox()).answer_high,
            (answer.bits >> 64) as u64,
        );
        set_state(STATE_OK);
        finish(true, 0)
    }
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    unsafe { fail(STATE_PANIC) }
}

#[macro_export]
macro_rules! solution {
    ($solver:expr) => {
        #[cfg(target_os = "none")]
        mod __aoc_bare_metal {
            use super::*;
            $crate::entry!($solver);
        }

        #[cfg(not(target_os = "none"))]
        fn main() {
            use std::io::Read as _;

            let mut input = std::string::String::new();
            std::io::stdin().read_to_string(&mut input).unwrap();
            std::println!("{}", ($solver)(&input));
        }
    };
}

#[macro_export]
macro_rules! entry {
    ($solver:expr) => {
        core::arch::global_asm!(
            r#"
            .section .text.entry
            .global _start
            .type _start, @function
        _start:
            .option push
            .option norelax
            la gp, __global_pointer$
            .option pop
            la sp, __stack_top

            li t0, ((3 << 9) | (3 << 13))
            csrs mstatus, t0

            la t0, __bss_start
            la t1, __bss_end
        1:
            bgeu t0, t1, 2f
            sd zero, 0(t0)
            addi t0, t0, 8
            j 1b
        2:
            call __aoc_rust_main
        3:
            wfi
            j 3b
            .size _start, . - _start
            "#
        );

        #[unsafe(no_mangle)]
        extern "C" fn __aoc_rust_main() -> ! {
            $crate::run($solver)
        }
    };
}
