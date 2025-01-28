// Copyright 2025 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use core::arch::global_asm;

global_asm!(include_str!("asm/entry.S"));
global_asm!(include_str!("asm/exceptions.S"));
#[cfg(platform = "qemu")]
global_asm!(include_str!(concat!("asm/idmap_qemu.S")));
#[cfg(platform = "crosvm")]
global_asm!(include_str!(concat!("asm/idmap_crosvm.S")));
