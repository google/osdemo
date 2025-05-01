/*
 * Copyright 2025 Google LLC.
 *
 * This project is dual-licensed under Apache 2.0 and MIT terms.
 * See LICENSE-APACHE and LICENSE-MIT for details.
 */

.global secondary_entry
secondary_entry:
    bl enable_mmu

    /* Disable trapping floating point access in EL1. */
    mrs x30, cpacr_el1
    orr x30, x30, #(0x3 << 20)
    msr cpacr_el1, x30
    isb

    /* Set the stack pointer which was passed. */
    mov sp, x0

    /* Call into Rust code. */
    b rust_secondary_entry
