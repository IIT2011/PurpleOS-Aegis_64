use crate::arch::x86_64::io::{inb, inw, outb, outw};
use crate::sync::Spinlock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Shutdown,
    Restart,
}

#[derive(Clone, Copy)]
struct Sleep {
    ports: [u16; 2],
    types: [u8; 2],
    enable: Option<(u16, u8)>,
}

#[derive(Clone, Copy)]
struct Platform {
    sleep: Option<Sleep>,
    reset: Option<(u16, u8)>,
}

static PLATFORM: Spinlock<Platform> = Spinlock::new(Platform {
    sleep: None,
    reset: None,
});

unsafe fn table(address: usize, signature: &[u8; 4], maximum: usize) -> Option<&'static [u8]> {
    if !(0x1000..0xfe00_0000 - 36).contains(&address) {
        return None;
    }
    let header = core::slice::from_raw_parts(address as *const u8, 36);
    let length = u32::from_le_bytes(header[4..8].try_into().ok()?) as usize;
    if &header[..4] != signature
        || !(36..=maximum).contains(&length)
        || address.checked_add(length)? > 0xfe00_0000
    {
        return None;
    }
    let data = core::slice::from_raw_parts(address as *const u8, length);
    (data.iter().fold(0u8, |sum, &byte| sum.wrapping_add(byte)) == 0).then_some(data)
}

fn integer(data: &mut &[u8]) -> Option<u8> {
    let (opcode, rest) = data.split_first()?;
    let (value, width) = match opcode {
        0 | 1 => (*opcode, 0),
        0x0a => (*rest.first()?, 1),
        0x0b | 0x0c | 0x0e => {
            let width = match opcode {
                0x0b => 2,
                0x0c => 4,
                _ => 8,
            };
            let value = rest.get(..width)?;
            if value[1..].iter().any(|&byte| byte != 0) {
                return None;
            }
            (value[0], width)
        }
        _ => return None,
    };
    *data = rest.get(width..)?;
    (value <= 7).then_some(value)
}

fn s5_types(aml: &[u8]) -> Option<[u8; 2]> {
    for offset in 0..aml.len() {
        let Some(name) = aml[offset..].strip_prefix(&[0x08]) else {
            continue;
        };
        let name = name.strip_prefix(b"\\").unwrap_or(name);
        let Some(package) = name.strip_prefix(b"_S5_\x12") else {
            continue;
        };
        let lead = *package.first()?;
        let follow = usize::from(lead >> 6);
        let mut length = usize::from(lead & if follow == 0 { 0x3f } else { 0x0f });
        for (index, &byte) in package.get(1..1 + follow)?.iter().enumerate() {
            length |= usize::from(byte) << (4 + 8 * index);
        }
        let mut items = package.get(1 + follow..length)?;
        let (&count, rest) = items.split_first()?;
        if count < 2 {
            continue;
        }
        items = rest;
        if let (Some(a), Some(b)) = (integer(&mut items), integer(&mut items)) {
            return Some([a, b]);
        }
    }
    None
}

pub unsafe fn init(address: usize) {
    let Some(fadt) = table(address, b"FACP", 4096).filter(|fadt| fadt.len() >= 116) else {
        return;
    };
    let word = |offset| u32::from_le_bytes(fadt[offset..offset + 4].try_into().unwrap());
    let flags = word(112);
    let reset = if flags & (1 << 10) != 0 && fadt.len() >= 129 && fadt[116..120] == [1, 8, 0, 1] {
        u16::try_from(u64::from_le_bytes(fadt[120..128].try_into().unwrap()))
            .ok()
            .filter(|&port| port != 0)
            .map(|port| (port, fadt[128]))
    } else {
        None
    };
    let sleep = (|| {
        if flags & (1 << 20) != 0 || fadt[89] < 2 {
            return None;
        }
        let a = u16::try_from(word(64)).ok().filter(|&port| port != 0)?;
        let b = u16::try_from(word(68)).ok()?;
        let dsdt = table(word(40) as usize, b"DSDT", 1024 * 1024).or_else(|| {
            let address = u64::from_le_bytes(fadt.get(140..148)?.try_into().ok()?);
            table(usize::try_from(address).ok()?, b"DSDT", 1024 * 1024)
        })?;
        let types = s5_types(&dsdt[36..])?;
        let enable = u16::try_from(word(48))
            .ok()
            .filter(|&port| port != 0 && fadt[52] != 0)
            .map(|port| (port, fadt[52]));
        Some(Sleep {
            ports: [a, b],
            types,
            enable,
        })
    })();
    *PLATFORM.lock() = Platform { sleep, reset };
}

fn shut_down(sleep: Sleep) -> Result<(), &'static str> {
    const SCI_EN: u16 = 1;
    const SLP_EN: u16 = 1 << 13;
    const SLP_TYP: u16 = 7 << 10;
    unsafe {
        if inw(sleep.ports[0]) & SCI_EN == 0 {
            let Some((port, value)) = sleep.enable else {
                return Err("ACPI mode is unavailable");
            };
            outb(port, value);
            for _ in 0..100 {
                if inw(sleep.ports[0]) & SCI_EN != 0 {
                    break;
                }
                crate::drivers::pit::delay_ms(1);
            }
            if inw(sleep.ports[0]) & SCI_EN == 0 {
                return Err("Could not enable ACPI mode");
            }
        }
        let controls = sleep.ports.map(|port| {
            if port == 0 {
                0
            } else {
                inw(port) & !(SLP_TYP | SLP_EN)
            }
        });
        for ((port, kind), control) in sleep.ports.into_iter().zip(sleep.types).zip(controls) {
            if port != 0 {
                outw(port, control | (u16::from(kind) << 10) | SLP_EN);
            }
        }
    }
    crate::drivers::pit::delay_ms(100);
    Err("Firmware did not power off")
}

fn restart(reset: Option<(u16, u8)>) -> Result<(), &'static str> {
    unsafe {
        if let Some((port, value)) = reset {
            outb(port, value);
            crate::drivers::pit::delay_ms(100);
        }
        for _ in 0..100_000 {
            if inb(0x64) & 2 == 0 {
                outb(0x64, 0xfe);
                crate::drivers::pit::delay_ms(100);
                break;
            }
            core::hint::spin_loop();
        }
    }
    Err("Firmware did not restart")
}

pub fn execute(action: Action) -> Result<(), &'static str> {
    let platform = *PLATFORM.lock();
    if action == Action::Shutdown && platform.sleep.is_none() {
        return Err("ACPI shutdown unavailable");
    }
    // Retain the mount and block-device locks through the power request so no
    // system-volume writes can slip between the final flush and the transition.
    crate::kernel::storage::with_fs(|fs| {
        fs.sync()?;
        match action {
            Action::Shutdown => {
                crate::drivers::serial::println("Power: disk sync complete; shutting down");
                shut_down(platform.sleep.unwrap())
            }
            Action::Restart => {
                crate::drivers::serial::println("Power: disk sync complete; restarting");
                restart(platform.reset)
            }
        }
    })
    .unwrap_or(Err("System disk is unavailable"))
}
