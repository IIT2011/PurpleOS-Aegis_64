# Purple OS GUI runtime ABI v1

Purple Desktop owns the framebuffer. Native clients run in isolated x86-64 Ring 3 tasks and draw into kernel-owned windows through a copy-based interface. A client owns only the window handles issued to its task. Closing or terminating the task releases its windows and process memory.

## Build and deploy

Build a freestanding, statically linked x86-64 ELF with entry symbol `_start` using `sdk/gui/purple_gui.ld`. Use `sdk/gui/purple_gui.h` for the C interface. The entry point starts with a 16-byte-aligned stack and cleared general registers. Query the ABI version and startup record with syscalls before using the desktop. The kernel preserves x87 and SSE state between userspace tasks. AVX, ELF TLS, dynamic libraries, and a hosted C runtime are outside ABI v1.

Use `scripts/package_gui_app.py pack` to convert the ELF to APEXEC and sign its `.apapplication` bundle with an Ed25519 private key. The tool prints the public-key policy entry. The system administrator puts the approved entries in an `APTRUST1` policy and signs it with `scripts/package_gui_app.py sign-policy` and the Creator key. Do not put private keys in the bundle or disk image.

The policy is UTF-8 ASCII text. Its first line is `APTRUST1`. Each subsequent line contains the application ID, the 64 lowercase hex digits of the publisher's raw Ed25519 public key, a hexadecimal capability ceiling, and the decimal priority ceiling, separated by whitespace. The Creator signs `PurpleOS/APP-TRUST/v1`, a NUL byte, then the complete policy bytes. The detached signature is 512 ASCII hex digits. The kernel loads a verified boot-scoped snapshot. Key removal takes effect on reboot with the updated signed policy. The format has no hardware-backed rollback counter.

Stage the bundle, `trust.policy` and `trust.policy.sig` using `build/mkpfs2_disk --applications-dir <directory> --signing-key <creator-key>`. The disk builder installs the files in `/System/Applications/` and includes them in the signed system component manifest. Rebuild the system image to deploy changes. Launch with the P menu or `launch <app-id>` in AegisShell. Menu entries come from the boot policy and installed filenames. Every launch still verifies the actual bundle and executable. The system launches with an empty application catalog when no trust policy is installed.

PurpleFS v2 limits the canonical app ID to 21 bytes. Bundles are limited to 512 KiB. The first client window uses the signed title and dimensions, constrained by the active framebuffer. Up to four windows per task share the compositor's global window limit.

## Calls

Use the x86-64 `syscall` instruction. The number is in RAX, the first three arguments in RDI, RSI, and RDX, and the signed return value is in RAX. RCX and R11 are clobbered. Negative values are errno codes. The kernel rejects calls outside this table and the explicitly allowed process calls.

| Number | Arguments | Result |
| --- | --- | --- |
| 0x1100 | None | GUI ABI version |
| 0x1101 | None | Address of the immutable 64-byte startup record |
| 0x1102 | NUL-terminated UTF-8 title, width, height | New task-owned window handle |
| 0x1103 | Window handle | Close an owned window |
| 0x1104 | Window handle, pixels, byte count | Copy one complete XRGB8888 frame; ARGB8888 when glass mode is enabled |
| 0x1105 | Window handle, writable event address | 1 for one event, 0 for an empty queue |
| 0x1106 | Window handle | Width in high 32 bits and height in low 32 bits |
| 0x1107 | UTF-8 input address, length | Write up to 256 bytes to serial with Console permission |
| 0x1108 | Nonzero byte count | Map zeroed, writable and nonexecutable memory |
| 0x1109 | Mapping base | Free the entire mapping at that exact base |
| 0x110c | Window handle, enabled (0 or 1) | Enable or disable live backdrop glass for an owned window |

Call `0x110a` is available to applications with the Shell capability (bit `0x04`). It accepts a command pointer, its byte length (up to 240 printable ASCII bytes), and a pointer to a writable 4096-byte output buffer. It returns the number of captured bytes or a negative errno. The kernel resolves each command through the authenticated command database and dispatches supported AegisShell builtins. The GUI shell supports help, navigation, text file reads, echo, and basic system status. The kernel supports up to four signed pipeline stages with supported stream commands. Output redirection with `>` and `>>` can replace or append to regular files in `/tmp/`. The kernel rejects input redirection, command chaining, background execution, and raw device reads.

Call `0x110b` takes a writable buffer pointer and its capacity, and returns the byte length of the current Shell prompt. It requires the Shell capability. Use a 96-byte buffer and refresh after each command to reflect changes to the working directory.

Calls 24, 39, 60 and 231 provide yield, process ID and termination. The startup record is eight little-endian u64 fields in header order and points to read-only resource and metadata bytes. A zero pointer and length indicate an empty section. Heap mappings round up to a page; at most 32 regions and 32 MiB are allowed per process. Mapped windows are kernel surfaces. The pixel source remains user-owned; the compositor copies it before returning.

The event record is four little-endian 32-bit words. The fields are kind, x, y and detail. Pointer events report signed client-relative x and y and the PS/2 button mask in detail. Key events place a cooked keyboard byte in x. Focus uses x=1 for gain and x=0 for loss. Resize uses width in x and height in y; query the current window size before submitting the next frame. The close request asks the app to close its window or exit, and has priority over queued input. Overflow indicates that older input was dropped. Motion with an identical button state may be coalesced.

ABI v1 enables the GUI and optional Console and Shell capabilities for system-installed clients. It has no user-profile installation, filesystem or network capability, clipboard, IME, accessibility service, guest GPU API, or toolkit. Use the resource section for bundled assets and software-render content into the pixel buffer.

Native windows are opaque by default. A client can call `0x110c` before its first presentation to enable live acrylic: the compositor blurs and tints the scene below the client area, then alpha-composites an ARGB8888 frame. Transparent client pixels reveal the glass; fully opaque pixels retain normal app rendering. The C helper is `purple_gui_set_glass`.
