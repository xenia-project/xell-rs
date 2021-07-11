# Stage 1 bootloader
This is the first bit of code that runs after the glitched CD bootloader [jumps to us](https://github.com/Free60Project/tools/blob/ddcd9c55875257e671813ca857374e03b5247b1f/reset_glitch_hack/cdxell/cdxell.S#L62-L89).

From there, it jumps to a bit of code implemented in `startup.s`. We set up the bare minimum system state required to run Rust code, and call `__start_rust` in `main.rs`.