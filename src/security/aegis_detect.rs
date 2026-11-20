use crate::crypto::rsa;
use crate::crypto::sha256;
use crate::drivers::serial;

/// Central Creator Identity and Cryptographic Trust Anchor Specification
pub const CREATOR_KEY_ID: &str = "AEGIS-CREATOR-V1";
pub const APDRV_SIGNATURE_HEX_SIZE: usize = rsa::RSA_2048_BYTES * 2;
pub const CREATOR_SIGNER: &str = "Creator (Aegis Root Developer)";

pub use crate::crypto::creator::CREATOR_RSA_MODULUS;

#[inline(always)]
fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Initialize AegisDetect as the Central Security & Signature Verification Authority
pub fn init() {
    serial::println("AegisDetect: RSA-2048 trust anchor initialized; storage checks run when a filesystem is mounted");
}

/// Gate startup before external drivers, the desktop, or the shell can run.
pub fn enforce_boot_policy() {
    if !crate::kernel::storage::is_mounted() {
        halt_boot("required system volume is unavailable");
    }
    let (verified, required, _, _) = verify_storage();
    if verified != required {
        halt_boot("required system files failed integrity verification");
    }
    const MANIFEST_PATH: &str = "/System/components.manifest";
    let has_manifest = crate::fs::vfs::file_size(MANIFEST_PATH).is_ok();
    if (has_manifest || cfg!(feature = "signed-components"))
        && !verify_component_manifest(MANIFEST_PATH)
    {
        halt_boot("native component manifest failed verification");
    }
    if !crate::kernel::cmddb::init() {
        halt_boot("required command database failed signature verification");
    }
    crate::kernel::pathdb::init();
}

fn verify_component_manifest(path: &str) -> bool {
    const MAX_MANIFEST: usize = 16_384;
    let mut bytes = alloc::vec::Vec::new();
    if bytes.try_reserve_exact(MAX_MANIFEST + 1).is_err() {
        return false;
    }
    bytes.resize(MAX_MANIFEST + 1, 0);
    let Ok(len @ 1..=MAX_MANIFEST) = crate::fs::vfs::read(path, &mut bytes) else {
        return false;
    };
    let payload = &bytes[..len];
    if !verify_control_file(path, b"PurpleOS/COMPONENTS/v1\0", payload) {
        return false;
    }
    let mut previous = "";
    let mut scratch = [0; 4096];
    let Ok(records) = core::str::from_utf8(payload) else {
        return false;
    };
    for line in records.lines() {
        let Some((digest, name)) = line.split_once("  ") else {
            return false;
        };
        if digest.len() != 64
            || name <= previous
            || name == "System/CLI/path.dtbase"
            || name
                .split('/')
                .any(|part| part.is_empty() || matches!(part, "." | ".."))
        {
            return false;
        }
        previous = name;
        let verified = crate::kernel::storage::with_fs(|fs| {
            let inode = fs.lookup(name)?;
            let size = fs.get_file_size(inode)?;
            let mut hash = sha256::Sha256::new();
            let mut offset = 0;
            while offset < size {
                let count = (size - offset).min(scratch.len());
                if fs.read_file_at(inode, offset, &mut scratch[..count]) != count {
                    return None;
                }
                hash.update(&scratch[..count]);
                offset += count;
            }
            Some(
                hash.finalize()
                    .iter()
                    .zip(digest.as_bytes().chunks_exact(2))
                    .all(|(&byte, hex)| {
                        hex_val(hex[0])
                            .zip(hex_val(hex[1]))
                            .is_some_and(|(hi, lo)| byte == (hi << 4 | lo))
                    }),
            )
        });
        if verified != Some(Some(true)) {
            return false;
        }
    }
    serial::println("AegisDetect: Native component manifest verified");
    !previous.is_empty()
}

pub(crate) fn halt_boot(reason: &str) -> ! {
    serial::print("AegisDetect: Boot blocked: ");
    serial::println(reason);
    crate::arch::x86_64::cpu::cli();
    loop {
        crate::arch::x86_64::cpu::hlt();
    }
}

/// Verify a declared 512-hex-character RSA-2048 PKCS#1 v1.5 signature against payload using Creator Public Key
fn verify_signature(payload: &[u8], signature_hex: &[u8]) -> bool {
    if signature_hex.len() != 512 {
        return false;
    }

    let mut sig_bytes = [0u8; 256];
    for i in 0..256 {
        let hi = match hex_val(signature_hex[i * 2]) {
            Some(v) => v,
            None => return false,
        };
        let lo = match hex_val(signature_hex[i * 2 + 1]) {
            Some(v) => v,
            None => return false,
        };
        sig_bytes[i] = (hi << 4) | lo;
    }

    rsa::verify_rsa_2048_pkcs1_v15(&CREATOR_RSA_MODULUS, payload, &sig_bytes)
}

