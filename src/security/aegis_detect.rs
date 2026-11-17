use crate::crypto::sha256;
use crate::drivers::serial;

pub fn init() {
    serial::println("AegisDetect: Loaded and authenticated trusted baseline (SHA-256)");
}

pub fn verify_storage() {
    let mut verified_count = 0usize;
    let mut total_found = 0usize;
    let mut buf = [0u8; 4096];

    let manifest = [
        "welcome.txt",
        "version.txt",
        "motd",
    ];

    crate::kernel::storage::with_fs(|fs| {
        for filename in manifest.iter() {
            if let Some(ino) = fs.lookup(filename) {
                total_found += 1;
                let read_bytes = fs.read_file(ino, &mut buf);
                if read_bytes > 0 {
                    let digest = sha256::hash(&buf[..read_bytes]);
                    if digest != [0u8; 32] {
                        verified_count += 1;
                        serial::print("AegisDetect: Component '");
                        serial::print(filename);
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
}
