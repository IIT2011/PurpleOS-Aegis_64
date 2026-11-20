//! Boot-scoped local account assigned to newly started user tasks.

use crate::sync::Spinlock;

const PROFILE: &str = "/System/Configuration/account.conf";
const DOMAIN: &[u8] = b"PurpleOS/LOCAL-ACCOUNT/v1\0";
const HOME_PREFIX: &str = "/Users/";

#[derive(Clone, Copy)]
pub(crate) struct Account {
    pub uid: u16,
    pub gid: u16,
    name: [u8; 21],
    len: usize,
}

static LOCAL: Spinlock<Account> = Spinlock::new(Account {
    uid: 1000,
    gid: 1000,
    name: *b"User\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    len: 4,
});

impl Account {
    pub fn name(&self) -> &str {
        core::str::from_utf8(&self.name[..self.len]).unwrap_or("")
    }

    pub fn home<'a>(&self, buffer: &'a mut [u8; 28]) -> &'a str {
        let prefix = HOME_PREFIX.as_bytes();
        let length = prefix.len() + self.len;
        buffer[..prefix.len()].copy_from_slice(prefix);
        buffer[prefix.len()..length].copy_from_slice(&self.name[..self.len]);
        core::str::from_utf8(&buffer[..length]).unwrap_or("")
    }
}

pub(crate) fn local() -> Account {
    *LOCAL.lock()
}

pub(crate) fn init() -> Result<(), &'static str> {
    let Ok(size) = crate::fs::vfs::file_size(PROFILE) else {
        return Ok(()); // Compatibility with system images predating account profiles.
    };
    let mut bytes = [0; 128];
    if size == 0 || size >= bytes.len() || crate::fs::vfs::read(PROFILE, &mut bytes)? != size {
        return Err("invalid local account profile size");
    }
    let payload = &bytes[..size];
    if !crate::security::aegis_detect::verify_control_file(PROFILE, DOMAIN, payload) {
        return Err("invalid local account profile signature");
    }
    let text = core::str::from_utf8(payload).map_err(|_| "invalid local account encoding")?;
    let mut fields = text.split_ascii_whitespace();
    if fields.next() != Some("APACCOUNT1") {
        return Err("invalid local account profile version");
    }
    let name = fields.next().ok_or("missing local account name")?;
    let uid = fields.next().and_then(|value| value.parse::<u16>().ok());
    let gid = fields.next().and_then(|value| value.parse::<u16>().ok());
    let (Some(uid @ 1..), Some(gid @ 1..)) = (uid, gid) else {
        return Err("local account must have nonzero UID and GID");
    };
    if !(1..=21).contains(&name.len()) || matches_ignore_case_reserved(name)
        || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        || fields.next().is_some()
    {
        return Err("invalid local account name or trailing fields");
    }
    let mut account = Account { uid, gid, name: [0; 21], len: name.len() };
    account.name[..name.len()].copy_from_slice(name.as_bytes());
    *LOCAL.lock() = account;
    Ok(())
}

fn matches_ignore_case_reserved(name: &str) -> bool {
    ["aegis", "admin", "root"].iter().any(|reserved| name.eq_ignore_ascii_case(reserved))
}