/// Authenticate the exact command records passed to the database parser.
pub fn verify_command_database(data: &[u8]) -> Option<&[u8]> {
    let mut signature = None;
    let mut offset = 0;
    for raw_line in data.split_inclusive(|&byte| byte == b'\n') {
        let line = raw_line.trim_ascii();
        if let Some(value) = line.strip_prefix(b"# SIGNATURE:") {
            let value = value.trim_ascii();
            if signature.is_some() || value.len() != APDRV_SIGNATURE_HEX_SIZE {
                return None;
            }
            signature = Some(value);
        } else if !line.is_empty() && !line.starts_with(b"#") {
            let payload = &data[offset..];
            return verify_signature(payload, signature?).then_some(payload);
        }
        offset += raw_line.len();
    }
    None
}

/// Verify a detached Creator signature over a purpose tag and the complete file.
pub fn verify_control_file(path: &str, purpose: &[u8], payload: &[u8]) -> bool {
    use alloc::{string::String, vec::Vec};

    let mut sidecar = String::new();
    let mut message = Vec::new();
    if sidecar.try_reserve_exact(path.len() + 4).is_err()
        || message
            .try_reserve_exact(purpose.len() + payload.len())
            .is_err()
    {
        return false;
    }
    sidecar.push_str(path);
    sidecar.push_str(".sig");
    let mut signature = [0; APDRV_SIGNATURE_HEX_SIZE + 1];
    if crate::fs::vfs::read(&sidecar, &mut signature) != Ok(APDRV_SIGNATURE_HEX_SIZE) {
        return false;
    }
    message.extend_from_slice(purpose);
    message.extend_from_slice(payload);
    verify_signature(&message, &signature[..APDRV_SIGNATURE_HEX_SIZE])
}

/// Verify an APDRV package whose final 512 ASCII bytes hold an RSA signature
/// over every preceding byte. Unsigned packages are rejected.
pub fn verify_driver(_mod_name: &str, binary: &[u8]) -> bool {
    let header_size = core::mem::size_of::<crate::kernel::apdrv::ApdrvHeader>();
    if binary.len() < header_size + APDRV_SIGNATURE_HEX_SIZE {
        return false;
    }

    let payload_end = binary.len() - APDRV_SIGNATURE_HEX_SIZE;
    verify_signature(&binary[..payload_end], &binary[payload_end..])
}

struct TrustedComponent {
    name: &'static str,
    hashes: &'static [[u8; 32]],
}

const TRUSTED_MANIFEST: &[TrustedComponent] = &[
    TrustedComponent {
        name: "welcome.txt",
        hashes: &[
            // Standard PurpleRootFS hash
            [
                0x43, 0xe1, 0xc0, 0x9f, 0x49, 0x4d, 0x1b, 0x00, 0x8d, 0xf0, 0xd6, 0x7d, 0xca, 0xe1,
                0xeb, 0xa7, 0x81, 0x80, 0x5c, 0x39, 0x18, 0x63, 0xd6, 0x81, 0x58, 0xea, 0xb0, 0xb3,
                0xd3, 0xad, 0xdc, 0x10,
            ],
            // 300M ESP layout hash
            [
                0x7b, 0x53, 0xdb, 0x00, 0xc7, 0x0d, 0xa1, 0x48, 0xc6, 0xdc, 0xb7, 0x21, 0xbf, 0xbe,
                0x58, 0x52, 0x69, 0x6f, 0xc6, 0xb5, 0x20, 0xa8, 0xe5, 0x2a, 0x27, 0xc7, 0x35, 0xd0,
                0xa1, 0x79, 0xed, 0xe8,
            ],
        ],
    },
    TrustedComponent {
        name: "version.txt",
        hashes: &[[
            0x39, 0x84, 0xcd, 0xa5, 0xcd, 0x32, 0x52, 0xff, 0xe9, 0x6a, 0xac, 0x63, 0x5f, 0xad,
            0x3e, 0x4f, 0x55, 0x3c, 0x4d, 0x36, 0x1c, 0xda, 0xe5, 0x41, 0xcf, 0xf0, 0xee, 0x0b,
            0x5a, 0x92, 0x0f, 0xd0,
        ]],
    },
    TrustedComponent {
        name: "motd",
        hashes: &[
            // Standard PurpleRootFS hash
            [
                0x07, 0x76, 0xba, 0x19, 0x43, 0x4a, 0xe2, 0x19, 0x35, 0x51, 0xf6, 0x72, 0xd2, 0x80,
                0xf9, 0x25, 0x62, 0xfd, 0xcf, 0xf5, 0xed, 0xfa, 0x40, 0xfe, 0xee, 0xb0, 0x2f, 0x39,
                0xf4, 0xb4, 0x54, 0x2b,
            ],
            // 300M ESP layout hash
            [
                0x0f, 0xf2, 0x46, 0x70, 0xf2, 0xd6, 0x33, 0x75, 0x18, 0x2e, 0x7d, 0x65, 0x34, 0x1c,
                0xe0, 0xa1, 0xb0, 0xf4, 0x0c, 0xfd, 0x16, 0x4c, 0xd1, 0x89, 0x7c, 0x4f, 0x26, 0x9b,
                0x6a, 0xc9, 0x2f, 0xa6,
            ],
        ],
    },
];

