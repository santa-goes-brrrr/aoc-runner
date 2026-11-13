#![no_std]

extern crate alloc;

use alloc::string::String;
use core::mem::offset_of;
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
pub const TRAP_ADDRESS: usize = 0x8D00_0040;
pub const TEST_DEVICE_ADDRESS: usize = 0x0010_0000;
#[cfg(target_os = "none")]
const TEST_DEVICE_FAIL: u32 = 0x3333;

pub const INPUT_MAGIC: [u8; 8] = *b"AOCIN001";
pub const OUTPUT_MAGIC: [u8; 8] = *b"AOCOUT01";
pub const TRAP_MAGIC: [u8; 8] = *b"AOCTRAP1";
pub const PROTOCOL_VERSION: u32 = 1;
pub const TRAP_VERSION: u32 = 1;

pub const STATE_EMPTY: u32 = 0;
pub const STATE_RUNNING: u32 = 1;
pub const STATE_OK: u32 = 2;
pub const STATE_INVALID_INPUT: u32 = 3;
pub const STATE_PANIC: u32 = 4;
/// The guest took a CPU exception; the `TrapRecord` at `TRAP_ADDRESS` is valid.
pub const STATE_TRAP: u32 = 5;

pub const ANSWER_UNSIGNED: u32 = 1;
pub const ANSWER_SIGNED: u32 = 2;
/// `answer_low` is the guest address of UTF-8 text and `answer_high` its
/// length in bytes. The host reads the text from the guest RAM image.
pub const ANSWER_TEXT: u32 = 3;

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

/// Written by the trap handler when the guest takes a CPU exception.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TrapRecord {
    pub magic: [u8; 8],
    pub version: u32,
    pub reserved: u32,
    /// `mcause`
    pub cause: u64,
    /// `mepc`
    pub pc: u64,
    /// `mtval`
    pub value: u64,
}

pub const INPUT_HEADER_SIZE: usize = size_of::<InputHeader>();
pub const OUTPUT_MAILBOX_SIZE: usize = size_of::<OutputMailbox>();
pub const TRAP_RECORD_SIZE: usize = size_of::<TrapRecord>();
const _: [(); 24] = [(); INPUT_HEADER_SIZE];
const _: [(); 40] = [(); OUTPUT_MAILBOX_SIZE];
const _: [(); 40] = [(); TRAP_RECORD_SIZE];
const _: () = {
    assert!(offset_of!(OutputMailbox, magic) == 0);
    assert!(offset_of!(OutputMailbox, version) == 8);
    assert!(offset_of!(OutputMailbox, state) == 12);
    assert!(offset_of!(OutputMailbox, answer_kind) == 16);
    assert!(offset_of!(OutputMailbox, answer_low) == 24);
    assert!(offset_of!(OutputMailbox, answer_high) == 32);
    assert!(offset_of!(TrapRecord, magic) == 0);
    assert!(offset_of!(TrapRecord, version) == 8);
    assert!(offset_of!(TrapRecord, cause) == 16);
    assert!(offset_of!(TrapRecord, pc) == 24);
    assert!(offset_of!(TrapRecord, value) == 32);
    assert!(OUTPUT_ADDRESS + OUTPUT_MAILBOX_SIZE <= TRAP_ADDRESS);
};

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

/// A text answer is the address and length of bytes that already exist when
/// the measured region ends. They must stay valid until the guest exits, which
/// `'static` guarantees.
impl IntoAnswer for &'static str {
    #[inline]
    fn into_answer(self) -> Answer {
        Answer {
            kind: ANSWER_TEXT,
            bits: ((self.len() as u128) << 64) | (self.as_ptr() as usize as u128),
        }
    }
}

impl IntoAnswer for String {
    #[inline]
    fn into_answer(self) -> Answer {
        let text: &'static str = self.leak();
        text.into_answer()
    }
}

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

    /* Boot and trap code have their own memory region (see linker.ld), so
     * changing them never moves solution code or data. Guest behaviour, and
     * with it a score, can depend on those addresses. */
    .section .aoc_boot, "ax", @progbits
    .global __aoc_boot
    .type __aoc_boot, @function
