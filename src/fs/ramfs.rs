//! Pure Rust Bare-Metal RAMFS / TmpFS In-Memory Virtual Filesystem Driver
//!
//! Provides high-speed, volatile RAM-backed file and directory storage
//! for temporary files (/tmp), process pipes, and dynamic system state.

use crate::sync::Spinlock;

pub const RAMFS_MAX_NODES: usize = 32;
pub const RAMFS_MAX_NAME: usize = 32;
pub const RAMFS_NODE_CAPACITY: usize = 4096;

#[derive(Copy, Clone)]
pub struct RamfsNode {
    pub in_use: bool,
    pub is_dir: bool,
    pub name: [u8; RAMFS_MAX_NAME],
    pub size: usize,
    pub data: [u8; RAMFS_NODE_CAPACITY],
}

impl RamfsNode {
    pub const fn empty() -> Self {
        Self {
            in_use: false,
            is_dir: false,
            name: [0; RAMFS_MAX_NAME],
            size: 0,
            data: [0; RAMFS_NODE_CAPACITY],
        }
    }
}

pub struct RamfsState {
    pub nodes: [RamfsNode; RAMFS_MAX_NODES],
    pub count: usize,
}

static RAMFS: Spinlock<RamfsState> = Spinlock::new(RamfsState {
    nodes: [RamfsNode::empty(); RAMFS_MAX_NODES],
    count: 0,
});

pub struct RamfsDriver;

impl RamfsDriver {
    pub fn create_file(name: &str, is_dir: bool) -> Result<usize, &'static str> {
        let mut st = RAMFS.lock();
        if st.count >= RAMFS_MAX_NODES {
            return Err("RAMFS node table exhausted");
        }

        // Check if file exists
        for (i, node) in st.nodes.iter().enumerate() {
            if node.in_use {
                let node_name = core::str::from_utf8(&node.name)
                    .unwrap_or("")
                    .trim_matches('\0');
                if node_name == name {
                    return Ok(i);
                }
            }
        }

        // Find free slot
        for (i, node) in st.nodes.iter_mut().enumerate() {
            if !node.in_use {
                node.in_use = true;
                node.is_dir = is_dir;
                node.size = 0;
                node.name = [0; RAMFS_MAX_NAME];
                let bytes = name.as_bytes();
                let len = bytes.len().min(RAMFS_MAX_NAME - 1);
                node.name[..len].copy_from_slice(&bytes[..len]);
                st.count += 1;
                return Ok(i);
            }
        }

        Err("No free RAMFS slots")
    }

    pub fn write_file(name: &str, data: &[u8]) -> Result<usize, &'static str> {
        let mut st = RAMFS.lock();
        for node in st.nodes.iter_mut() {
            if node.in_use {
                let node_name = core::str::from_utf8(&node.name)
                    .unwrap_or("")
                    .trim_matches('\0');
                if node_name == name {
                    let len = data.len().min(RAMFS_NODE_CAPACITY);
                    node.data[..len].copy_from_slice(&data[..len]);
                    node.size = len;
                    return Ok(len);
                }
            }
        }
        Err("File not found in RAMFS")
    }

    pub fn read_file(name: &str, buf: &mut [u8]) -> Result<usize, &'static str> {
        let st = RAMFS.lock();
        for node in st.nodes.iter() {
            if node.in_use {
                let node_name = core::str::from_utf8(&node.name)
                    .unwrap_or("")
                    .trim_matches('\0');
                if node_name == name {
                    let len = node.size.min(buf.len());
                    buf[..len].copy_from_slice(&node.data[..len]);
                    return Ok(len);
                }
            }
        }
        Err("File not found in RAMFS")
    }

    pub fn remove_file(name: &str) -> Result<(), &'static str> {
        let mut st = RAMFS.lock();
        for node in st.nodes.iter_mut() {
            if node.in_use {
                let node_name = core::str::from_utf8(&node.name)
                    .unwrap_or("")
                    .trim_matches('\0');
                if node_name == name {
                    node.in_use = false;
                    node.size = 0;
                    st.count = st.count.saturating_sub(1);
                    return Ok(());
                }
            }
        }
        Err("File not found in RAMFS")
    }

    pub fn list<F: FnMut(&str, usize, bool)>(mut cb: F) {
        let st = RAMFS.lock();
        for node in st.nodes.iter() {
            if node.in_use {
                let name = core::str::from_utf8(&node.name)
                    .unwrap_or("")
                    .trim_matches('\0');
                cb(name, node.size, node.is_dir);
            }
        }
    }
}
