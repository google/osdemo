// Copyright 2025 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::{
    cpus::{MPIDR_AFFINITY_MASK, MPIDR_MT_BIT, MPIDR_U_BIT, current_cpu_index, read_mpidr_el1},
    interrupts::{GIC, remove_private_irq_handler, set_private_irq_handler},
    secondary_entry::start_core_with_stack,
    smc_for_psci,
};
use arm_gic::{
    IntId,
    gicv3::{GicCpuInterface, SgiTarget, SgiTargetGroup},
    irq_enable, wfi,
};
use dtoolkit::fdt::Fdt;
use embedded_io::Write;
use log::{error, info};
use smccc::{
    Hvc, Smc,
    psci::{self, AffinityState, LowestAffinityLevel},
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

    let Some(cpu) = fdt.cpus().unwrap().cpus().nth(cpu_index) else {
        writeln!(console, "cpu_index out of bounds").unwrap();
        return;
    };

    let id = cpu.ids().unwrap().next().unwrap().to_int::<u64>().unwrap();
    writeln!(console, "CPU {cpu_index}: ID {id:#012x}").unwrap();
    let state = if smc_for_psci() {
        psci::affinity_info::<Smc>(id, LowestAffinityLevel::All)
    } else {
        psci::affinity_info::<Hvc>(id, LowestAffinityLevel::All)
    }
    .unwrap();
    if state == AffinityState::Off {
        let result = start_core_with_stack(id, move || secondary_entry(arg));
        writeln!(console, " => {result:?}").unwrap();
    } else {
        writeln!(console, " already {state:?}").unwrap();
    }
}

fn secondary_entry(arg: u64) {
    let cpu = current_cpu_index();
    info!("Secondary CPU {cpu} started with arg {arg}");
    {
        let mut gic = GIC.get().unwrap().lock();
        for i in 0..IntId::SGI_COUNT {
            let sgi = IntId::sgi(i);
            gic.enable_interrupt(sgi, Some(cpu), true).unwrap();
            gic.set_interrupt_priority(sgi, Some(cpu), 0x80).unwrap();
        }
    }
    for sgi in 0..IntId::SGI_COUNT {
        set_private_irq_handler(IntId::sgi(sgi), &secondary_irq_handler);
    }
    irq_enable();

    info!("Waiting for interrupt...");
    wfi();
    info!("Finished waiting");

    for sgi in 0..IntId::SGI_COUNT {
        remove_private_irq_handler(IntId::sgi(sgi));
    }

    if smc_for_psci() {
        psci::cpu_off::<Smc>()
    } else {
        psci::cpu_off::<Hvc>()
    }
    .unwrap();
    error!("PSCI_CPU_OFF returned unexpectedly");
    #[allow(clippy::empty_loop)]
    loop {}
}

fn secondary_irq_handler(intid: IntId) {
    info!(
        "Secondary CPU {} IRQ handler {intid:?}",
        current_cpu_index()
    );
}

pub fn cpus(console: &mut impl Write, fdt: &Fdt) {
    let smc_for_psci = smc_for_psci();

    writeln!(
        console,
        "PSCI version {}",
        if smc_for_psci {
            psci::version::<Smc>()
        } else {
            psci::version::<Hvc>()
        }
        .unwrap()
    )
    .unwrap();

    let mpidr = read_mpidr_el1();
    let uniprocessor = mpidr & MPIDR_U_BIT != 0;
    let multithreading = mpidr & MPIDR_MT_BIT != 0;
    let current_cpu = mpidr & MPIDR_AFFINITY_MASK;
    writeln!(
        console,
        "MPIDR {mpidr:#012x}: uniprocessor {uniprocessor}, multithreading {multithreading}"
    )
    .unwrap();
    writeln!(
        console,
        "Current CPU {:#012x} affinity state {:?}",
        current_cpu,
        if smc_for_psci {
            psci::affinity_info::<Smc>(current_cpu, LowestAffinityLevel::All)
        } else {
            psci::affinity_info::<Hvc>(current_cpu, LowestAffinityLevel::All)
        }
        .unwrap(),
    )
    .unwrap();

    for (i, cpu) in fdt.cpus().unwrap().cpus().enumerate() {
        let id = cpu.ids().unwrap().next().unwrap().to_int::<u64>().unwrap();
        writeln!(console, "CPU {i}: ID {id:#012x}").unwrap();
        if smc_for_psci {
            writeln!(
                console,
                "  affinity state {:?} {:?} {:?} {:?}",
                psci::affinity_info::<Smc>(id, LowestAffinityLevel::All).unwrap(),
                psci::affinity_info::<Smc>(id, LowestAffinityLevel::Aff0Ignored).unwrap(),
                psci::affinity_info::<Smc>(id, LowestAffinityLevel::Aff0Aff1Ignored).unwrap(),
                psci::affinity_info::<Smc>(id, LowestAffinityLevel::Aff0Aff1Aff2Ignored).unwrap(),
            )
        } else {
            writeln!(
                console,
                "  affinity state {:?} {:?} {:?} {:?}",
                psci::affinity_info::<Hvc>(id, LowestAffinityLevel::All).unwrap(),
                psci::affinity_info::<Hvc>(id, LowestAffinityLevel::Aff0Ignored).unwrap(),
                psci::affinity_info::<Hvc>(id, LowestAffinityLevel::Aff0Aff1Ignored).unwrap(),
                psci::affinity_info::<Hvc>(id, LowestAffinityLevel::Aff0Aff1Aff2Ignored).unwrap(),
            )
        }
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
    writeln!(console, "Sending {intid:?} to all CPUs").unwrap();
    GicCpuInterface::send_sgi(intid, SgiTarget::All, SgiTargetGroup::CurrentGroup1).unwrap();
}
