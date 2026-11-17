//! Address Resolution Protocol (ARP)

use crate::sync::Spinlock;

pub const ARP_OP_REQUEST: u16 = 1;
pub const ARP_OP_REPLY: u16 = 2;
pub const ARP_PACKET_LEN: usize = 28;

#[derive(Copy, Clone, Debug)]
pub struct ArpPacket {
    pub htype: u16,
    pub ptype: u16,
    pub hlen: u8,
    pub plen: u8,
    pub opcode: u16,
    pub sender_mac: [u8; 6],
    pub sender_ip: [u8; 4],
    pub target_mac: [u8; 6],
    pub target_ip: [u8; 4],
}

impl ArpPacket {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < ARP_PACKET_LEN {
            return None;
        }
        let htype = u16::from_be_bytes([data[0], data[1]]);
        let ptype = u16::from_be_bytes([data[2], data[3]]);
        let hlen = data[4];
        let plen = data[5];
        let opcode = u16::from_be_bytes([data[6], data[7]]);

        let mut smac = [0u8; 6];
        let mut sip = [0u8; 4];
        let mut tmac = [0u8; 6];
        let mut tip = [0u8; 4];

        smac.copy_from_slice(&data[8..14]);
        sip.copy_from_slice(&data[14..18]);
        tmac.copy_from_slice(&data[18..24]);
        tip.copy_from_slice(&data[24..28]);

        Some(Self {
            htype,
            ptype,
            hlen,
            plen,
            opcode,
            sender_mac: smac,
            sender_ip: sip,
            target_mac: tmac,
            target_ip: tip,
        })
    }

    pub fn write_to(&self, out: &mut [u8]) -> usize {
        if out.len() < ARP_PACKET_LEN {
            return 0;
        }
        out[0..2].copy_from_slice(&self.htype.to_be_bytes());
        out[2..4].copy_from_slice(&self.ptype.to_be_bytes());
        out[4] = self.hlen;
        out[5] = self.plen;
        out[6..8].copy_from_slice(&self.opcode.to_be_bytes());
        out[8..14].copy_from_slice(&self.sender_mac);
        out[14..18].copy_from_slice(&self.sender_ip);
        out[18..24].copy_from_slice(&self.target_mac);
        out[24..28].copy_from_slice(&self.target_ip);
        ARP_PACKET_LEN
    }
}

pub struct ArpEntry {
    pub ip: [u8; 4],
    pub mac: [u8; 6],
    pub valid: bool,
}

pub const ARP_CACHE_SIZE: usize = 16;
static ARP_CACHE: Spinlock<[ArpEntry; ARP_CACHE_SIZE]> = Spinlock::new([
    ArpEntry { ip: [0; 4], mac: [0; 6], valid: false },
    ArpEntry { ip: [0; 4], mac: [0; 6], valid: false },
    ArpEntry { ip: [0; 4], mac: [0; 6], valid: false },
    ArpEntry { ip: [0; 4], mac: [0; 6], valid: false },
    ArpEntry { ip: [0; 4], mac: [0; 6], valid: false },
    ArpEntry { ip: [0; 4], mac: [0; 6], valid: false },
    ArpEntry { ip: [0; 4], mac: [0; 6], valid: false },
    ArpEntry { ip: [0; 4], mac: [0; 6], valid: false },
    ArpEntry { ip: [0; 4], mac: [0; 6], valid: false },
    ArpEntry { ip: [0; 4], mac: [0; 6], valid: false },
    ArpEntry { ip: [0; 4], mac: [0; 6], valid: false },
    ArpEntry { ip: [0; 4], mac: [0; 6], valid: false },
    ArpEntry { ip: [0; 4], mac: [0; 6], valid: false },
    ArpEntry { ip: [0; 4], mac: [0; 6], valid: false },
    ArpEntry { ip: [0; 4], mac: [0; 6], valid: false },
    ArpEntry { ip: [0; 4], mac: [0; 6], valid: false },
]);

pub fn update_arp_cache(ip: [u8; 4], mac: [u8; 6]) {
    let mut cache = ARP_CACHE.lock();
    for entry in cache.iter_mut() {
        if entry.valid && entry.ip == ip {
            entry.mac = mac;
            return;
        }
    }
    for entry in cache.iter_mut() {
        if !entry.valid {
            entry.ip = ip;
            entry.mac = mac;
            entry.valid = true;
            return;
        }
    }
    // Replace first slot if full
    cache[0] = ArpEntry { ip, mac, valid: true };
}

pub fn lookup_mac(ip: [u8; 4]) -> Option<[u8; 6]> {
    // Broadcast IP
    if ip == [255, 255, 255, 255] {
        return Some([0xFF; 6]);
    }
    let cache = ARP_CACHE.lock();
    for entry in cache.iter() {
        if entry.valid && entry.ip == ip {
            return Some(entry.mac);
        }
    }
    // Default QEMU SLIRP gateway fallback: 52:54:00:12:34:02
    if ip == [10, 0, 2, 2] {
        return Some([0x52, 0x54, 0x00, 0x12, 0x34, 0x02]);
    }
    None
}
