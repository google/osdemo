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

fn main() {
    env::set_var("CROSS_COMPILE", "aarch64-none-elf");
    env::set_var("CC", "clang");

    let platform = env::var("CARGO_CFG_PLATFORM").expect("Missing platform name");
    match platform.as_ref() {
        "qemu" => {
            Build::new()
                .file("asm/entry.S")
                .file("asm/exceptions.S")
                .file("asm/idmap_qemu.S")
                .compile("empty");
            println!("cargo:rustc-link-arg=-Tlinker/qemu.ld");
        }
        "crosvm" => {
            Build::new()
                .file("asm/entry.S")
                .file("asm/exceptions.S")
                .file("asm/idmap_crosvm.S")
                .compile("empty");
            println!("cargo:rustc-link-arg=-Tlinker/crosvm.ld");
        }
        _ => {
            panic!("Unexpected platform name \"{}\"", platform);
        }
    }
    println!("cargo:rustc-link-arg=-Tlinker/image.ld");
}
