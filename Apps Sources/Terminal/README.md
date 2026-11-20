# Purple Terminal

Native PurpleOS terminal application. It uses the AegisShell GUI command interface and the system-signed command database. The published source is the bare-metal `purple-terminal` crate; the former OXIDE/egui desktop prototype is not part of this app.

## Source

- `native/Cargo.toml` and `native/Cargo.lock` define the `x86_64-unknown-none` app build.
- `src/purple.rs` implements the terminal UI, tabs, input, command history, and Shell calls.
- `src/inter.rs` rasterizes Inter font glyphs from their real outlines.

Build from the repository root configuration:

```sh
cd 'Apps Sources/Terminal/native'
cargo build --release
```

## Published package

`build/terminal.apapplication` is the signed app package. Its Creator-signed trust policy and signature are included alongside the Inter Regular font resource and license.
