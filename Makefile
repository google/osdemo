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

CROSVM_BIN := target/demoos.crosvm.bin
CROSVM_RUSTFLAGS := "--cfg platform=\"crosvm\""
QEMU_BIN := target/demoos.qemu.bin
QEMU_RUSTFLAGS := "--cfg platform=\"qemu\""

.PHONY: all build.qemu build.crosvm clean clippy crosvm qemu

all: $(CROSVM_BIN) $(QEMU_BIN)

clippy:
	RUSTFLAGS=$(QEMU_RUSTFLAGS) cargo clippy $(TARGET)

build.crosvm:
	RUSTFLAGS=$(CROSVM_RUSTFLAGS) cargo build $(TARGET)

build.qemu:
	RUSTFLAGS=$(QEMU_RUSTFLAGS) cargo build $(TARGET)

$(CROSVM_BIN): build.crosvm
	RUSTFLAGS=$(CROSVM_RUSTFLAGS) cargo objcopy $(TARGET) -- -O binary $@

$(QEMU_BIN): build.qemu
	RUSTFLAGS=$(QEMU_RUSTFLAGS) cargo objcopy $(TARGET) -- -O binary $@

crosvm: $(CROSVM_BIN)
	adb shell 'mkdir -p /data/local/tmp/virt_raw'
	adb push $< /data/local/tmp/virt_raw/demoos
	adb shell "/apex/com.android.virt/bin/crosvm --log-level=info --extended-status run --disable-sandbox --bios=/data/local/tmp/virt_raw/demoos --rwdisk=/dev/null"

qemu: $(QEMU_BIN)
	qemu-system-aarch64 -machine virt,gic-version=3 -cpu max -display none -kernel $< -s \
	  -serial mon:stdio \
	  -global virtio-mmio.force-legacy=false \
	  -drive file=/dev/null,if=none,format=raw,id=x0 \
	  -device virtio-blk-device,drive=x0 \
	  -device virtio-serial,id=virtio-serial0 \
	  -chardev socket,path=/tmp/qemu-console,server=on,wait=off,id=char0,mux=on \
	  -device virtconsole,chardev=char0

clean:
	cargo clean
	rm -f target/*.bin
