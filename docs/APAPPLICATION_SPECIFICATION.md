# Purple OS / Aegis64 .apapplication Bundle Specification

**Status:** System-installed GUI bundles launch through the [GUI runtime ABI](PURPLE_GUI_ABI.md). User-profile installation and the additional capability classes in this format remain reserved for future runtime support.

## 1. File format

The canonical extension is `.apapplication`. Version 2 uses the eight ASCII bytes `APAPP200`. AegisDetect rejects version 1 and unknown versions. Existing version 1 bundles must be repackaged and signed under version 2.

A bundle contains a fixed 336-byte manifest followed by the mandatory executable and up to two optional sections. All integer fields are unsigned and little-endian. Offsets are absolute byte offsets from the start of the file. Decode fields directly from the file bytes.

| Byte offset | Size | Field |
| --- | ---: | --- |
| 0x000 | 8 | Magic, `APAPP200` |
| 0x008 | 4 | Format version, 2 |
| 0x00C | 4 | Manifest size, 336 |
| 0x010 | 32 | Application ID |
| 0x030 | 32 | Display name |
| 0x050 | 16 | Application version label |
| 0x060 | 32 | Author display label |
| 0x080 | 64 | Description |
| 0x0C0 | 4 | Requested capability bitmask |
| 0x0C4 | 4 | Priority, 1 low, 2 normal, 3 high |
| 0x0C8 | 4 | Initial client width in pixels |
| 0x0CC | 4 | Initial client height in pixels |
| 0x0D0 | 8 | Executable offset and size, two u32 fields |
| 0x0D8 | 8 | Resource offset and size, two u32 fields |
| 0x0E0 | 8 | Reserved library offset and size, both zero in version 2 |
| 0x0E8 | 8 | Metadata offset and size, two u32 fields |
| 0x0F0 | 32 | Signer key ID |
| 0x110 | 64 | Ed25519 signature |

Text fields contain valid UTF-8 followed by a NUL and zero padding. Reject control characters. The app ID, display name, and version label must be nonempty. The app ID uses one or more lowercase ASCII alphanumeric segments separated by single dots. The filename extension is separate from the ID. The author is a display label. Use the verified signing key's identity for publisher attribution.

Priority must be in the listed range. GUI apps must specify nonzero width and height within the system's client surface limits. Both dimensions must be zero for apps without the GUI capability.

The executable starts at byte 336 and must have nonzero size. Present optional sections follow in table order, immediately after the previous present section. An absent section has offset and size both zero. Reject mixed-zero pairs, gaps, overlaps, trailing bytes, arithmetic overflow and ranges outside the file. Apply a system bundle size limit before allocating a snapshot. Version 2 has no standalone CRC field.

The executable is a complete [APEXEC binary](APEXEC_FORMAT_SPECIFICATION.md). Resources and metadata are optional, opaque data sections. Applications may define their own encoding for these bytes. Their contents confer no capabilities or executable mappings. Shared library loading requires a future format revision.

## 2. AegisDetect verification

Version 2 uses pure Ed25519 as specified in [RFC 8032](https://www.rfc-editor.org/rfc/rfc8032). The key ID is the 32-byte SHA-256 digest of the raw 32-byte Ed25519 public key. The 64-byte signature field carries the raw signature. The signed message is:

```text
ASCII bytes "PurpleOS/APAPPLICATION/v2" followed by one NUL byte
|| bundle bytes [0, 272)
|| 64 zero bytes
|| bundle bytes [336, end)
```

The ranges are half-open. Every manifest field and payload section is authenticated. The signature is verified using the trusted public key selected by the signed key ID.

AegisDetect performs these steps for each launch:

1. Obtain a bounded, immutable snapshot of the package. Check the manifest and section layout against that snapshot.
2. Resolve the key ID through the system trust store and verify the signature over the specified message. Reject unknown, disabled or revoked keys and malformed or invalid signatures.
3. Evaluate signer policy, app identity, installation scope and local policy. Reject requests that exceed the allowed capabilities or priority.
4. Pass the same verified bytes and granted capability set to the loader. Keep resource and metadata access bound to that snapshot throughout the task lifetime.

The GUI runtime loads a Creator-signed boot policy with publisher public keys, authorized system app IDs, and capability and priority ceilings. Bundles cannot change this policy. Key rotation or revocation requires deployment of a replacement policy and reboot. Publisher private keys stay with the external signing tool or a hardware-backed key store. ABI v1 does not implement hardware-backed policy rollback protection.

## 3. Capabilities

| Flag | Value | Access |
| --- | --- | --- |
| `APAPP_PERM_CONSOLE` | 0x01 | Standard console I/O |
| `APAPP_PERM_GUI` | 0x02 | Owned windows and their input events |
| `APAPP_PERM_STORAGE` | 0x04 | User files within explicitly granted scopes |
| `APAPP_PERM_NETWORK` | 0x08 | Network operations permitted by local policy |
| `APAPP_PERM_SYSADMIN` | 0x10 | Explicitly authorized privileged maintenance operations |

The manifest requests capabilities. AegisDetect rejects unknown bits and denies launch if any requested capability falls outside either signer policy or local policy. Only an explicitly authorized system signer and app identity may receive SYSADMIN for a system-installed bundle. The kernel binds granted capabilities to the task and checks both the capability and object scope on each protected operation. SYSADMIN does not bypass userspace isolation or object ownership.

## 4. Discovery and runtime

Core applications reside in `/System/Applications/`. User-installed applications reside in `/System/Users/<user>/Applications/`. The launcher resolves the system location first. If a system candidate exists but verification fails, the launch fails. It does not fall back to a user copy. `/System/Kernel/Applications/` is invalid. The application manager checks the verified manifest ID against the requested ID or registered launch entry.

1. AegisDetect supplies the verified bundle and authorization to the APEXEC loader. The loader validates the executable, creates a private user address space, maps code and data with the permissions specified by APEXEC, and prepares the user stack and entry point.
2. The application service maps present resource and metadata sections read-only. For a GUI app, it provisions an initial client window with the manifest title and size and prepares its handle before starting the task.
3. Launch passes the initial handles through a versioned startup interface. The GUI service accepts pixel updates from task-owned buffers through checked window handles and routes input and close events to the owning task. The compositor owns the framebuffer. Handles are scoped to the task and invalidated on close or task exit.
4. If validation, mapping, handle creation, window provisioning, or task startup fails, the service releases the resources acquired for that launch and returns an error. Normal exit or termination closes owned windows and handles and releases task resources.
5. Report distinct errors for malformed packages, unsupported formats, signature or trust failures, capability denial, invalid executables, and runtime resource failures.

See the [GUI runtime ABI](PURPLE_GUI_ABI.md) for the startup record, syscall numbers, events, pixel buffers, and implemented capability limits.
