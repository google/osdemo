// Copyright 2025 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use aarch64_rt::Stack;
use alloc::{boxed::Box, collections::btree_map::BTreeMap};
use core::{
    arch::{asm, global_asm},
    ops::DerefMut,
};
use smccc::{psci, Hvc};
use spin::mutex::SpinMutex;

/// The number of pages to allocate for each secondary core stack.
const SECONDARY_STACK_PAGE_COUNT: usize = 4;

/// Stacks allocated for secondary cores.
static SECONDARY_STACKS: SpinMutex<BTreeMap<u64, SecondaryStack>> = SpinMutex::new(BTreeMap::new());

/// A pointer to a stack allocated for a secondary CPU.
///
/// This must not be dropped as long as the secondary CPU is running.
struct SecondaryStack {
    stack: Box<Stack<SECONDARY_STACK_PAGE_COUNT>>,
}

impl SecondaryStack {
    fn ptr(&mut self) -> *mut Stack<SECONDARY_STACK_PAGE_COUNT> {
        self.stack.deref_mut()
    }
}

impl Default for SecondaryStack {
    fn default() -> Self {
        Self {
            stack: Box::new(Stack::<SECONDARY_STACK_PAGE_COUNT>::new()),
        }
    }
}

/// Returns a pointer to the stack allocated for the core with the given MPIDR.
fn get_secondary_stack(mpidr: u64) -> *mut Stack<SECONDARY_STACK_PAGE_COUNT> {
    SECONDARY_STACKS.lock().entry(mpidr).or_default().ptr()
}

/// Issues a PSCI CPU_ON call to start the CPU core with the given MPIDR.
///
/// This starts the core with an assembly entry point which will enable the MMU, disable trapping of
/// floating point instructions, initialise the stack pointer to the given value, and then jump to
/// the given Rust entry point function, passing it the given argument value.
///
/// # Safety
///
/// `stack` must point to a region of memory which is reserved for this core's stack. It must remain
/// valid as long as the core is running, and there must not be any other access to it during that
/// time.
unsafe fn start_core<const N: usize>(
    mpidr: u64,
    stack: *mut Stack<N>,
    rust_entry: extern "C" fn(arg: u64) -> !,
    arg: u64,
) -> Result<(), psci::Error> {
    assert!(stack.is_aligned());
    // The stack grows downwards on aarch64, so get a pointer to the end of the stack.
    let stack_end = stack.wrapping_add(1);

    // Write Rust entry point to the stack, so the assembly entry point can jump to it.
    let params = stack_end as *mut u64;
    // SAFETY: Our caller promised that the stack is valid and nothing else will access it.
    unsafe {
        *params.wrapping_sub(1) = rust_entry as _;
        *params.wrapping_sub(2) = arg;
    }
    // Wait for the stores above to complete before starting the secondary CPU core.
    dsb_st();

    psci::cpu_on::<Hvc>(mpidr, secondary_entry as _, stack_end as _)
}

/// Issues a PSCI CPU_ON call to start the CPU core with the given MPIDR, first allocating an
/// appropriate stack if necessary.
pub fn start_core_with_stack(
    mpidr: u64,
    rust_entry: extern "C" fn(arg: u64) -> !,
    arg: u64,
) -> Result<(), psci::Error> {
    let stack = get_secondary_stack(mpidr);
    // SAFETY: We allocate a unique stack per MPIDR, and never deallocate it.
    unsafe { start_core(mpidr, stack, rust_entry, arg) }
}

unsafe extern "C" {
    /// An assembly entry point for secondary cores.
    ///
    /// It will enable the MMU, disable trapping of floating point instructions, initialise the
    /// stack pointer to `stack_end` and then jump to the function pointer at the bottom of the
    /// stack with the u64 value second on the stack as a parameter.
    unsafe fn secondary_entry(stack_end: *mut u64) -> !;
}

global_asm!(include_str!("secondary_entry.S"));

/// Data synchronisation barrier that waits for stores to complete, for the full system.
fn dsb_st() {
    unsafe {
        asm!("dsb st", options(nostack));
    }
}
