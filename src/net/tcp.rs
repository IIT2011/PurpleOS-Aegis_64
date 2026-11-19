//! Transmission Control Protocol (TCP)

pub const TCP_HEADER_MIN_LEN: usize = 20;
pub const TCP_FLAG_FIN: u16 = 0x01;
pub const TCP_FLAG_SYN: u16 = 0x02;
pub const TCP_FLAG_RST: u16 = 0x04;
pub const TCP_FLAG_PSH: u16 = 0x08;
pub const TCP_FLAG_ACK: u16 = 0x10;
pub const TCP_FLAG_URG: u16 = 0x20;

#[derive(Copy, Clone, Debug)]
pub struct TcpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub data_offset_flags: u16,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_ptr: u16,
}

impl TcpHeader {
    pub fn parse(data: &[u8]) -> Option<(Self, &[u8])> {
        if data.len() < TCP_HEADER_MIN_LEN {
            return None;
        }
        let src_port = u16::from_be_bytes([data[0], data[1]]);
        let dst_port = u16::from_be_bytes([data[2], data[3]]);
        let seq_num = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let ack_num = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let data_offset_flags = u16::from_be_bytes([data[12], data[13]]);
        let window_size = u16::from_be_bytes([data[14], data[15]]);
        let checksum = u16::from_be_bytes([data[16], data[17]]);
        let urgent_ptr = u16::from_be_bytes([data[18], data[19]]);

        let header_len = ((data_offset_flags >> 12) as usize) * 4;
        if header_len < TCP_HEADER_MIN_LEN || header_len > data.len() {
            return None;
        }

        Some((
            Self {
                src_port,
                dst_port,
                seq_num,
                ack_num,
                data_offset_flags,
                window_size,
                checksum,
                urgent_ptr,
            },
            &data[header_len..],
        ))
    }

    pub fn write_to(&self, out: &mut [u8]) -> usize {
        if out.len() < TCP_HEADER_MIN_LEN
            || (self.data_offset_flags >> 12) as usize != TCP_HEADER_MIN_LEN / 4
        {
            return 0;
        }
        out[0..2].copy_from_slice(&self.src_port.to_be_bytes());
        out[2..4].copy_from_slice(&self.dst_port.to_be_bytes());
        out[4..8].copy_from_slice(&self.seq_num.to_be_bytes());
        out[8..12].copy_from_slice(&self.ack_num.to_be_bytes());
        out[12..14].copy_from_slice(&self.data_offset_flags.to_be_bytes());
        out[14..16].copy_from_slice(&self.window_size.to_be_bytes());
        out[16..18].copy_from_slice(&self.checksum.to_be_bytes());
        out[18..20].copy_from_slice(&self.urgent_ptr.to_be_bytes());
        TCP_HEADER_MIN_LEN
    }
}