pub fn verify_storage() -> (usize, usize, usize, usize) {
    let mut verified_count = 0usize;
    let mut total_found = 0usize;
    let mut unverified_present = 0usize;
    let mut buf = [0u8; 4096];

    crate::kernel::storage::with_fs(|fs| {
        for entry in TRUSTED_MANIFEST.iter() {
            if let Some(ino) = fs.lookup(entry.name) {
                total_found += 1;
                let expected_len = fs.get_file_size(ino);
                let read_bytes = match expected_len {
                    Some(len) if len > 0 && len <= buf.len() => fs.read_file(ino, &mut buf[..len]),
                    _ => 0,
                };
                if expected_len == Some(read_bytes) && read_bytes > 0 {
                    let digest = sha256::hash(&buf[..read_bytes]);
                    let is_trusted = entry.hashes.iter().any(|&expected| digest == expected);
                    if is_trusted {
                        verified_count += 1;
                        serial::print("AegisDetect: Component '");
                        serial::print(entry.name);
                        serial::print("' [TRUSTED] (SHA-256 verified, ");
                        serial::print_u64(read_bytes as u64);
                        serial::println(" bytes)");
                    } else {
                        unverified_present += 1;
                        serial::print("AegisDetect: Component '");
                        serial::print(entry.name);
                        serial::println("' [UNVERIFIED]");
                    }
                } else {
                    unverified_present += 1;
                    serial::print("AegisDetect: Component '");
                    serial::print(entry.name);
                    serial::println("' could not be read");
                }
            }
        }
    });

    serial::print("AegisDetect: Verification summary: ");
    serial::print_u64(verified_count as u64);
    serial::print("/");
    serial::print_u64(TRUSTED_MANIFEST.len() as u64);
    let missing = TRUSTED_MANIFEST.len().saturating_sub(total_found);
    serial::print(" components verified (");
    serial::print_u64(verified_count as u64);
    serial::print(" trusted, ");
    serial::print_u64(unverified_present as u64);
    serial::print(" present but unverified, ");
    serial::print_u64(missing as u64);
    serial::println(" missing)");

    (
        verified_count,
        TRUSTED_MANIFEST.len(),
        unverified_present,
        missing,
    )
}

/// An active signer authorization obtained from trusted system policy.
pub struct AppSigner<'a> {
    pub public_key: &'a [u8; 32],
    pub app_id: &'a str,
    pub scope: crate::kernel::apapplication::Scope,
    pub capability_ceiling: u32,
    pub priority_ceiling: u32,
}

pub struct VerifiedApplication<'a> {
    pub(crate) bundle: crate::kernel::apapplication::Bundle<'a>,
    pub(crate) image: crate::kernel::apexec::Image<'a>,
}

pub fn verify_application<'a>(
    bytes: &'a [u8],
    requested_id: &str,
    scope: crate::kernel::apapplication::Scope,
    signers: &[AppSigner<'_>],
    local_policy: (u32, u32),
    size_limit: usize,
    max_surface: (u32, u32),
) -> Result<VerifiedApplication<'a>, crate::kernel::apapplication::Error> {
    use crate::kernel::apapplication::{Bundle, Error, Scope, SYSADMIN};
    use ed25519_dalek::{Signature, VerifyingKey};

    const DOMAIN: &[u8] = b"PurpleOS/APAPPLICATION/v2\0";
    let bundle = Bundle::parse(bytes, size_limit, max_surface)?;
    let mut trusted = signers.iter().filter(|signer| {
        signer.app_id == requested_id
            && signer.scope == scope
            && crate::crypto::sha256::hash(signer.public_key) == *bundle.key_id
    });
    let signer = trusted.next().ok_or(Error::Trust)?;
    if trusted.next().is_some() {
        return Err(Error::Trust);
    }
    let key = VerifyingKey::from_bytes(signer.public_key).map_err(|_| Error::Trust)?;
    let capacity = DOMAIN.len().checked_add(bytes.len()).ok_or(Error::Memory)?;
    let mut message = crate::mm::pmm::PageBuffer::new(capacity).ok_or(Error::Memory)?;
    let (domain, manifest) = message.split_at_mut(DOMAIN.len());
    domain.copy_from_slice(DOMAIN);
    manifest[..0x110].copy_from_slice(&bytes[..0x110]);
    manifest[0x150..].copy_from_slice(&bytes[0x150..]);
    key.verify_strict(&message, &Signature::from_bytes(bundle.signature))
        .map_err(|_| Error::Signature)?;
    drop(message);

    if bundle.id != requested_id
        || bundle.capabilities & !(local_policy.0 & signer.capability_ceiling) != 0
        || bundle.priority > local_policy.1.min(signer.priority_ceiling)
        || (scope != Scope::System && bundle.capabilities & SYSADMIN != 0)
    {
        return Err(Error::Policy);
    }
    let image =
        crate::kernel::apexec::Image::parse(bundle.executable).map_err(Error::Executable)?;
    Ok(VerifiedApplication { bundle, image })
}
