//! Internet Protocol Version 4 (IPv4)

pub const IPV4_HEADER_MIN_LEN: usize = 20;
pub const IPV4_VERSION: u8 = 4;
pub const IPV4_VERSION_IHL_MIN: u8 = 0x45;
pub const IPV4_FRAGMENT_MASK: u16 = 0x3FFF;
pub const IP_PROTO_ICMP: u8 = 1;
pub const IP_PROTO_TCP: u8 = 6;
pub const IP_PROTO_UDP: u8 = 17;

#[derive(Copy, Clone, Debug)]
pub struct Ipv4Header {
    pub version_ihl: u8,
    pub dscp_ecn: u8,
    pub total_len: u16,
    pub id: u16,
    pub flags_frag: u16,
    pub ttl: u8,
    pub protocol: u8,
    pub checksum: u16,
    pub src_ip: [u8; 4],
    pub dst_ip: [u8; 4],
}

impl Ipv4Header {
    pub fn parse(data: &[u8]) -> Option<(Self, &[u8])> {
        if data.len() < IPV4_HEADER_MIN_LEN {
            return None;
        }
        let version_ihl = data[0];
        let version = version_ihl >> 4;
        let ihl = (version_ihl & 0x0F) as usize * 4;
        if version != IPV4_VERSION || ihl < IPV4_HEADER_MIN_LEN || data.len() < ihl {
            return None;
        }

        let dscp_ecn = data[1];
        let total_len = u16::from_be_bytes([data[2], data[3]]);
        let id = u16::from_be_bytes([data[4], data[5]]);
        let flags_frag = u16::from_be_bytes([data[6], data[7]]);
        let ttl = data[8];
        let protocol = data[9];
        let checksum = u16::from_be_bytes([data[10], data[11]]);
        if (total_len as usize) < ihl
            || total_len as usize > data.len()
            || flags_frag & IPV4_FRAGMENT_MASK != 0
            || flags_frag & 0x8000 != 0
            || compute_checksum(&data[..ihl]) != 0
        {
            return None;
        }

        let mut src_ip = [0u8; 4];
        let mut dst_ip = [0u8; 4];
        src_ip.copy_from_slice(&data[12..16]);
        dst_ip.copy_from_slice(&data[16..20]);

        let payload_len = total_len as usize - ihl;
        Some((
            Self {
                version_ihl,
                dscp_ecn,
                total_len,
                id,
                flags_frag,
                ttl,
                protocol,
                checksum,
                src_ip,
                dst_ip,
            },
            &data[ihl..ihl + payload_len],
        ))
    }

    pub fn write_to(&self, out: &mut [u8]) -> usize {
        if out.len() < IPV4_HEADER_MIN_LEN
            || self.version_ihl >> 4 != IPV4_VERSION
            || (self.version_ihl & 0x0F) as usize * 4 != IPV4_HEADER_MIN_LEN
            || (self.total_len as usize) < IPV4_HEADER_MIN_LEN
        {
            return 0;
        }
        out[0] = self.version_ihl;
        out[1] = self.dscp_ecn;
        out[2..4].copy_from_slice(&self.total_len.to_be_bytes());
        out[4..6].copy_from_slice(&self.id.to_be_bytes());
        out[6..8].copy_from_slice(&self.flags_frag.to_be_bytes());
        out[8] = self.ttl;
        out[9] = self.protocol;
        out[10..12].copy_from_slice(&[0, 0]); // Zero for checksum calc
        out[12..16].copy_from_slice(&self.src_ip);
        out[16..20].copy_from_slice(&self.dst_ip);

        // Calculate and insert 16-bit Internet Checksum
        let csum = compute_checksum(&out[..IPV4_HEADER_MIN_LEN]);
        out[10..12].copy_from_slice(&csum.to_be_bytes());

        IPV4_HEADER_MIN_LEN
    }
}

pub fn compute_checksum(data: &[u8]) -> u16 {
    checksum_from(0, data)
}

/// IPv4 TCP/UDP checksum including the pseudo-header. A packet that already
/// carries a correct checksum yields zero.
pub fn transport_checksum(
    source: [u8; 4],
    destination: [u8; 4],
    protocol: u8,
    segment: &[u8],
) -> Option<u16> {
    let length = u16::try_from(segment.len()).ok()?;
    let mut sum = u32::from(length) + u32::from(protocol);
    for address in [source, destination] {
        sum += u32::from(u16::from_be_bytes([address[0], address[1]]));
        sum += u32::from(u16::from_be_bytes([address[2], address[3]]));
    }
    Some(checksum_from(sum, segment))
}

fn checksum_from(mut sum: u32, data: &[u8]) -> u16 {
    let mut i = 0;
    while i + 1 < data.len() {
        let word = u16::from_be_bytes([data[i], data[i + 1]]) as u32;
        sum = sum.wrapping_add(word);
        i += 2;
    }
    if i < data.len() {
        let word = (data[i] as u32) << 8;
        sum = sum.wrapping_add(word);
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}
