// Copyright 2025 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::{interrupts::secondary_init_gic, pagetable::PAGETABLE, smc_for_psci};
use aarch64_rt::{Stack, start_core};
use alloc::{boxed::Box, collections::btree_map::BTreeMap};
use core::ops::DerefMut;
use log::debug;
use smccc::{Hvc, Smc, psci};
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

/// Issues a PSCI CPU_ON call to start the CPU core with the given MPIDR, first allocating an
/// appropriate stack if necessary.
///
/// `entry` should never return.
pub fn start_core_with_stack(
    mpidr: u64,
    entry: impl FnOnce() + Send + 'static,
) -> Result<(), psci::Error> {
    let stack = get_secondary_stack(mpidr);

    // SAFETY: We allocate a unique stack per MPIDR, and never deallocate it.
    unsafe {
        if smc_for_psci() {
            start_core::<Smc, _, SECONDARY_STACK_PAGE_COUNT>(mpidr, stack, move || {
                secondary_init();
                entry()
            })
        } else {
            start_core::<Hvc, _, SECONDARY_STACK_PAGE_COUNT>(mpidr, stack, move || {
                secondary_init();
                entry()
            })
        }
    }
}

fn secondary_init() {
    // SAFETY: All relevant memory was mapped before the pagetable was activated on the primary
    // core.
    unsafe {
        PAGETABLE.get().unwrap().activate_secondary();
    }
    debug!("Page table activated on secondary CPU.");
    secondary_init_gic();
}
