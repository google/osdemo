// Copyright 2025 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::FDT;
use core::arch::asm;

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

/// Returns the index of the current CPU core in the FDT.
pub fn current_cpu_index() -> usize {
    let mpidr = read_mpidr_el1();
    mpidr_to_cpu_index(mpidr & MPIDR_AFFINITY_MASK).unwrap()
}

/// Returns the index in the FDT of the CPU core with the given MPIDR affinity fields, if it exists.
fn mpidr_to_cpu_index(mpidr_affinity: u64) -> Option<usize> {
    FDT.get()
        .unwrap()
        .cpus()
        .position(|cpu| cpu.ids().unwrap().first().unwrap() as u64 == mpidr_affinity)
}
