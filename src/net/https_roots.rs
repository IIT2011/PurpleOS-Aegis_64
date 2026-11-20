use alloc::vec::Vec;

use crate::{drivers::serial, fs::vfs, security::aegis_detect, sync::Spinlock};

// System image layout and signature domain shared with stage_https_roots.py.
const PATH: &str = "/System/Network/https-roots.pem";
const SIGNATURE_DOMAIN: &[u8] = b"PurpleOS/HTTPS-ROOTS/v1\0";
const MAX_BUNDLE_BYTES: usize = 1024 * 1024;
static AUTHENTICATED_BUNDLE: Spinlock<Option<Vec<u8>>> = Spinlock::new(None);

pub(super) fn init() {
    match load() {
        Ok(bundle) => {
            *AUTHENTICATED_BUNDLE.lock() = Some(bundle);
            serial::println("Network: AegisDetect authenticated the HTTPS CA bundle");
        }
        Err(reason) => {
            *AUTHENTICATED_BUNDLE.lock() = None;
            serial::print("Network: HTTPS trust unavailable: ");
            serial::println(reason);
        }
    }
}

fn load() -> Result<Vec<u8>, &'static str> {
    let len = vfs::file_size(PATH)?;
    if !(1..=MAX_BUNDLE_BYTES).contains(&len) {
        return Err("invalid CA bundle size");
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(len + 1)
        .map_err(|_| "CA bundle allocation failed")?;
    bytes.resize(len + 1, 0);
    if vfs::read(PATH, &mut bytes)? != len {
        return Err("CA bundle size changed during read");
    }
    bytes.truncate(len);
    if !aegis_detect::verify_control_file(PATH, SIGNATURE_DOMAIN, &bytes) {
        return Err("invalid or missing CA bundle signature");
    }
    Ok(bytes)
}
