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
        if htype != 1 || ptype != crate::net::eth::ETHERTYPE_IPV4
            || hlen != 6 || plen != 4
            || (opcode != ARP_OP_REQUEST && opcode != ARP_OP_REPLY)
        {
            return None;
        }

        let mut smac = [0u8; 6];
        let mut sip = [0u8; 4];
        let mut tmac = [0u8; 6];
        let mut tip = [0u8; 4];

        smac.copy_from_slice(&data[8..14]);
        sip.copy_from_slice(&data[14..18]);
        tmac.copy_from_slice(&data[18..24]);
        tip.copy_from_slice(&data[24..28]);
        if smac == [0; 6] || (smac[0] & 1) != 0 {
            return None;
        }

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

pub fn send_arp_request(target_ip: [u8; 4]) -> Result<(), &'static str> {
    let my_mac = match crate::net::get_my_mac() {
        Some(m) => m,
        None => return Err("No active Ethernet NIC"),
    };
    let my_ip = crate::net::get_ip();

    let arp_req = ArpPacket {
        htype: 1,
        ptype: crate::net::eth::ETHERTYPE_IPV4,
        hlen: 6,
        plen: 4,
        opcode: ARP_OP_REQUEST,
        sender_mac: my_mac,
        sender_ip: my_ip,
        target_mac: [0x00; 6],
        target_ip,
    };

    let eth_hdr = crate::net::eth::EthHeader {
        dst_mac: [0xFF; 6], // Broadcast
        src_mac: my_mac,
        ether_type: crate::net::eth::ETHERTYPE_ARP,
    };

    let mut buf = [0u8; 64];
    let eth_len = eth_hdr.write_to(&mut buf);
    let arp_len = arp_req.write_to(&mut buf[eth_len..]);
    let total = (eth_len + arp_len).max(60);
    crate::net::send_raw(&buf[..total])
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
    None
}
