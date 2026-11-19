//! Pure Rust (#![no_std]) RSA-2048 PKCS#1 v1.5 Cryptographic Signature Verifier
//!
//! Provides RSA-2048 PKCS#1 v1.5 signature verification with public exponent
//! e = 65537 (0x10001) and SHA-256.
//!
//! This implementation uses no heap allocation and is written for bare-metal
//! Rust. Signature verification processes public inputs; this is not a claim of
//! constant-time behavior or a guarantee against every implementation attack.

use crate::crypto::sha256;

pub const RSA_2048_BYTES: usize = 256;
pub const RSA_2048_WORDS: usize = 32;

/// ASN.1 DigestInfo prefix for SHA-256 (RFC 8017 / PKCS #1 v2.2)
pub const SHA256_ASN1_PREFIX: [u8; 19] = [
    0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01,
    0x05, 0x00, 0x04, 0x20,
];

/// 2048-bit Unsigned Big Integer (stored as 32 little-endian 64-bit words)
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct BigUint2048 {
    pub words: [u64; RSA_2048_WORDS],
}

impl BigUint2048 {
    pub const fn zero() -> Self {
        Self { words: [0u64; RSA_2048_WORDS] }
    }

    /// Parse big-endian 256-byte array into 2048-bit little-endian words
    pub fn from_be_bytes(bytes: &[u8; RSA_2048_BYTES]) -> Self {
        let mut words = [0u64; RSA_2048_WORDS];
        for i in 0..RSA_2048_WORDS {
            let offset = (RSA_2048_WORDS - 1 - i) * 8;
            words[i] = u64::from_be_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]);
        }
        Self { words }
    }

    /// Export 2048-bit number as big-endian 256-byte array
    pub fn to_be_bytes(&self, out: &mut [u8; RSA_2048_BYTES]) {
        for i in 0..RSA_2048_WORDS {
            let chunk = self.words[RSA_2048_WORDS - 1 - i].to_be_bytes();
            out[i * 8..(i + 1) * 8].copy_from_slice(&chunk);
        }
    }

    pub fn is_less_than(&self, other: &Self) -> bool {
        for i in (0..RSA_2048_WORDS).rev() {
            if self.words[i] < other.words[i] {
                return true;
            } else if self.words[i] > other.words[i] {
                return false;
            }
        }
        false
    }
}

/// Multiply two 2048-bit numbers to produce a 4096-bit product
pub fn mul_wide(a: &[u64; RSA_2048_WORDS], b: &[u64; RSA_2048_WORDS]) -> [u64; 64] {
    let mut out = [0u64; 64];
    for i in 0..RSA_2048_WORDS {
        let mut carry = 0u128;
        for j in 0..RSA_2048_WORDS {
            let prod = (a[i] as u128) * (b[j] as u128) + (out[i + j] as u128) + carry;
            out[i + j] = prod as u64;
            carry = prod >> 64;
        }
        let mut k = i + RSA_2048_WORDS;
        while carry > 0 && k < 64 {
            let sum = (out[k] as u128) + carry;
            out[k] = sum as u64;
            carry = sum >> 64;
            k += 1;
        }
    }
    out
}

#[inline(always)]
fn ge_33_32(rem: &[u64; 33], n: &[u64; 32]) -> bool {
    if rem[32] > 0 {
        return true;
    }
    for i in (0..32).rev() {
        if rem[i] > n[i] {
            return true;
        } else if rem[i] < n[i] {
            return false;
        }
    }
    true
}

#[inline(always)]
fn sub_33_32(rem: &mut [u64; 33], n: &[u64; 32]) {
    let mut borrow = 0u64;
    for i in 0..32 {
        let (d1, b1) = rem[i].overflowing_sub(n[i]);
        let (d2, b2) = d1.overflowing_sub(borrow);
        rem[i] = d2;
        borrow = (b1 as u64) + (b2 as u64);
    }
    rem[32] = rem[32].wrapping_sub(borrow);
}

/// Compute 4096-bit number modulo 2048-bit modulus N
pub fn rem_wide(a: &[u64; 64], n: &[u64; 32]) -> [u64; 32] {
    let mut rem = [0u64; 33];
    for w_idx in (0..64).rev() {
        let word = a[w_idx];
        for bit_idx in (0..64).rev() {
            let bit = (word >> bit_idx) & 1;
            let mut carry = bit;
            for i in 0..33 {
                let next_carry = rem[i] >> 63;
                rem[i] = (rem[i] << 1) | carry;
                carry = next_carry;
            }
            if ge_33_32(&rem, n) {
                sub_33_32(&mut rem, n);
            }
        }
    }
    let mut out = [0u64; 32];
    out.copy_from_slice(&rem[..32]);
    out
}

/// Multiply two 2048-bit numbers modulo N
pub fn mul_mod(a: &[u64; 32], b: &[u64; 32], n: &[u64; 32]) -> [u64; 32] {
    let wide = mul_wide(a, b);
    rem_wide(&wide, n)
}

/// Modular exponentiation for e = 65537: S^65537 mod N
pub fn mod_pow_65537(s: &[u64; 32], n: &[u64; 32]) -> [u64; 32] {
    let mut x = *s;
    for _ in 0..16 {
        x = mul_mod(&x, &x, n);
    }
    mul_mod(&x, s, n)
}

/// Verify an RSA-2048 PKCS#1 v1.5 signature with SHA-256
pub fn verify_rsa_2048_pkcs1_v15(
    n_bytes: &[u8; RSA_2048_BYTES],
    payload: &[u8],
    sig_bytes: &[u8; RSA_2048_BYTES],
) -> bool {
    let modulus = BigUint2048::from_be_bytes(n_bytes);
    let signature = BigUint2048::from_be_bytes(sig_bytes);

    // Signature must be strictly less than modulus
    if !signature.is_less_than(&modulus) {
        return false;
    }

    // Compute M = S^65537 mod N
    let recovered_words = mod_pow_65537(&signature.words, &modulus.words);
    let recovered = BigUint2048 { words: recovered_words };

    let mut em = [0u8; RSA_2048_BYTES];
    recovered.to_be_bytes(&mut em);

    // Check PKCS#1 v1.5 padding:
    // EM = 0x00 || 0x01 || PS (0xFF...) || 0x00 || T (ASN.1 SHA-256) || H (SHA-256)
    // For 2048-bit RSA:
    // EM is 256 bytes (indices 0..255).
    // byte 0: 0x00
    // byte 1: 0x01
    // PS: 202 bytes of 0xFF (indices 2..204: 2, 3, ..., 203)
    // separator: 1 byte of 0x00 (index 204)
    // ASN.1 prefix: 19 bytes (indices 205..224: 205, 206, ..., 223)
    // SHA-256: 32 bytes (indices 224..256: 224, 225, ..., 255)
    if em[0] != 0x00 || em[1] != 0x01 {
        return false;
    }

    let mut diff = 0u8;
    for i in 2..204 {
        diff |= em[i] ^ 0xFF;
    }
    diff |= em[204] ^ 0x00;

    // Verify ASN.1 header (19 bytes)
    for i in 0..19 {
        diff |= em[205 + i] ^ SHA256_ASN1_PREFIX[i];
    }

    // Verify SHA-256 digest of payload (32 bytes)
    let expected_hash = sha256::hash(payload);
    for i in 0..32 {
        diff |= em[224 + i] ^ expected_hash[i];
    }

    diff == 0
}
