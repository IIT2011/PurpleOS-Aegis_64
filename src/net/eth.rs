//! Ethernet II Frame Processing

pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;
pub const ETH_HEADER_LEN: usize = 14;

#[derive(Copy, Clone, Debug)]
pub struct EthHeader {
    pub dst_mac: [u8; 6],
    pub src_mac: [u8; 6],
    pub ether_type: u16,
}

impl EthHeader {
    pub fn parse(data: &[u8]) -> Option<(Self, &[u8])> {
        if data.len() < ETH_HEADER_LEN {
            return None;
        }
        let mut dst = [0u8; 6];
        let mut src = [0u8; 6];
        dst.copy_from_slice(&data[0..6]);
        src.copy_from_slice(&data[6..12]);
        let et = u16::from_be_bytes([data[12], data[13]]);
        Some((
            Self {
                dst_mac: dst,
                src_mac: src,
                ether_type: et,
            },
            &data[ETH_HEADER_LEN..],
        ))
    }

    pub fn write_to(&self, out: &mut [u8]) -> usize {
        if out.len() < ETH_HEADER_LEN {
            return 0;
        }
        out[0..6].copy_from_slice(&self.dst_mac);
        out[6..12].copy_from_slice(&self.src_mac);
        let et_b = self.ether_type.to_be_bytes();
        out[12] = et_b[0];
        out[13] = et_b[1];
        ETH_HEADER_LEN
    }
}
