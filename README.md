# wd-rs — A safe Rust wrapper for libwdi

Warning: this crate is extremely work in progress and the actual wrapping is very bare bones. The functionality
implemented exists almost entirely for [bmputil](https://github.com/blackmagic-debug/bmputil).

## Cross compilation

Considerable effort has been put into libwdi-sys's [build script](./libwdi-sys/build.rs) to ensure cross compilation
works as seamlessly as possible. It will, however, require an existing Windows target cross toolchain setup, but
[cargo-xwin](https://github.com/messense/cargo-xwin) can take care of most of that for you.

The only other requirement is the [Windows 8.0 Driver Kit redistributable components](https://go.microsoft.com/fwlink/p/?LinkID=253170),
with the environment variable `WDK_DIR` set to the path it's been extracted to, e.g. `export WDK_DIR=/opt/wdk/8.0`.
