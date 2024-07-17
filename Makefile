# Copyright 2024 Google LLC
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

TARGET := --target aarch64-unknown-none

CROSVM_RUSTFLAGS := "--cfg platform=\"crosvm\""
QEMU_RUSTFLAGS := "--cfg platform=\"qemu\""

.PHONY: all build.qemu build.crosvm clean crosvm qemu

all: demoos.crosvm.bin demoos.qemu.bin

build.crosvm:
	RUSTFLAGS=$(CROSVM_RUSTFLAGS) cargo build $(TARGET)

build.qemu:
	RUSTFLAGS=$(QEMU_RUSTFLAGS) cargo build $(TARGET)

demoos.crosvm.bin: build.crosvm
	RUSTFLAGS=$(CROSVM_RUSTFLAGS) cargo objcopy $(TARGET) -- -O binary $@

demoos.qemu.bin: build.qemu
	RUSTFLAGS=$(QEMU_RUSTFLAGS) cargo objcopy $(TARGET) -- -O binary $@

crosvm: demoos.crosvm.bin
	adb shell 'mkdir -p /data/local/tmp/virt_raw'
	adb push $< /data/local/tmp/virt_raw/demoos
	adb shell "/apex/com.android.virt/bin/crosvm --log-level=trace --extended-status run --disable-sandbox --bios=/data/local/tmp/virt_raw/demoos"

qemu: demoos.qemu.bin
	qemu-system-aarch64 -machine virt -cpu max -serial mon:stdio -display none -kernel $< -s

clean:
	cargo clean
	rm -f *.bin
