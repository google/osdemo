// Copyright 2025 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use aarch64_rt::Stack;
use alloc::{boxed::Box, collections::btree_map::BTreeMap};
use core::arch::{asm, global_asm};
use embedded_io::Write;
use flat_device_tree::Fdt;
use log::error;
use smccc::{
    psci::{self, AffinityState, LowestAffinityLevel},
    Hvc,
};
use spin::mutex::SpinMutex;

const SECONDARY_STACK_PAGE_COUNT: usize = 4;

static SECONDARY_STACKS: SpinMutex<BTreeMap<usize, SecondaryStack>> =
    SpinMutex::new(BTreeMap::new());

fn get_secondary_stack_end(cpu_index: usize) -> *mut Stack<SECONDARY_STACK_PAGE_COUNT> {
    SECONDARY_STACKS.lock().entry(cpu_index).or_default().end()
}

#[derive(Debug)]
struct SecondaryStack {
    stack: *mut Stack<SECONDARY_STACK_PAGE_COUNT>,
}

impl SecondaryStack {
    fn end(&self) -> *mut Stack<SECONDARY_STACK_PAGE_COUNT> {
        self.stack.wrapping_add(1)
    }
}

impl Default for SecondaryStack {
    fn default() -> Self {
        Self {
            stack: Box::into_raw(Box::new(Stack::<SECONDARY_STACK_PAGE_COUNT>::new())),
        }
    }
}

// SAFETY: A secondary stack can be sent between CPUs; in fact it must be to start a secondary CPU.
// It's just a memory allocation, there's nothing CPU-specific about it.
unsafe impl Send for SecondaryStack {}

pub fn start_cpu<'a>(console: &mut impl Write, fdt: &Fdt, mut args: impl Iterator<Item = &'a str>) {
    let Some(cpu_index) = args.next() else {
        writeln!(console, "Usage:").unwrap();
        writeln!(console, "  start_cpu <cpu_index>").unwrap();
        return;
    };
    let Ok(cpu_index) = cpu_index.parse() else {
        writeln!(console, "Invalid cpu_index").unwrap();
        return;
    };

    let Some(cpu) = fdt.cpus().nth(cpu_index) else {
        writeln!(console, "cpu_index out of bounds").unwrap();
        return;
    };

    let id = cpu.ids().unwrap().first().unwrap() as u64;
    writeln!(console, "CPU {}: ID {:#012x}", cpu_index, id).unwrap();
    let state = psci::affinity_info::<Hvc>(id, LowestAffinityLevel::All).unwrap();
    if state == AffinityState::Off {
        let stack = get_secondary_stack_end(cpu_index);
        writeln!(console, " Starting with stack {:?}", stack).unwrap();
        let result = psci::cpu_on::<Hvc>(id, secondary_entry as _, stack as _);
        writeln!(console, " => {:?}", result).unwrap();
    } else {
        writeln!(console, " already {:?}", state).unwrap();
    }
}

#[unsafe(no_mangle)]
extern "C" fn rust_secondary_entry() -> ! {
    //info!("Secondary CPU started");
    psci::cpu_off::<Hvc>().unwrap();
    error!("PSCI_CPU_OFF returned unexpectedly");
    #[allow(clippy::empty_loop)]
    loop {}
}

unsafe extern "C" {
    unsafe fn secondary_entry() -> !;
}
global_asm!(include_str!("../secondary_entry.S"));

pub fn cpus(console: &mut impl Write, fdt: &Fdt) {
    writeln!(console, "PSCI version {}", psci::version::<Hvc>().unwrap()).unwrap();

    let mpidr = read_mpidr_el1();
    let uniprocessor = mpidr & (1 << 30) != 0;
    let multithreading = mpidr & (1 << 24) != 0;
    let current_cpu = mpidr & 0xff00ffffff;
    writeln!(
        console,
        "MPIDR {:#012x}: uniprocessor {}, multithreading {}",
        mpidr, uniprocessor, multithreading
    )
    .unwrap();
    writeln!(
        console,
        "Current CPU {:#012x} affinity state {:?}",
        current_cpu,
        psci::affinity_info::<Hvc>(current_cpu, LowestAffinityLevel::All).unwrap(),
    )
    .unwrap();

    for (i, cpu) in fdt.cpus().enumerate() {
        let id = cpu.ids().unwrap().first().unwrap() as u64;
        writeln!(console, "CPU {}: ID {:#012x}", i, id).unwrap();
        writeln!(
            console,
            "  affinity state {:?} {:?} {:?} {:?}",
            psci::affinity_info::<Hvc>(id, LowestAffinityLevel::All).unwrap(),
            psci::affinity_info::<Hvc>(id, LowestAffinityLevel::Aff0Ignored).unwrap(),
            psci::affinity_info::<Hvc>(id, LowestAffinityLevel::Aff0Aff1Ignored).unwrap(),
            psci::affinity_info::<Hvc>(id, LowestAffinityLevel::Aff0Aff1Aff2Ignored).unwrap(),
        )
        .unwrap();
    }
}

fn read_mpidr_el1() -> u64 {
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