__aoc_boot:
    .option push
    .option norelax
    la gp, __global_pointer$
    .option pop
    la sp, __stack_top

    la t0, __aoc_trap
    csrw mtvec, t0

    /* Lock a no-access PMP region over the guard below the stack so that an
     * overflow traps instead of silently corrupting the heap. */
    la t0, __stack_guard_start
    la t1, __stack_guard_end
    sub t1, t1, t0
    srli t1, t1, 3
    addi t1, t1, -1
    srli t0, t0, 2
    or t0, t0, t1
    csrw pmpaddr0, t0
    li t0, 0x98
    csrw pmpcfg0, t0

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
    .size __aoc_boot, . - __aoc_boot

    /* Any CPU exception lands here: record it and stop QEMU. The handler uses
     * no stack, so it also works after a stack overflow. */
    .balign 4
    .global __aoc_trap
    .type __aoc_trap, @function
__aoc_trap:
    li t0, {trap}
    li t1, {trap_magic}
    sd t1, {trap_magic_offset}(t0)
    li t1, {trap_version}
    sd t1, {trap_version_offset}(t0)
    csrr t1, mcause
    sd t1, {trap_cause_offset}(t0)
    csrr t1, mepc
    sd t1, {trap_pc_offset}(t0)
    csrr t1, mtval
    sd t1, {trap_value_offset}(t0)

    /* The trap may precede the mailbox header written by `run`. */
    li t0, {mailbox}
    li t1, {mailbox_magic}
    sd t1, {mailbox_magic_offset}(t0)
    li t1, {mailbox_version}
    sw t1, {mailbox_version_offset}(t0)
    li t1, {state}
    sw t1, {mailbox_state_offset}(t0)
    fence rw, rw

    li t0, {finisher}
    li t1, {exit_word}
    sw t1, 0(t0)
4:
    wfi
    j 4b
    .size __aoc_trap, . - __aoc_trap
    "#,
    trap = const TRAP_ADDRESS,
    trap_magic = const u64::from_le_bytes(TRAP_MAGIC),
    trap_magic_offset = const offset_of!(TrapRecord, magic),
    trap_version = const TRAP_VERSION,
    trap_version_offset = const offset_of!(TrapRecord, version),
    trap_cause_offset = const offset_of!(TrapRecord, cause),
    trap_pc_offset = const offset_of!(TrapRecord, pc),
    trap_value_offset = const offset_of!(TrapRecord, value),
    mailbox = const OUTPUT_ADDRESS,
    mailbox_magic = const u64::from_le_bytes(OUTPUT_MAGIC),
    mailbox_magic_offset = const offset_of!(OutputMailbox, magic),
    mailbox_version = const PROTOCOL_VERSION,
    mailbox_version_offset = const offset_of!(OutputMailbox, version),
    state = const STATE_TRAP,
    mailbox_state_offset = const offset_of!(OutputMailbox, state),
    finisher = const TEST_DEVICE_ADDRESS,
    exit_word = const (STATE_TRAP << 16) | TEST_DEVICE_FAIL,
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

/// The solver is inlined into this function, so its instructions share a
/// stack frame and register allocation with the code here. Any edit to this
/// body can change the compiler's output for the solver and therefore every
/// score; put new runtime work in the boot code or the host instead.
#[cfg(target_os = "none")]
pub fn run<S, T>(solve: S) -> !
where
    S: FnOnce(&'static str) -> T,
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

/// Registers the solver. Only a path to a function is accepted: an arbitrary
/// expression would be evaluated before the measured region starts.
#[macro_export]
macro_rules! solution {
    ($solver:path) => {
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
            let input: &'static str = input.leak();
            std::println!("{}", ($solver)(input));
        }
    };
}

#[macro_export]
macro_rules! entry {
    ($solver:path) => {
        // The entry stub is a fixed 68 bytes, the size of the boot code it
        // replaced, so that everything linked after it keeps its address.
        core::arch::global_asm!(
            r#"
            .section .text.entry
            .global _start
            .type _start, @function
        _start:
            .option push
            .option norelax
            tail __aoc_boot
            .option pop
            .zero 60
            .size _start, . - _start
            "#
        );

        #[unsafe(no_mangle)]
        extern "C" fn __aoc_rust_main() -> ! {
            $crate::run($solver)
        }
    };
}
