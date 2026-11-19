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
    pub fn parse_ipv4(data: &[u8], source: [u8; 4], destination: [u8; 4]) -> Option<(Self, &[u8])> {
        let (header, payload) = Self::parse(data)?;
        if header.checksum != 0 && super::ipv4::transport_checksum(
            source, destination, super::ipv4::IP_PROTO_UDP, &data[..usize::from(header.length)],
        )? != 0 {
            return None;
        }
        Some((header, payload))
    }

    pub fn write_ipv4(&self, payload: &[u8], source: [u8; 4], destination: [u8; 4], out: &mut [u8]) -> usize {
        let length = self.write_to(payload, out);
        if length == 0 { return 0; }
        let checksum = super::ipv4::transport_checksum(source, destination, super::ipv4::IP_PROTO_UDP, &out[..length]).unwrap();
        out[6..8].copy_from_slice(&(if checksum == 0 { u16::MAX } else { checksum }).to_be_bytes());
        length
    }

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
