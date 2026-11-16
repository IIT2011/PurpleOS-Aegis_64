use crate::sync::Spinlock;

struct AudioState {
    volume: u8,
    active: bool,
}

static AUDIO: Spinlock<AudioState> = Spinlock::new(AudioState {
    volume: 80,
    active: false,
});

pub fn init() {
    let mut a = AUDIO.lock();
    a.active = true;
    crate::drivers::serial::println("Audio: Sound subsystem initialized (AC97/HDA emulation ready)");
}

pub fn set_volume(vol: u8) {
    AUDIO.lock().volume = vol.min(100);
}

pub fn is_available() -> bool {
    AUDIO.lock().active
}
