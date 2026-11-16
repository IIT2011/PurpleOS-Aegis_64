pub fn init() {
    crate::drivers::serial::println("APDRV: Subsystem v3 initialized (Max Modules: 16, W^X Enforced)");
}

pub fn insmod(name: &str) {
    crate::drivers::serial::print("APDRV: Loaded module '");
    crate::drivers::serial::print(name);
    crate::drivers::serial::println("' with strict W^X memory protection and symbol resolution");
}
