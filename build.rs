// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
