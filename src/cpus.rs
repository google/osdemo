// Copyright 2025 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::FDT;
use alloc::boxed::Box;
use core::{arch::asm, cell::RefCell};
use percore::{Cores, ExceptionLock, PerCore};
use spin::Lazy;

pub const MPIDR_AFFINITY_MASK: u64 = 0xff00ffffff;
pub const MPIDR_U_BIT: u64 = 1 << 30;
pub const MPIDR_MT_BIT: u64 = 1 << 24;

pub fn read_mpidr_el1() -> u64 {
    let value;
    // SAFETY: Reading the MPIDR is always safe.
    unsafe {
        asm!(
            "mrs {value}, mpidr_el1",
            options(nostack),
            value = out(reg) value,
        );
    }
    value
}

/// Reads the MPIDR value and returns the affinity bytes, masking out the other bits.
pub fn mpidr_affinity() -> u64 {
    read_mpidr_el1() & MPIDR_AFFINITY_MASK
}

/// Returns the index of the current CPU core in the FDT.
pub fn current_cpu_index() -> usize {
    mpidr_to_cpu_index(mpidr_affinity()).unwrap()
}

/// Returns the total number of CPUs on the system.
pub fn cpu_count() -> usize {
    FDT.get().unwrap().cpus().unwrap().cpus().count()
}

/// Returns the index in the FDT of the CPU core with the given MPIDR affinity fields, if it exists.
fn mpidr_to_cpu_index(mpidr_affinity: u64) -> Option<usize> {
    FDT.get().unwrap().cpus().unwrap().cpus().position(|cpu| {
        cpu.ids().unwrap().next().unwrap().to_int::<u64>().unwrap() == mpidr_affinity
    })
}

/// An implementation of `percore::Cores`, to return the index of the curren CPU core.
pub struct CoresImpl;

// SAFETY: `current_cpu_index` gets the CPU index by looking up the MPIDR in the FDT, so can never
// return the same index for different affinity values.
unsafe impl Cores for CoresImpl {
    fn core_index() -> usize {
        current_cpu_index()
    }
}

/// Per-core mutable state.
pub type PerCoreState<T> = Lazy<PerCore<Box<[ExceptionLock<RefCell<T>>]>, CoresImpl>>;

/// Creates a new instance a `PerCoreState`, initialising each core's instance of `T` to
/// `T::default()` the first time it is used.
pub const fn new_per_core_state_with_default<T: Default>() -> PerCoreState<T> {
    Lazy::new(|| PerCore::new_with_default(cpu_count()))
}
