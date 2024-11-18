// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use cc::Build;
use std::env;

const PLATFORMS: [&str; 2] = ["crosvm", "qemu"];

fn main() {
    println!(
        "cargo::rustc-check-cfg=cfg(platform, values(\"{}\"))",
        PLATFORMS.join("\", \"")
    );

    env::set_var("CROSS_COMPILE", "aarch64-none-elf");
    env::set_var("CC", "clang");

    let platform = env::var("CARGO_CFG_PLATFORM").expect("Missing platform name");
    assert!(
        PLATFORMS.contains(&platform.as_str()),
        "Unexpected platform name {:?}. Supported platforms: {:?}",
        platform,
        PLATFORMS,
    );

    Build::new()
        .file("asm/entry.S")
        .file("asm/exceptions.S")
        .file(format!("asm/idmap_{}.S", platform))
        .compile("empty");
    println!("cargo:rustc-link-arg=-Tlinker/{platform}.ld");
    println!("cargo:rustc-link-arg=-Tlinker/image.ld");
}
