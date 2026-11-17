//! Internet Control Message Protocol (ICMP)

pub const ICMP_TYPE_ECHO_REPLY: u8 = 0;
pub const ICMP_TYPE_ECHO_REQUEST: u8 = 8;
pub const ICMP_HEADER_LEN: usize = 8;

#[derive(Copy, Clone, Debug)]
pub struct IcmpEcho {
    pub msg_type: u8,
    pub code: u8,
    pub checksum: u16,
    pub identifier: u16,
    pub seq_no: u16,
}

impl IcmpEcho {
    pub fn parse(data: &[u8]) -> Option<(Self, &[u8])> {
        if data.len() < ICMP_HEADER_LEN {
            return None;
        }
        let msg_type = data[0];
        let code = data[1];
        let checksum = u16::from_be_bytes([data[2], data[3]]);
        let identifier = u16::from_be_bytes([data[4], data[5]]);
        let seq_no = u16::from_be_bytes([data[6], data[7]]);

        Some((
            Self {
                msg_type,
                code,
                checksum,
                identifier,
                seq_no,
            },
            &data[ICMP_HEADER_LEN..],
        ))
    }

    pub fn write_to(&self, payload: &[u8], out: &mut [u8]) -> usize {
        let total_len = ICMP_HEADER_LEN + payload.len();
        if out.len() < total_len {
            return 0;
        }
        out[0] = self.msg_type;
        out[1] = self.code;
        out[2..4].copy_from_slice(&[0, 0]); // Zero for checksum
        out[4..6].copy_from_slice(&self.identifier.to_be_bytes());
        out[6..8].copy_from_slice(&self.seq_no.to_be_bytes());
        out[ICMP_HEADER_LEN..total_len].copy_from_slice(payload);

        let csum = crate::net::ipv4::compute_checksum(&out[..total_len]);
        out[2..4].copy_from_slice(&csum.to_be_bytes());

        total_len
    }
}
