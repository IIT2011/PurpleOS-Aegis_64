//! User Datagram Protocol (UDP)

pub const UDP_HEADER_LEN: usize = 8;

#[derive(Copy, Clone, Debug)]
pub struct UdpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
}

impl UdpHeader {
    pub fn parse(data: &[u8]) -> Option<(Self, &[u8])> {
        if data.len() < UDP_HEADER_LEN {
            return None;
        }
        let src_port = u16::from_be_bytes([data[0], data[1]]);
        let dst_port = u16::from_be_bytes([data[2], data[3]]);
        let length = u16::from_be_bytes([data[4], data[5]]);
        let checksum = u16::from_be_bytes([data[6], data[7]]);

        let packet_len = length as usize;
        if packet_len < UDP_HEADER_LEN || packet_len > data.len() {
            return None;
        }
        let payload_len = packet_len - UDP_HEADER_LEN;
        Some((
            Self {
                src_port,
                dst_port,
                length,
                checksum,
            },
            &data[UDP_HEADER_LEN..UDP_HEADER_LEN + payload_len],
        ))
    }

    pub fn write_to(&self, payload: &[u8], out: &mut [u8]) -> usize {
        let Some(total_len) = UDP_HEADER_LEN.checked_add(payload.len()) else {
            return 0;
        };
        if total_len > u16::MAX as usize || out.len() < total_len {
            return 0;
        }
        out[0..2].copy_from_slice(&self.src_port.to_be_bytes());
        out[2..4].copy_from_slice(&self.dst_port.to_be_bytes());
        out[4..6].copy_from_slice(&(total_len as u16).to_be_bytes());
        out[6..8].copy_from_slice(&[0, 0]); // Optional UDP checksum
        out[UDP_HEADER_LEN..total_len].copy_from_slice(payload);
        total_len
    }
}
