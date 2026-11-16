use crate::arch::x86_64::io::{inw, outw};

const VBE_DISPI_IOPORT_INDEX: u16 = 0x1CE;
const VBE_DISPI_IOPORT_DATA: u16 = 0x1CF;

const VBE_DISPI_INDEX_ID: u16 = 0;
const VBE_DISPI_INDEX_XRES: u16 = 1;
const VBE_DISPI_INDEX_YRES: u16 = 2;
const VBE_DISPI_INDEX_BPP: u16 = 3;
const VBE_DISPI_INDEX_ENABLE: u16 = 4;

const VBE_DISPI_ENABLED: u16 = 0x01;
const VBE_DISPI_LFB_ENABLED: u16 = 0x40;

unsafe fn vbe_write(index: u16, val: u16) {
    outw(VBE_DISPI_IOPORT_INDEX, index);
    outw(VBE_DISPI_IOPORT_DATA, val);
}

unsafe fn vbe_read(index: u16) -> u16 {
    outw(VBE_DISPI_IOPORT_INDEX, index);
    inw(VBE_DISPI_IOPORT_DATA)
}

pub fn is_available() -> bool {
    unsafe {
        let id = vbe_read(VBE_DISPI_INDEX_ID);
        id >= 0xB0C0 && id <= 0xB0C6
    }
}

pub fn set_mode(width: u16, height: u16, bpp: u16) -> bool {
    if !is_available() {
        return false;
    }
    unsafe {
        vbe_write(VBE_DISPI_INDEX_ENABLE, 0); // Disable
        vbe_write(VBE_DISPI_INDEX_XRES, width);
        vbe_write(VBE_DISPI_INDEX_YRES, height);
        vbe_write(VBE_DISPI_INDEX_BPP, bpp);
        vbe_write(VBE_DISPI_INDEX_ENABLE, VBE_DISPI_ENABLED | VBE_DISPI_LFB_ENABLED);
    }
    true
}
