# Copyright 2024 Google LLC
#
# This project is dual-licensed under Apache 2.0 and MIT terms.
# See LICENSE-APACHE and LICENSE-MIT for details.

TARGET := --target aarch64-unknown-none

CROSVM_BIN := target/osdemo.crosvm.bin
CROSVM_RUSTFLAGS := "--cfg platform=\"crosvm\""
QEMU_BIN := target/osdemo.qemu.bin
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
	adb push $< /data/local/tmp/virt_raw/osdemo
	adb shell "/apex/com.android.virt/bin/crosvm --log-level=info --extended-status run --disable-sandbox --bios=/data/local/tmp/virt_raw/osdemo --rwdisk=/dev/null"

qemu: $(QEMU_BIN)
	qemu-system-aarch64 -machine virt,gic-version=3 -cpu max -display none -kernel $< -s \
	  -smp 4 -serial mon:stdio \
	  -global virtio-mmio.force-legacy=false \
	  -drive file=/dev/null,if=none,format=raw,id=x0 \
	  -device virtio-blk-device,drive=x0 \
	  -device virtio-serial,id=virtio-serial0 \
	  -chardev socket,path=/tmp/qemu-console,server=on,wait=off,id=char0,mux=on \
	  -device virtconsole,chardev=char0 \
	  -device vhost-vsock-device,id=virtiosocket0,guest-cid=102

clean:
	cargo clean
	rm -f target/*.bin
