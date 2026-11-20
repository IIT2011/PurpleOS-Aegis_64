use core::str;

pub const MANIFEST_SIZE: usize = 336;
pub const GUI: u32 = 0x02;
pub const SYSADMIN: u32 = 0x10;
const CAPABILITIES: u32 = 0x1f;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Format,
    Unsupported,
    Trust,
    Signature,
    Policy,
    Memory,
    Executable(super::apexec::Error),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    System,
    User,
}

pub struct Bundle<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub capabilities: u32,
    pub priority: u32,
    pub width: u32,
    pub height: u32,
    pub executable: &'a [u8],
    pub resources: &'a [u8],
    pub metadata: &'a [u8],
    pub key_id: &'a [u8; 32],
    pub signature: &'a [u8; 64],
}

fn field(bytes: &[u8]) -> Result<&str, Error> {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .ok_or(Error::Format)?;
    let value = str::from_utf8(&bytes[..end]).map_err(|_| Error::Format)?;
    if bytes[end..].iter().any(|&byte| byte != 0) || value.chars().any(char::is_control) {
        return Err(Error::Format);
    }
    Ok(value)
}

pub fn valid_id(id: &str) -> bool {
    id.split('.').all(|part| {
        !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    })
}

impl<'a> Bundle<'a> {
    pub fn parse(
        bytes: &'a [u8],
        size_limit: usize,
        max_surface: (u32, u32),
    ) -> Result<Self, Error> {
        if bytes.len() > size_limit {
            return Err(Error::Format);
        }
        let header = bytes.get(..MANIFEST_SIZE).ok_or(Error::Format)?;
        let word = |offset| u32::from_le_bytes(header[offset..offset + 4].try_into().unwrap());
        if &header[..8] != b"APAPP200" || word(8) != 2 {
            return Err(Error::Unsupported);
        }
        if word(12) as usize != MANIFEST_SIZE || word(0xe0) != 0 || word(0xe4) != 0 {
            return Err(Error::Format);
        }
        let id = field(&header[0x10..0x30])?;
        let name = field(&header[0x30..0x50])?;
        let version = field(&header[0x50..0x60])?;
        field(&header[0x60..0x80])?;
        field(&header[0x80..0xc0])?;
        let capabilities = word(0xc0);
        let priority = word(0xc4);
        let width = word(0xc8);
        let height = word(0xcc);
        let valid_geometry = if capabilities & GUI != 0 {
            width > 0 && height > 0 && width <= max_surface.0 && height <= max_surface.1
        } else {
            width == 0 && height == 0
        };
        if !valid_id(id)
            || name.is_empty()
            || version.is_empty()
            || !(1..=3).contains(&priority)
            || !valid_geometry
        {
            return Err(Error::Format);
        }
        if capabilities & !CAPABILITIES != 0 {
            return Err(Error::Policy);
        }
        let mut cursor = MANIFEST_SIZE;
        let mut section = |index| {
            let offset = word(index) as usize;
            let size = word(index + 4) as usize;
            if offset == 0 && size == 0 {
                return Ok(&bytes[..0]);
            }
            if offset != cursor || size == 0 {
                return Err(Error::Format);
            }
            let end = offset.checked_add(size).ok_or(Error::Format)?;
            let data = bytes.get(offset..end).ok_or(Error::Format)?;
            cursor = end;
            Ok(data)
        };
        let executable = section(0xd0)?;
        let resources = section(0xd8)?;
        let metadata = section(0xe8)?;
        if executable.is_empty() || cursor != bytes.len() {
            return Err(Error::Format);
        }
        Ok(Self {
            id,
            name,
            capabilities,
            priority,
            width,
            height,
            executable,
            resources,
            metadata,
            key_id: header[0xf0..0x110].try_into().unwrap(),
            signature: header[0x110..0x150].try_into().unwrap(),
        })
    }
}
