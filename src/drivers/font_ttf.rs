//! Pure Rust `#![no_std]` TrueType (TTF) Parser for Aegis64 / PurpleOS.
//!
//! Provides zero-allocation parsing of SFNT table directories, `name` records,
//! `head`, `hhea`, `maxp`, and metrics directly from TrueType binary data
//! or streamed on-demand via VFS offsets.

#[derive(Copy, Clone, Debug)]
pub struct TableRecord {
    pub tag: [u8; 4],
    pub checksum: u32,
    pub offset: usize,
    pub length: usize,
}

#[derive(Copy, Clone, Debug)]
pub struct TtfMetadata {
    pub family: [u8; 64],
    pub family_len: usize,
    pub subfamily: [u8; 32],
    pub subfamily_len: usize,
    pub version: [u8; 64],
    pub version_len: usize,
    pub designer: [u8; 64],
    pub designer_len: usize,
    pub units_per_em: u16,
    pub num_glyphs: u16,
    pub ascender: i16,
    pub descender: i16,
    pub line_gap: i16,
}

impl TtfMetadata {
    pub const fn empty() -> Self {
        Self {
            family: [0; 64],
            family_len: 0,
            subfamily: [0; 32],
            subfamily_len: 0,
            version: [0; 64],
            version_len: 0,
            designer: [0; 64],
            designer_len: 0,
            units_per_em: 0,
            num_glyphs: 0,
            ascender: 0,
            descender: 0,
            line_gap: 0,
        }
    }

    pub fn family_str(&self) -> &str {
        core::str::from_utf8(&self.family[..self.family_len]).unwrap_or("Unknown")
    }

    pub fn subfamily_str(&self) -> &str {
        core::str::from_utf8(&self.subfamily[..self.subfamily_len]).unwrap_or("Unknown")
    }

    pub fn version_str(&self) -> &str {
        core::str::from_utf8(&self.version[..self.version_len]).unwrap_or("Unknown")
    }

    pub fn designer_str(&self) -> &str {
        core::str::from_utf8(&self.designer[..self.designer_len]).unwrap_or("Unknown")
    }
}

pub struct TtfTableDirectory {
    pub num_tables: usize,
    pub tables: [Option<TableRecord>; 32],
}

impl TtfTableDirectory {
    pub fn parse(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 12 {
            return Err("TTF data too short for SFNT header");
        }

        let sfnt_version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        if sfnt_version != 0x00010000 && sfnt_version != 0x74727565 && sfnt_version != 0x4F54544F {
            return Err("Invalid SFNT version identifier");
        }

        let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;
        let mut tables: [Option<TableRecord>; 32] = [None; 32];
        let record_count = num_tables.min(32);

        for i in 0..record_count {
            let off = 12 + i * 16;
            if off + 16 > data.len() {
                break;
            }
            let tag = [data[off], data[off + 1], data[off + 2], data[off + 3]];
            let checksum =
                u32::from_be_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
            let offset =
                u32::from_be_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]])
                    as usize;
            let length = u32::from_be_bytes([
                data[off + 12],
                data[off + 13],
                data[off + 14],
                data[off + 15],
            ]) as usize;

            tables[i] = Some(TableRecord {
                tag,
                checksum,
                offset,
                length,
            });
        }

        Ok(Self { num_tables, tables })
    }

    pub fn find_table(&self, tag: &[u8; 4]) -> Option<TableRecord> {
        for t_opt in self.tables.iter() {
            if let Some(t) = t_opt {
                if &t.tag == tag {
                    return Some(*t);
                }
            }
        }
        None
    }

    pub fn parse_head(data: &[u8]) -> u16 {
        if data.len() >= 20 {
            u16::from_be_bytes([data[18], data[19]])
        } else {
            0
        }
    }

    pub fn parse_hhea(data: &[u8]) -> (i16, i16, i16) {
        if data.len() >= 10 {
            let ascender = i16::from_be_bytes([data[4], data[5]]);
            let descender = i16::from_be_bytes([data[6], data[7]]);
            let line_gap = i16::from_be_bytes([data[8], data[9]]);
            (ascender, descender, line_gap)
        } else {
            (0, 0, 0)
        }
    }

    pub fn parse_maxp(data: &[u8]) -> u16 {
        if data.len() >= 6 {
            u16::from_be_bytes([data[4], data[5]])
        } else {
            0
        }
    }

    pub fn parse_name(data: &[u8], meta: &mut TtfMetadata) {
        if data.len() < 6 {
            return;
        }

        let count = u16::from_be_bytes([data[2], data[3]]) as usize;
        let str_offset = u16::from_be_bytes([data[4], data[5]]) as usize;

        for i in 0..count {
            let rec = 6 + i * 12;
            if rec + 12 > data.len() {
                break;
            }

            let platform_id = u16::from_be_bytes([data[rec], data[rec + 1]]);
            let name_id = u16::from_be_bytes([data[rec + 6], data[rec + 7]]);
            let length = u16::from_be_bytes([data[rec + 8], data[rec + 9]]) as usize;
            let offset = str_offset + u16::from_be_bytes([data[rec + 10], data[rec + 11]]) as usize;

            if offset + length <= data.len() {
                let raw = &data[offset..offset + length];
                match name_id {
                    1 if meta.family_len == 0 => {
                        Self::decode_name(platform_id, raw, &mut meta.family, &mut meta.family_len);
                    }
                    2 if meta.subfamily_len == 0 => {
                        Self::decode_name(
                            platform_id,
                            raw,
                            &mut meta.subfamily,
                            &mut meta.subfamily_len,
                        );
                    }
                    5 if meta.version_len == 0 => {
                        Self::decode_name(
                            platform_id,
                            raw,
                            &mut meta.version,
                            &mut meta.version_len,
                        );
                    }
                    9 if meta.designer_len == 0 => {
                        Self::decode_name(
                            platform_id,
                            raw,
                            &mut meta.designer,
                            &mut meta.designer_len,
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    fn decode_name(platform_id: u16, raw: &[u8], dst: &mut [u8], dst_len: &mut usize) {
        if platform_id == 0 || platform_id == 3 {
            let mut pos = 0;
            let mut i = 0;
            while i + 1 < raw.len() && pos < dst.len() {
                if raw[i] == 0 && raw[i + 1] >= 32 && raw[i + 1] <= 126 {
                    dst[pos] = raw[i + 1];
                    pos += 1;
                }
                i += 2;
            }
            *dst_len = pos;
        } else {
            let to_copy = raw.len().min(dst.len());
            for i in 0..to_copy {
                if raw[i] >= 32 && raw[i] <= 126 {
                    dst[i] = raw[i];
                } else {
                    dst[i] = b' ';
                }
            }
            *dst_len = to_copy;
        }
    }
}
