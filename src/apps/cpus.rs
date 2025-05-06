// Copyright 2025 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::{
    cpus::{current_cpu_index, read_mpidr_el1, MPIDR_AFFINITY_MASK, MPIDR_MT_BIT, MPIDR_U_BIT},
    interrupts::GIC,
    pagetable::PAGETABLE,
    secondary_entry::start_core_with_stack,
};
use arm_gic::{
    gicv3::{GicV3, Group, SgiTarget},
    wfi, IntId,
};
use embedded_io::Write;
use flat_device_tree::Fdt;
use log::{error, info};
use smccc::{
    psci::{self, AffinityState, LowestAffinityLevel},
    Hvc,
};

pub fn start_cpu<'a>(console: &mut impl Write, fdt: &Fdt, mut args: impl Iterator<Item = &'a str>) {
    let Some(cpu_index) = args.next() else {
        writeln!(console, "Usage:").unwrap();
        writeln!(console, "  start_cpu <cpu_index> <arg>").unwrap();
        return;
    };
    let Ok(cpu_index) = cpu_index.parse() else {
        writeln!(console, "Invalid cpu_index").unwrap();
        return;
    };
    let Some(arg) = args.next() else {
        writeln!(console, "Usage:").unwrap();
        writeln!(console, "  start_cpu <cpu_index> <arg>").unwrap();
        return;
    };
    let Ok(arg) = arg.parse() else {
        writeln!(console, "Invalid arg").unwrap();
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
        let result = start_core_with_stack(id, rust_secondary_entry, arg);
        writeln!(console, " => {:?}", result).unwrap();
    } else {
        writeln!(console, " already {:?}", state).unwrap();
    }
}

extern "C" fn rust_secondary_entry(arg: u64) -> ! {
    info!("Secondary CPU started: {}", arg);
    // SAFETY: All relevant memory was mapped before the pagetable was activated on the primary
    // core.
    unsafe {
        PAGETABLE.get().unwrap().activate_secondary();
    }
    info!("Page table activated on secondary CPU.");
    let cpu = current_cpu_index();
    info!("Initialising GIC for CPU {}", cpu);
    {
        let mut gic = GIC.get().unwrap().lock();
        gic.init_cpu(cpu);
        for i in 0..IntId::SGI_COUNT {
            let sgi = IntId::sgi(i);
            gic.enable_interrupt(sgi, Some(cpu), true);
            gic.set_interrupt_priority(sgi, Some(cpu), 0x80);
            gic.set_group(sgi, Some(cpu), Group::Group1NS);
        }
    }
    GicV3::enable_group1(true);
    GicV3::set_priority_mask(0xff);
    // Don't actually unmask interrupts, as we haven't registered a handler.
    info!("Waiting for interrupt...");
    wfi();
    let intid = GicV3::get_and_acknowledge_interrupt().expect("No pending interrupt");
    info!("Secondary CPU {} received interrupt {:?}.", cpu, intid);
    psci::cpu_off::<Hvc>().unwrap();
    error!("PSCI_CPU_OFF returned unexpectedly");
    #[allow(clippy::empty_loop)]
    loop {}
}

pub fn cpus(console: &mut impl Write, fdt: &Fdt) {
    writeln!(console, "PSCI version {}", psci::version::<Hvc>().unwrap()).unwrap();

    let mpidr = read_mpidr_el1();
    let uniprocessor = mpidr & MPIDR_U_BIT != 0;
    let multithreading = mpidr & MPIDR_MT_BIT != 0;
    let current_cpu = mpidr & MPIDR_AFFINITY_MASK;
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

pub fn sgi<'a>(console: &mut impl Write, mut args: impl Iterator<Item = &'a str>) {
    let Some(id) = args.next() else {
        writeln!(console, "Usage:").unwrap();
        writeln!(console, "  sgi <id>").unwrap();
        return;
    };
    let Ok(id) = id.parse() else {
        writeln!(console, "Invalid id").unwrap();
        return;
    };
    if id >= IntId::SGI_COUNT {
        writeln!(
            console,
            "Invalid SGI, must be less than {}",
            IntId::SGI_COUNT
        )
        .unwrap();
        return;
    }

    let intid = IntId::sgi(id);
    writeln!(console, "Sending {:?} to all CPUs", intid).unwrap();
    GicV3::send_sgi(intid, SgiTarget::All);
}
