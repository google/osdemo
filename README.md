# aarch64 Rust OS demo

An example of combining various libraries for aarch64 OS development in Rust.

This crate demonstrates how to use a number of aarch64-specific crates:

- `aarch64-paging` for page table management.
- `aarch64-rt` for the entry point and exception handling.
- `smccc` for PSCI and other standard SMC calls to EL3 firmware.

As well as some more general crates for embedded development:

- `buddy_system_allocator` for heap allocation.
- `percore` for exception masking.

And some device driver crates:

- `arm-gic` for the Arm Generic Interrupt Controller.
- `arm_pl031` for the PL031 real-time clock.
- `arm-pl011-uart` for the PL011 UART.
- `virtio-drivers` for various VirtIO devices.

This is not an officially supported Google product.

## License

Licensed under either of

- Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license
  ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributing

If you want to contribute to the project, see details of
[how we accept contributions](CONTRIBUTING.md).
