# Xell-rs
This is the [xell-reloaded](https://github.com/xenia-project/xell-reloaded) bootloader, rewritten in Rust.

Currently this project is a huge WIP and can't do much, but eventually this can be used to boot Linux from TFTP or a local storage device.

![Serial Terminal](/docs/images/serial_terminal.png)

## Crates
 * boot/stage1: The very first stage bootloader.
 * shared/
   * core_reqs: Bare-minimum functionality required for Rust's libcore. Originally from the [chocolate milk](https://github.com/gamozolabs/chocolate_milk/blob/643f47b901ceda1f688d3c20ff92b0f41af80251/shared/core_reqs/src/lib.rs) project.
   * sync: Xenon-specific mutex spinlock implementation
   * xenon-cpu: Xenon-specific CPU intrinsics
   * xenon-enet: Xenon fast ethernet driver
   * xenon-soc: Drivers for Xenon SoC functionality