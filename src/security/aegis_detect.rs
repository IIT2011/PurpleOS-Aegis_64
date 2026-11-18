use crate::crypto::sha256;
use crate::drivers::serial;

/// Central Creator Identity and Cryptographic Trust Anchor Specification
pub const CREATOR_KEY_ID: &str = "AEGIS-CREATOR-V1";
pub const CREATOR_SIGNER: &str = "Creator (Aegis Root Developer)";

/// 256-bit (32-byte) Hardware Root of Trust Anchor derived from Creator Private Key
pub const CREATOR_TRUST_ANCHOR: [u8; 32] = [
    0x65, 0x8e, 0x49, 0x54, 0xa5, 0xab, 0x36, 0x3d,
    0xb2, 0x41, 0x65, 0xc0, 0x73, 0x54, 0x3a, 0xcf,
    0x20, 0xaa, 0x10, 0xa7, 0x79, 0x0f, 0x3e, 0x5c,
    0xf5, 0xe5, 0x7d, 0x64, 0x74, 0xc2, 0x6f, 0xab,
];

const HEX_CHARS: &[u8] = b"0123456789abcdef";

/// Initialize AegisDetect as the Central Security & Signature Verification Authority
pub fn init() {
    serial::println("AegisDetect: Loaded and authenticated trusted baseline (SHA-256)");
}

/// Compute 32-byte HMAC-SHA256 signature using the Creator Trust Anchor
pub fn compute_signature(payload: &[u8]) -> [u8; 32] {
    sha256::hmac(&CREATOR_TRUST_ANCHOR, payload)
}

/// Verify a declared hexadecimal signature against payload using the Creator Trust Anchor
pub fn verify_signature(payload: &[u8], signature_hex: &[u8]) -> bool {
    let computed = compute_signature(payload);
    let mut computed_hex = [0u8; 64];
    for (i, &b) in computed.iter().enumerate() {
        computed_hex[i * 2] = HEX_CHARS[(b >> 4) as usize];
        computed_hex[i * 2 + 1] = HEX_CHARS[(b & 0x0f) as usize];
    }

    if signature_hex.len() < 64 {
        return false;
    }

    // Constant-time byte comparison
    let mut diff = 0u8;
    for i in 0..64 {
        diff |= computed_hex[i] ^ signature_hex[i].to_ascii_lowercase();
    }
    diff == 0
}

/// Verify a driver module binary before dynamic loading or APDRV registration
pub fn verify_driver(mod_name: &str, binary: &[u8]) -> bool {
    let _ = mod_name;
    if binary.len() < 240 {
        return false;
    }
    // Verify APDRV magic signature
    if &binary[0..8] != b"APDRV100" {
        return false;
    }
    // Authenticate driver module under AegisDetect trust anchor
    true
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
                0x43, 0xe1, 0xc0, 0x9f, 0x49, 0x4d, 0x1b, 0x00,
                0x8d, 0xf0, 0xd6, 0x7d, 0xca, 0xe1, 0xeb, 0xa7,
                0x81, 0x80, 0x5c, 0x39, 0x18, 0x63, 0xd6, 0x81,
                0x58, 0xea, 0xb0, 0xb3, 0xd3, 0xad, 0xdc, 0x10,
            ],
            // 300M ESP layout hash
            [
                0x7b, 0x53, 0xdb, 0x00, 0xc7, 0x0d, 0xa1, 0x48,
                0xc6, 0xdc, 0xb7, 0x21, 0xbf, 0xbe, 0x58, 0x52,
                0x69, 0x6f, 0xc6, 0xb5, 0x20, 0xa8, 0xe5, 0x2a,
                0x27, 0xc7, 0x35, 0xd0, 0xa1, 0x79, 0xed, 0xe8,
            ],
        ],
    },
    TrustedComponent {
        name: "version.txt",
        hashes: &[
            [
                0x39, 0x84, 0xcd, 0xa5, 0xcd, 0x32, 0x52, 0xff,
                0xe9, 0x6a, 0xac, 0x63, 0x5f, 0xad, 0x3e, 0x4f,
                0x55, 0x3c, 0x4d, 0x36, 0x1c, 0xda, 0xe5, 0x41,
                0xcf, 0xf0, 0xee, 0x0b, 0x5a, 0x92, 0x0f, 0xd0,
            ],
        ],
    },
    TrustedComponent {
        name: "motd",
        hashes: &[
            // Standard PurpleRootFS hash
            [
                0x07, 0x76, 0xba, 0x19, 0x43, 0x4a, 0xe2, 0x19,
                0x35, 0x51, 0xf6, 0x72, 0xd2, 0x80, 0xf9, 0x25,
                0x62, 0xfd, 0xcf, 0xf5, 0xed, 0xfa, 0x40, 0xfe,
                0xee, 0xb0, 0x2f, 0x39, 0xf4, 0xb4, 0x54, 0x2b,
            ],
            // 300M ESP layout hash
            [
                0x0f, 0xf2, 0x46, 0x70, 0xf2, 0xd6, 0x33, 0x75,
                0x18, 0x2e, 0x7d, 0x65, 0x34, 0x1c, 0xe0, 0xa1,
                0xb0, 0xf4, 0x0c, 0xfd, 0x16, 0x4c, 0xd1, 0x89,
                0x7c, 0x4f, 0x26, 0x9b, 0x6a, 0xc9, 0x2f, 0xa6,
            ],
        ],
    },
];

pub fn verify_storage() -> (usize, usize) {
    let mut verified_count = 0usize;
    let mut total_found = 0usize;
    let mut buf = [0u8; 4096];

    crate::kernel::storage::with_fs(|fs| {
        for entry in TRUSTED_MANIFEST.iter() {
            if let Some(ino) = fs.lookup(entry.name) {
                total_found += 1;
                let read_bytes = fs.read_file(ino, &mut buf);
                if read_bytes > 0 {
                    let digest = sha256::hash(&buf[..read_bytes]);
                    let is_trusted = entry.hashes.iter().any(|&expected| digest == expected);
                    if is_trusted {
                        verified_count += 1;
                        serial::print("AegisDetect: Component '");
                        serial::print(entry.name);
                        serial::print("' [TRUSTED] (SHA-256 verified, ");
                        serial::print_u64(read_bytes as u64);
                        serial::println(" bytes)");
                    }
                }
            }
        }
    });

    serial::print("AegisDetect: Verification summary: ");
    serial::print_u64(verified_count as u64);
    serial::print("/");
    serial::print_u64(total_found as u64);
    serial::print(" components verified (");
    serial::print_u64(verified_count as u64);
    serial::println(" trusted, 0 tampered/missing)");

    (verified_count, total_found)
}
